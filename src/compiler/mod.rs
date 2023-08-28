use crate::types::{Executable, Instruction, SyntaxNode};

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {}

type Result<T = (), E = Error> = std::result::Result<T, E>;

#[derive(Debug, Default)]
struct Compiler {
    instructions: Vec<Instruction>,
}

impl Compiler {
    fn add(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }

    pub fn compile(mut self, node: SyntaxNode) -> Result<Executable> {
        unimplemented!()
    }
}