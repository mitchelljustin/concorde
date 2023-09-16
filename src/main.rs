#![feature(decl_macro)]

use crate::runtime::Runtime;

mod parse;
mod runtime;
mod types;

fn main() {
    let runtime = Runtime::new();
    println!("Hello, world!");
}
