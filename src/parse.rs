use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::types::{
    AnyNodeVariant, Block, Call, Expression, Ident, LValue, Literal, Node, NodeVariant, Program,
    Statement, String as StringVariant, TopError,
};

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
            Rule::stmt => self.parse_statement(pair.into_inner().next().unwrap()),
            Rule::expr => {
                let expression =
                    self.parse_expression(pair.clone().into_inner().next().unwrap())?;
                Ok(Statement::Expression(expression).into_node(&pair))
            }
            rule => unreachable!("{rule:?}"),
        }
    }

    pub fn parse_expression(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::expr => self.parse_expression(pair.into_inner().next().unwrap()),
            Rule::call => {
                let [target, arg] = pair.clone().into_inner().next_chunk().unwrap();
                Ok(Expression::Call(
                    Call {
                        target: LValue::Ident(
                            Ident {
                                name: target.as_str().into(),
                            }
                            .into_node(&target),
                        )
                        .into_node(&target),
                        arguments: vec![self.parse_expression(arg)?],
                    }
                    .into_node(&pair),
                )
                .into_node(&pair))
            }
            Rule::string => Ok(Expression::Literal(
                Literal::String(
                    StringVariant {
                        value: pair.as_str().into(),
                    }
                    .into_node(&pair),
                )
                .into_node(&pair),
            )
            .into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }
}
