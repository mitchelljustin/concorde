use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::types::{AnyNodeVariant, Block, Node, NodeVariant, Program, Statement, TopError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("pest error: {0}")]
    Pest(#[from] pest::error::Error<Rule>),
}

type Result<T = Node<AnyNodeVariant>, E = Error> = std::result::Result<T, E>;

#[derive(Parser)]
#[grammar = "concorde.pest"]
struct ConcordeParser;

#[derive(Default, Debug)]
pub struct SourceParser {}

impl SourceParser {
    pub fn parse(mut self, source: &str) -> Result<Node<Program>, TopError> {
        let pair = ConcordeParser::parse(Rule::program, source)
            .map_err(Error::Pest)?
            .next()
            .unwrap();
        let body = Block {
            statements: pair
                .clone()
                .into_inner()
                .map(|pair| self.parse_statement(pair))
                .collect::<Result<_>>()?,
        }
        .into_node(&pair);
        Ok(Program { body }.into_node(&pair))
    }

    pub fn parse_statement(&mut self, pair: Pair<Rule>) -> Result<Node<Statement>> {
        match pair.as_rule() {
            rule => unreachable!("{rule:?}"),
        }
    }
}
