#![feature(decl_macro)]
#![feature(new_uninit)]
#![feature(strict_provenance)]
#![feature(iter_next_chunk)]
#![feature(let_chains)]
#![feature(map_try_insert)]
#![feature(iter_array_chunks)]

use crate::parse::SourceParser;
use crate::runtime::Runtime;
use crate::types::TopError;
use std::fs;

mod parse;
mod runtime;
mod types;

fn run() -> Result<(), TopError> {
    let mut runtime = Runtime::new();
    let lib_source = fs::read_to_string("./lib.concorde")?;
    let program = SourceParser::default().parse(&lib_source)?;
    runtime.exec_program(program)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
    }
}
