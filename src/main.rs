#![feature(decl_macro)]
#![feature(new_uninit)]
#![feature(strict_provenance)]

use crate::runtime::Runtime;
use crate::types::TopError;

mod parse;
mod runtime;
mod types;

fn main() -> Result<(), TopError> {
    let mut runtime = Runtime::new();
    println!("{}", runtime.resolve("String").unwrap().borrow().debug());
    println!("{}", runtime.create_string("Swag".into()).borrow().debug());
    Ok(())
}
