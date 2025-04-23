//! # Anywrap
//! Anywrap is an error handler designed for applications, similar to anyhow, but it supports matching on enum variants, making it more ergonomic.

//! # Example
//!
//! ```rust
//! use std::fmt;
//! use std::fs::File;
//! use anywrap::{anywrap, AnyWrap};
//! 
//! pub struct ErrorCode(pub u32);
//! 
//! impl fmt::Display for ErrorCode {
//!    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//!       write!(f, "{}", self.0)
//!    }
//! }
//! 
//! #[derive(AnyWrap)]
//! #[anywrap]
//! pub enum Error {
//!    #[anywrap_attr(display = "Error Code: {code}", from = "code")]
//!    Code { code: ErrorCode },
//!    #[anywrap_attr(display = "{source}")]
//!    IO { source: std::io::Error },
//! }
//! 
//! pub type Result<T, E = Error> = std::result::Result<T, E>;
//! 
//! pub fn define_error() -> Result<()> {
//!    let e = Error::from(ErrorCode(1));
//!    Err(e)
//! }
//! 
//! pub fn chain1() -> Result<()> {
//!    define_error().context("chain1")
//! }
//! 
//! pub fn with_chain() -> Result<()> {
//!    chain1().context("with_chain")
//! }
//! 
//! pub fn auto() -> Result<()> {
//!    let _ = File::open("test.txt")?;
//! 
//!    Ok(())
//! }
//! 
//! fn main() {
//!    if let Err(e) = auto() {
//!       println!("--12: {e:?}");
//!    }
//!    if let Err(e) = with_chain() {
//!       println!("--15 display: {e}");
//!       println!("--15 debug: {e:?}");
//!    }
//! }
//! ```

pub mod location;
pub use anywrap_macro::{anywrap, AnyWrap};
