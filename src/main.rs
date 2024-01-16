#![feature(decl_macro)]
#![feature(new_uninit)]
#![feature(strict_provenance)]
#![feature(iter_next_chunk)]
#![feature(let_chains)]
#![feature(map_try_insert)]
#![feature(iter_array_chunks)]
#![feature(iter_map_windows)]
#![feature(iterator_try_collect)]
#![feature(yeet_expr)]
#![feature(try_blocks)]

use std::env::args;

use crate::runtime::Runtime;
use crate::types::TopError;

mod parse;
mod runtime;
mod types;

fn run() -> Result<(), TopError> {
    let [_executable, filename] = args().next_chunk().unwrap_or_default();
    let mut runtime = Runtime::new();
    runtime.exec_file("./examples/std.concorde")?;
    runtime.exec_file(filename)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
    }
}
