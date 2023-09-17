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
        let body = self.parse_block(pair.clone().into_inner().next().unwrap())?;
        Ok(Program { body }.into_node(&pair))
    }

    fn parse_block(&mut self, pair: Pair<Rule>) -> Result<Node<Block>> {
        Ok(Block {
            statements: self.parse_statements(pair.clone())?,
        }
        .into_node(&pair))
    }

    fn parse_statements(&mut self, pair: Pair<Rule>) -> Result<Vec<Node<Statement>>> {
        pair.into_inner()
            .map(|pair| self.parse_statement(pair.into_inner().next().unwrap()))
            .collect()
    }

    pub fn parse_statement(&mut self, pair: Pair<Rule>) -> Result<Node<Statement>> {
        match pair.as_rule() {
            Rule::expr => Ok(Statement::Expression(
                self.parse_expression(pair.clone().into_inner().next().unwrap())?,
            )
            .into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }

    pub fn parse_expression(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::expr => self.parse_expression(pair.into_inner().next().unwrap()),
            Rule::call => {
                let [target, args] = pair.clone().into_inner().next_chunk().unwrap();
                let arguments = self.parse_expr_list(args)?;
                let target = self.parse_lvalue(target)?;
                Ok(Expression::Call(Call { target, arguments }.into_node(&pair)).into_node(&pair))
            }
            Rule::literal => Ok(self.parse_literal(pair.into_inner().next().unwrap())?),
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_literal(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::string => Ok(Expression::Literal(
                Literal::String(
                    StringVariant {
                        value: pair.clone().into_inner().next().unwrap().as_str().into(),
                    }
                    .into_node(&pair),
                )
                .into_node(&pair),
            )
            .into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_expr_list(&mut self, expr_list: Pair<Rule>) -> Result<Vec<Node<Expression>>, Error> {
        expr_list
            .into_inner()
            .map(|arg| self.parse_expression(arg))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn parse_lvalue(&mut self, pair: Pair<Rule>) -> Result<Node<LValue>> {
        match pair.as_rule() {
            Rule::ident => Ok(LValue::Ident(
                Ident {
                    name: pair.as_str().into(),
                }
                .into_node(&pair),
            )
            .into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }
}
