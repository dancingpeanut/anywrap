#![allow(warnings)]
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Expr, ExprLit, Fields, FieldsNamed, ItemEnum, Lit, Meta, Token};
use syn::punctuated::Punctuated;

#[proc_macro_attribute]
pub fn anywrap(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_enum = parse_macro_input!(item as ItemEnum);

    if let Err(err) = add_attr_impl(&mut input_enum) {
        return err.to_compile_error().into();
    }

    let std_error_ts = from_std_error_impl(&input_enum);
    let chain_ts = chain_impl(&input_enum);
    let context_ts = context_impl(&input_enum);
    let wrap_ts = wrap_impl(&input_enum);

    let output = quote! {
        #input_enum
        #std_error_ts
        #chain_ts
        #context_ts
        #wrap_ts
    };

    output.into()
}

// --- 增加属性 逻辑 ---
fn add_attr_impl(input_enum: &mut ItemEnum) -> Result<(), syn::Error> {
    let enum_ident = &input_enum.ident;

    let extra_fields = quote! {
        location: anywrap::location::Location,
        chain: Option<Box<#enum_ident>>
    };

    for variant in &mut input_enum.variants {
        if let Fields::Named(fields_named) = &mut variant.fields {
            let parsed: FieldsNamed = syn::parse2(quote!({ #extra_fields }))?;
            fields_named.named.extend(parsed.named);
        } else {
            return Err(syn::Error::new_spanned(
                variant,
                "Only struct-like enum variants are supported",
            ));
        }
    }

    let extra_variants: ItemEnum = syn::parse2(quote! {
        enum Dummy {
            #[anywrap_attr(display = "{msg}")]
            Context {
                msg: String,
                #extra_fields
            },
            #[anywrap_attr(display = "{source}")]
            Any {
                source: Box<dyn std::error::Error + Send + Sync + 'static>,
                #extra_fields
            }
        }
    })?;

    input_enum.variants.extend(extra_variants.variants);

    Ok(())
}

// --- enrich_with_chain 逻辑 ---
fn chain_impl(input_enum: &ItemEnum) -> proc_macro2::TokenStream {
    let enum_ident = &input_enum.ident;
    let mut match_arms = Vec::new();

    for variant in &input_enum.variants {
        let ident = &variant.ident;

        // 只匹配具名字段
        if let Fields::Named(ref fields_named) = variant.fields {
            let has_chain = fields_named.named.iter().any(|f| {
                f.ident.as_ref().map(|i| i == "chain").unwrap_or(false)
            });

            if has_chain {
                match_arms.push(quote! {
                    #enum_ident::#ident { chain, .. } => {
                        if let Some(chained) = chain {
                            current = chained;
                        } else {
                            *chain = Some(Box::new(next));
                            break;
                        }
                    }
                });
            }
        }
    }

    quote! {
        impl #enum_ident {
            pub fn push_chain(mut self, next: Self) -> Self {
                let mut current = &mut self;
                loop {
                    match current {
                        #(#match_arms),*
                        _ => break,
                    }
                }
                self
            }
        }
    }
}

// --- 标准Error的From实现 逻辑 ---
fn from_std_error_impl(input_enum: &ItemEnum) -> proc_macro2::TokenStream {
    let enum_ident = &input_enum.ident;
    quote! {
        impl<E> From<E> for #enum_ident
        where
            E: core::error::Error + Send + Sync + 'static,
        {
            #[track_caller]
            fn from(e: E) -> Self {
                #enum_ident::Any {
                    source: Box::new(e),
                    location: anywrap::location::Location::default(),
                    chain: None,
                }
            }
        }
    }
}

// --- Context 逻辑 ---
fn context_impl(input_enum: &ItemEnum) -> proc_macro2::TokenStream {
    let enum_ident = &input_enum.ident;
    quote! {
        pub trait Context<T, E> {
            fn context<M>(self, msg: M) -> std::result::Result<T, #enum_ident>
            where
                M: std::fmt::Display + Send + Sync + 'static;
        }

        impl<T, E> Context<T, E> for std::result::Result<T, E>
        where
            E: core::error::Error + Send + Sync + 'static,
        {
            #[track_caller]
            fn context<M>(self, msg: M) -> std::result::Result<T, #enum_ident>
            where
                M: std::fmt::Display + Send + Sync + 'static,
            {
                self.map_err(|e| {
                    let a = #enum_ident::Any {
                        source: Box::new(e),
                        location: anywrap::location::Location::default(),
                        chain: None,
                    };
                    let m = #enum_ident::Context {
                        msg: msg.to_string(),
                        location: anywrap::location::Location::default(),
                        chain: None,
                    };
                    a.push_chain(m)
                })
            }
        }

        impl<T> Context<T, #enum_ident> for std::result::Result<T, #enum_ident> {
            #[track_caller]
            fn context<M>(self, msg: M) -> std::result::Result<T, #enum_ident>
            where
                M: std::fmt::Display + Send + Sync + 'static,
            {
                let location = anywrap::location::Location::default();
                self.map_err(|e| {
                    let m = #enum_ident::Context {
                        msg: msg.to_string(),
                        location: location,
                        chain: None,
                    };
                    e.push_chain(m)
                })
            }
        }
    }
}

// --- wrap 逻辑 ---
fn wrap_impl(input_enum: &ItemEnum) -> proc_macro2::TokenStream {
    let enum_name = &input_enum.ident;
    let mut impls = vec![];

    for variant in &input_enum.variants {
        let variant_name = &variant.ident;

        if let Fields::Named(fields_named) = &variant.fields {
            let mut error_field_type = None;
            let mut field_assignments = vec![];

            for field in &fields_named.named {
                let ident = field.ident.as_ref().unwrap();
                if ident == "source" {
                    error_field_type = Some(&field.ty);
                    field_assignments.push(quote! { #ident: e });
                } else if ident == "location" {
                    field_assignments.push(quote! { #ident: location });
                } else {
                    field_assignments.push(quote! { #ident: Default::default() });
                }
            }

            if let Some(error_ty) = error_field_type {
                impls.push(quote! {
                    impl<T> Wrap<T> for std::result::Result<T, #error_ty> {
                        #[track_caller]
                        fn wrap(self) -> std::result::Result<T, #enum_name> {
                            let location = anywrap::location::Location::default();
                            self.map_err(|e| {
                                #enum_name::#variant_name {
                                    #(#field_assignments),*
                                }
                            })
                        }
                    }
                });
            }
        }
    }

    quote! {
        pub trait Wrap<T> {
            fn wrap(self) -> std::result::Result<T, #enum_name>;
        }

        #(#impls)*
    }
}

#[proc_macro_derive(AnyWrap, attributes(anywrap_attr))]
pub fn derive_anywrap(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let enum_ident = &input.ident;
    let Data::Enum(data_enum) = &input.data else {
        return syn::Error::new_spanned(&input, "only enums are supported")
            .to_compile_error()
            .into();
    };

    let mut match_lines = Vec::new();
    let mut chain_lines = Vec::new();
    let mut chain_arms = Vec::new();
    let mut from_impls = Vec::new();

    for variant in &data_enum.variants {
        let variant_ident = &variant.ident;

        let Fields::Named(fields_named) = &variant.fields else {
            return syn::Error::new_spanned(variant, "only named fields are supported")
                .to_compile_error()
                .into();
        };

        let field_idents: Vec<&Ident> = fields_named
            .named
            .iter()
            .filter_map(|f| f.ident.as_ref())
            .collect();

        // 解析属性
        let mut display_format = None;
        let mut from_field = None;

        for attr in &variant.attrs {
            if let Some(expr) = get_attr_value(attr, "anywrap_attr", "display") {
                if let Lit::Str(lit) = &expr.lit {
                    display_format = Some(lit.value());
                }
            }
            if let Some(expr) = get_attr_value(attr, "anywrap_attr", "from") {
                if let Lit::Str(lit) = &expr.lit {
                    from_field = Some(lit.value());
                }
            }
        }
        let display_fmt = display_format.unwrap_or_else(|| {
            panic!(
                "Missing #[anywrap_attr(display = \"...\")] for variant `{}`",
                variant_ident
            )
        });

        if from_field.is_some() {
            // 获取变体字段信息
            let fields = match &variant.fields {
                Fields::Named(fields) => &fields.named,
                _ => panic!("枚举变体必须使用命名字段"),
            };

            if fields.len() != 1 {
                panic!("只有单个字段的变体才能实现 From trait. 变体名: {}", variant_ident);
            }

            // 查找指定的字段
            let field = fields.iter().find(|f| {
                f.ident.as_ref().map(|i| i.to_string()) == from_field
            }).expect(&format!("找不到指定的字段: {:?}", from_field));

            // 获取字段名
            let field_name = field.ident.as_ref().unwrap();
            // 获取字段类型
            let field_type = &field.ty;

            // 生成 From 实现
            from_impls.push(quote! {
                impl From<#field_type> for #enum_ident {
                    fn from(source: #field_type) -> Self {
                        #enum_ident::#variant_ident {
                            #field_name: source,
                            location: Default::default(),
                            chain: None,
                        }
                    }
                }
            });
        }

        let match_arm = quote! {
            #enum_ident::#variant_ident { #( #field_idents, )* .. } => format!(#display_fmt),
        };
        match_lines.push(match_arm);

        let chain_arm = quote! {
            #enum_ident::#variant_ident { #( #field_idents, )* location, .. } => {
                format!("{idx}: {}, at {location}", format!(#display_fmt))
            }
        };
        chain_lines.push(chain_arm);

        let chain_extractor = quote! {
            #enum_ident::#variant_ident { chain, .. } => chain.as_deref(),
        };
        chain_arms.push(chain_extractor);
    }

    let output = quote! {
        impl std::fmt::Display for #enum_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                writeln!(f, "{}", match self {
                    #( #match_lines )*
                    Error::Context { msg, .. } => format!("{msg}"),
                    Error::Any { source, .. } => format!("{source}"),
                })?;
                Ok(())
            }
        }
        impl std::fmt::Debug for #enum_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                writeln!(f, "{}", match self {
                    #( #match_lines )*
                    Error::Context { msg, .. } => format!("{msg}"),
                    Error::Any { source, .. } => format!("{source}"),
                })?;

                fn write_chain(err: &#enum_ident, f: &mut std::fmt::Formatter<'_>, idx: usize) -> std::fmt::Result {
                    let line = match err {
                        #( #chain_lines )*
                        Error::Context { msg, location, .. } => format!("{idx}: {msg}, at {location}"),
                        Error::Any { source, location, .. } => format!("{idx}: {source}, at {location}"),
                    };
                    writeln!(f, "{}", line)?;

                    if let Some(inner) = match err {
                        #( #chain_arms )*
                        Error::Context { chain, .. } => chain.as_deref(),
                        Error::Any { chain, .. } => chain.as_deref(),
                    } {
                        write_chain(inner, f, idx + 1)?;
                    }

                    Ok(())
                }

                write_chain(self, f, 0)
            }
        }

        #(#from_impls)*
    };

    output.into()
}

fn get_attr_value(attr: &Attribute, attr_name: &str, key: &str) -> Option<ExprLit> {
    if attr.path().is_ident(attr_name) {
        if let Meta::List(meta) = &attr.meta {
            for nested in meta.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).unwrap() {
                match nested {
                    Meta::NameValue(name_value) => {
                        if name_value.path.is_ident(key) {
                            if let Expr::Lit(expr_lit) = &name_value.value {
                                return Some(expr_lit.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    None
}
