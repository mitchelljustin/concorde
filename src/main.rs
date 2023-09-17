#![feature(decl_macro)]
#![feature(new_uninit)]
#![feature(strict_provenance)]
#![feature(iter_next_chunk)]

use crate::parse::SourceParser;
use crate::runtime::Runtime;
use crate::types::TopError;
use std::fs;

mod parse;
mod runtime;
mod types;

fn main() -> Result<(), TopError> {
    let mut runtime = Runtime::new();
    let lib_source = fs::read_to_string("./lib.concorde")?;
    let program = SourceParser::default().parse(&lib_source)?;
    runtime.exec(program.v.body.v.statements[0].clone())?;
    Ok(())
}
