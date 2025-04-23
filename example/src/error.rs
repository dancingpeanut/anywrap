use std::fmt;
use anyhow::anyhow;
use anywrap::{anywrap, AnyWrap};

pub struct ErrorCode(pub u32);

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(AnyWrap)]
#[anywrap]
pub enum Error {
    // 自定义Error，可以指定from来自动实现From Trait，必须是单字段类型
    #[anywrap_attr(display = "Error Code: {code}", from = "code")]
    Code { code: ErrorCode },
    // 标准错误，无需指定from
    #[anywrap_attr(display = "{source}")]
    IO { source: std::io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// TODO 自动实现，参考anyhow
impl From<Error> for Box<dyn std::error::Error + Send + Sync + 'static> {
    fn from(e: Error) -> Self {
        match e {
            Error::Code { code, .. } => anyhow!("Error Code: {code}").into(),
            Error::IO { source, .. } => Box::new(source),
            Error::Context { msg, .. } => anyhow!("{msg}").into(),
            Error::Any { source, .. } => source,
        }
    }
}

#[macro_export]
macro_rules! anyerr {
    ($($arg:tt)*) => {{
        let formatted_msg = format!($($arg)*);
        $crate::error::Error::Context { msg: formatted_msg, location: Default::default(), chain: None, }
    }};
}
