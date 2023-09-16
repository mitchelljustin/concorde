#![feature(decl_macro)]

use crate::runtime::Runtime;
use crate::types::TopError;

mod parse;
mod runtime;
mod types;

fn main() -> Result<(), TopError> {
    let runtime = Runtime::new();
    let result = runtime.resolve("String")?;
    println!("{}", result.borrow().debug());
    Ok(())
}
