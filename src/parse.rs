use std::num::ParseFloatError;

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::parse::Error::IllegalLValue;
use crate::types::{
    Access, AnyNodeVariant, Assignment, Block, Boolean, Call, ClassDefinition, Expression, Ident,
    IfElse, LValue, Literal, MethodDefinition, Nil, Node, NodeMeta, NodeVariant, Number, Parameter,
    Program, Statement, String as StringVariant, TopError, Variable,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("pest error: {0}")]
    Pest(#[from] pest::error::Error<Rule>),
    #[error("parse float error: {0}")]
    ParseFloat(#[from] ParseFloatError),
    #[error("illegal lvalue for assignment: {lvalue}")]
    IllegalLValue { lvalue: NodeMeta },
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
        let statements = self.parse_list(pair.clone(), Self::parse_statement)?;
        Ok(Block { statements }.into_node(&pair))
    }

    fn parse_list<T: NodeVariant>(
        &mut self,
        pair: Pair<Rule>,
        parse_one: fn(&mut Self, Pair<Rule>) -> Result<Node<T>>,
    ) -> Result<Vec<Node<T>>> {
        pair.into_inner()
            .map(|pair| parse_one(self, pair))
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
            Rule::class_def => {
                let [name, body] = pair.clone().into_inner().next_chunk().unwrap();
                let name = self.parse_ident(&name)?;
                let body = self.parse_list(body, Self::parse_statement)?;
                Ok(
                    Statement::ClassDefinition(ClassDefinition { name, body }.into_node(&pair))
                        .into_node(&pair),
                )
            }
            Rule::method_def => Ok(Statement::MethodDefinition(
                self.parse_method_def(pair.clone())?,
            )
            .into_node(&pair)),
            Rule::expr => {
                Ok(Statement::Expression(self.parse_expression(pair.clone())?).into_node(&pair))
            }
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_method_def(&mut self, pair: Pair<Rule>) -> Result<Node<MethodDefinition>> {
        let [name, param_list, body] = pair.clone().into_inner().next_chunk().unwrap();
        let name = self.parse_ident(&name)?;
        let parameters = self.parse_list(param_list, |s, pair| {
            Ok(Parameter {
                name: s.parse_ident(&pair.clone().into_inner().next().unwrap())?,
            }
            .into_node(&pair))
        })?;
        let body = self.parse_block(body)?;
        Ok(MethodDefinition {
            name,
            body,
            parameters,
        }
        .into_node(&pair))
    }

    fn parse_expression(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::expr | Rule::primary | Rule::grouping => {
                self.parse_expression(pair.into_inner().next().unwrap())
            }
            Rule::access => self.parse_access(pair),
            Rule::call => {
                let mut inner = pair.clone().into_inner();
                let target = inner.next().unwrap();
                let Some(expr_list) = inner.next() else {
                    return self.parse_expression(target);
                };
                let arguments = self.parse_list(expr_list, Self::parse_expression)?;
                let target = Box::new(self.parse_expression(target)?);
                Ok(Expression::Call(Call { target, arguments }.into_node(&pair)).into_node(&pair))
            }
            Rule::literal => {
                let literal = self.parse_literal(pair.clone())?;
                Ok(Expression::Literal(literal).into_node(&pair))
            }
            Rule::variable => {
                Ok(Expression::Variable(self.parse_variable(&pair)?).into_node(&pair))
            }
            Rule::if_else => {
                let mut inner = pair.clone().into_inner();
                let [condition, then_body] = inner.next_chunk().unwrap();
                let condition = Box::new(self.parse_expression(condition)?);
                let then_body = self.parse_block(then_body)?;
                let else_body = inner
                    .next()
                    .map(|else_body| self.parse_block(else_body))
                    .transpose()?;
                Ok(Expression::IfElse(
                    IfElse {
                        condition,
                        then_body,
                        else_body,
                    }
                    .into_node(&pair),
                )
                .into_node(&pair))
            }
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_access(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
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

    fn parse_literal(&mut self, pair: Pair<Rule>) -> Result<Node<Literal>> {
        match pair.as_rule() {
            Rule::literal => self.parse_literal(pair.into_inner().next().unwrap()),
            Rule::nil => Ok(Literal::Nil(Nil {}.into_node(&pair)).into_node(&pair)),
            Rule::bool => Ok(Literal::Boolean(
                Boolean {
                    value: pair.as_str() == "true",
                }
                .into_node(&pair),
            )
            .into_node(&pair)),
            Rule::number => {
                let value: f64 = pair.as_str().parse()?;
                Ok(Literal::Number(Number { value }.into_node(&pair)).into_node(&pair))
            }
            Rule::string => Ok(Literal::String(
                StringVariant {
                    value: pair.clone().into_inner().next().unwrap().as_str().into(),
                }
                .into_node(&pair),
            )
            .into_node(&pair)),
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_lvalue(&mut self, pair: Pair<Rule>) -> Result<Node<LValue>> {
        match pair.as_rule() {
            Rule::lvalue => self.parse_lvalue(pair.into_inner().next().unwrap()),
            Rule::access => {
                let lvalue = self.parse_access(pair.clone())?;
                match lvalue.v {
                    Expression::Access(access) => Ok(LValue::Access(access).into_node(&pair)),
                    Expression::Variable(var) => Ok(LValue::Variable(var).into_node(&pair)),
                    _ => {
                        return Err(IllegalLValue {
                            lvalue: lvalue.meta,
                        })
                    }
                }
            }
            rule => unreachable!("{rule:?}"),
        }
    }

    fn parse_variable(&mut self, pair: &Pair<Rule>) -> Result<Node<Variable>> {
        Ok(Variable {
            ident: self.parse_ident(pair)?,
        }
        .into_node(&pair))
    }

    fn parse_ident(&mut self, pair: &Pair<Rule>) -> Result<Node<Ident>> {
        Ok(Ident {
            name: pair.as_str().into(),
        }
        .into_node(&pair))
    }
}
