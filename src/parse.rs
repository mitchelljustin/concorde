use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::types::{
    Access, AnyNodeVariant, Assignment, Block, Call, Expression, Ident, LValue, Literal, Node,
    NodeVariant, Program, Statement, String as StringVariant, TopError, Variable,
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
        let statements = self.parse_statements(pair.clone())?;
        Ok(Block { statements }.into_node(&pair))
    }

    fn parse_statements(&mut self, pair: Pair<Rule>) -> Result<Vec<Node<Statement>>> {
        pair.into_inner()
            .map(|pair| self.parse_statement(pair.into_inner().next().unwrap()))
            .collect()
    }

    fn parse_statement(&mut self, pair: Pair<Rule>) -> Result<Node<Statement>> {
        match pair.as_rule() {
            Rule::stmt => self.parse_statement(pair.into_inner().next().unwrap()),
            Rule::assignment => {
                let [target, value] = pair.clone().into_inner().next_chunk().unwrap();
                let target = self.parse_lvalue(target)?;
                let value = self.parse_expression(value)?;
                Ok(
                    Statement::Assignment(Assignment { target, value }.into_node(&pair))
                        .into_node(&pair),
                )
            }
            Rule::expr => {
                Ok(Statement::Expression(self.parse_expression(pair.clone())?).into_node(&pair))
            }
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_expression(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::expr | Rule::primary => self.parse_expression(pair.into_inner().next().unwrap()),
            Rule::access => {
                let mut inner = pair.clone().into_inner();
                let target = inner.next().unwrap();
                let mut target = self.parse_expression(target)?;
                while let Some(member) = inner.next() {
                    let member = self.parse_expression(member)?;
                    target = Expression::Access(
                        Access {
                            target: Box::new(target),
                            member: Box::new(member),
                        }
                        .into_node(&pair),
                    )
                    .into_node(&pair);
                }
                Ok(target)
            }
            Rule::call => {
                let mut inner = pair.clone().into_inner();
                let target = inner.next().unwrap();
                let Some(expr_list) = inner.next() else {
                    return self.parse_expression(target);
                };
                let arguments = self.parse_expr_list(expr_list)?;
                let target = Box::new(self.parse_expression(target)?);
                Ok(Expression::Call(Call { target, arguments }.into_node(&pair)).into_node(&pair))
            }
            Rule::literal => Ok(self.parse_literal(pair)?),
            Rule::variable => Ok(Expression::Variable(self.parse_variable(&pair)).into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_literal(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::literal => self.parse_literal(pair.into_inner().next().unwrap()),
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

    fn parse_lvalue(&mut self, pair: Pair<Rule>) -> Result<Node<LValue>> {
        match pair.as_rule() {
            Rule::lvalue | Rule::variable => self.parse_lvalue(pair.into_inner().next().unwrap()),
            Rule::ident => Ok(LValue::Variable(self.parse_variable(&pair)).into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_variable(&mut self, pair: &Pair<Rule>) -> Node<Variable> {
        Variable {
            ident: Ident {
                name: pair.as_str().into(),
            }
            .into_node(&pair),
        }
        .into_node(&pair)
    }
}
