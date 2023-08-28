use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::types::{AnyNode, Block, Program, Statement, SyntaxNode, TopError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("pest error: {0}")]
    Pest(#[from] pest::error::Error<Rule>),
}

type Result<T = SyntaxNode<AnyNode>, E = Error> = std::result::Result<T, E>;

#[derive(Parser)]
#[grammar = "concorde.pest"]
struct ConcordeParser;

#[derive(Default, Debug)]
pub struct SourceParser {}

impl SourceParser {
    pub fn parse(mut self, source: &str) -> Result<SyntaxNode<Program>, TopError> {
        let pair = ConcordeParser::parse(Rule::program, source)
            .map_err(Error::Pest)?
            .next()
            .unwrap();
        Ok(SyntaxNode {
            source: pair.as_str().to_string(),
            variant: Program {
                body: Block {
                    stmts: pair
                        .into_inner()
                        .map(|pair| self.parse_statement(pair))
                        .collect::<Result<_, _>>()?,
                },
            },
        })
    }

    pub fn parse_statement(&mut self, pair: Pair<Rule>) -> Result<SyntaxNode<Statement>> {}
}
