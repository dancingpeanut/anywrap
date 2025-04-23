use std::fs::File;
use crate::error::{Context, Error, ErrorCode, Result, Wrap};

pub fn wrap_to_io() -> Result<()> {
    // 使用wrap转换成Error::IO
    let _ = File::open("test.txt").wrap()?;

    Ok(())
}

pub fn auto() -> Result<()> {
    // 类似anyhow，自动转成Error::Any
    let _ = File::open("test.txt")?;

    Ok(())
}

pub fn with_context() -> Result<()> {
    wrap_to_io().context("11")
}

pub fn define_error() -> Result<()> {
    let e = Error::from(ErrorCode(1));
    Err(e)
}

pub fn chain1() -> Result<()> {
    define_error().context("chain1")
}

pub fn with_chain() -> Result<()> {
    chain1().context("with_chain")
}
