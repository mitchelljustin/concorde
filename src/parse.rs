use std::num::ParseFloatError;

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::parse::Error::{IllegalLValue, RuleMismatch};
use crate::types::{
    Access, AnyNodeVariant, Array, Assignment, Binary, Block, Boolean, Break, Call,
    ClassDefinition, Continue, Expression, ForIn, Ident, IfElse, Index, LValue, Literal,
    MethodDefinition, Nil, Node, NodeMeta, NodeVariant, Number, Operator, Parameter, Path, Program,
    Statement, StringLit, TopError, Unary, Use, Variable, WhileLoop,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("pest error: {0}")]
    Pest(#[from] pest::error::Error<Rule>),
    #[error("parse float error: {0}")]
    ParseFloat(#[from] ParseFloatError),
    #[error("illegal lvalue for assignment: {lvalue}")]
    IllegalLValue { lvalue: NodeMeta },
    #[error("rule mismatch: expected '{expected:?}', got '{actual:?}'")]
    RuleMismatch { expected: Rule, actual: Rule },
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
            Rule::for_in => {
                let [binding, iterable, body] = pair.clone().into_inner().next_chunk().unwrap();
                let binding = self.parse_variable(binding)?;
                let iterable = self.parse_expression(iterable)?;
                let body = self.parse_block(body)?;
                Ok(Statement::ForIn(
                    ForIn {
                        binding,
                        iterable,
                        body,
                    }
                    .into_node(&pair),
                )
                .into_node(&pair))
            }
            Rule::while_loop => {
                let [condition, body] = pair.clone().into_inner().next_chunk().unwrap();
                let condition = self.parse_expression(condition)?;
                let body = self.parse_block(body)?;
                Ok(
                    Statement::WhileLoop(WhileLoop { condition, body }.into_node(&pair))
                        .into_node(&pair),
                )
            }
            Rule::loop_break => Ok(Statement::Break(Break {}.into_node(&pair)).into_node(&pair)),
            Rule::loop_continue => {
                Ok(Statement::Continue(Continue {}.into_node(&pair)).into_node(&pair))
            }
            Rule::expr => {
                Ok(Statement::Expression(self.parse_expression(pair.clone())?).into_node(&pair))
            }
            Rule::use_stmt => {
                let path = pair.clone().into_inner().next().unwrap();
                let mut components = self.parse_list(path.clone(), Self::parse_variable)?;
                let path = Path { components }.into_node(&path);
                Ok(Statement::Use(Use { path }.into_node(&pair)).into_node(&pair))
            }
            rule => unreachable!("{:?}", rule),
        }
    }

    fn parse_method_def(&mut self, pair: Pair<Rule>) -> Result<Node<MethodDefinition>> {
        let mut inner = pair.clone().into_inner();
        let first = inner.next().unwrap();
        let is_class_method = first.as_rule() == Rule::class_method_spec;
        let name = if is_class_method {
            inner.next().unwrap()
        } else {
            first
        };
        let [param_list, body] = inner.next_chunk().unwrap();
        let name = self.parse_ident(&name)?;
        let parameters = self.parse_list(param_list, |s, pair| {
            Ok(Parameter {
                name: s.parse_ident(&pair.clone().into_inner().next().unwrap())?,
            }
            .into_node(&pair))
        })?;
        let body = self.parse_block(body)?;
        Ok(MethodDefinition {
            is_class_method,
            name,
            body,
            parameters,
        }
        .into_node(&pair))
    }

    fn parse_operator(&mut self, pair: &Pair<Rule>) -> Node<Operator> {
        match pair.as_rule() {
            Rule::op_eq => Operator::EqualEqual,
            Rule::op_neq => Operator::NotEqual,
            Rule::op_gt => Operator::Greater,
            Rule::op_gte => Operator::GreaterEqual,
            Rule::op_lt => Operator::Less,
            Rule::op_lte => Operator::LessEqual,
            Rule::op_minus => Operator::Minus,
            Rule::op_plus => Operator::Plus,
            Rule::op_star => Operator::Star,
            Rule::op_slash => Operator::Slash,
            Rule::op_not => Operator::LogicalNot,
            Rule::op_or => Operator::LogicalOr,
            Rule::op_and => Operator::LogicalAnd,
            rule => unreachable!("{:?}", rule),
        }
        .into_node(pair)
    }

    fn parse_left_assoc(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        let mut inner = pair.clone().into_inner();
        let mut lhs = self.parse_expression(inner.next().unwrap())?;
        for [op, rhs] in inner.array_chunks() {
            let rhs = self.parse_expression(rhs)?;
            let op = self.parse_operator(&op);
            lhs = Expression::Binary(
                Binary {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    op,
                }
                .into_node(&pair),
            )
            .into_node(&pair);
        }
        Ok(lhs)
    }

    fn parse_expression(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        match pair.as_rule() {
            Rule::expr | Rule::primary | Rule::grouping => {
                self.parse_expression(pair.into_inner().next().unwrap())
            }
            Rule::logical_or
            | Rule::logical_and
            | Rule::equality
            | Rule::comparison
            | Rule::term
            | Rule::factor => self.parse_left_assoc(pair),
            Rule::logical_not | Rule::unary_minus => {
                let mut inner = pair.clone().into_inner().rev();
                let mut expr = self.parse_expression(inner.next().unwrap())?;
                for operator in inner {
                    expr = Expression::Unary(
                        Unary {
                            rhs: Box::new(expr),
                            op: self.parse_operator(&operator),
                        }
                        .into_node(&pair),
                    )
                    .into_node(&pair);
                }
                Ok(expr)
            }
            Rule::index => self.parse_index(pair),
            Rule::access => self.parse_access(pair),
            Rule::call => self.parse_call(&pair),
            Rule::literal => {
                let literal = self.parse_literal(pair.clone())?;
                Ok(Expression::Literal(literal).into_node(&pair))
            }
            Rule::path => {
                let mut components = self.parse_list(pair.clone(), Self::parse_variable)?;
                if components.len() == 1 {
                    Ok(Expression::Variable(components.pop().unwrap()).into_node(&pair))
                } else {
                    Ok(Expression::Path(Path { components }.into_node(&pair)).into_node(&pair))
                }
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
            rule => unreachable!("{:?}", rule),
        }
    }

    fn parse_call(&mut self, pair: &Pair<Rule>) -> Result<Node<Expression>> {
        self.assert_rule(&pair, Rule::call)?;
        let mut inner = pair.clone().into_inner();
        let mut expr = self.parse_expression(inner.next().unwrap())?;
        for arg_list in inner {
            let arguments = if let Some(expr_list) = arg_list.into_inner().next() {
                self.parse_list(expr_list, Self::parse_expression)?
            } else {
                Vec::new()
            };
            expr = Expression::Call(
                Call {
                    target: Box::new(expr),
                    arguments,
                }
                .into_node(&pair),
            )
            .into_node(&pair)
        }
        Ok(expr)
    }

    fn parse_index(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        self.assert_rule(&pair, Rule::index)?;
        let mut inner = pair.clone().into_inner();
        let mut expr = self.parse_expression(inner.next().unwrap())?;
        for index in inner {
            let index = self.parse_expression(index)?;
            expr = Expression::Index(
                Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                }
                .into_node(&pair),
            )
            .into_node(&pair);
        }
        Ok(expr)
    }

    fn parse_access(&mut self, pair: Pair<Rule>) -> Result<Node<Expression>> {
        self.assert_rule(&pair, Rule::access)?;
        let mut inner = pair.clone().into_inner();
        let mut expr = self.parse_expression(inner.next().unwrap())?;
        for member in inner {
            let member = self.parse_expression(member)?;
            expr = Expression::Access(
                Access {
                    target: Box::new(expr),
                    member: Box::new(member),
                }
                .into_node(&pair),
            )
            .into_node(&pair);
        }
        Ok(expr)
    }

    fn parse_literal(&mut self, pair: Pair<Rule>) -> Result<Node<Literal>> {
        self.assert_rule(&pair, Rule::literal)?;
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
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
            Rule::string => Ok(Literal::StringLit(
                StringLit {
                    value: pair.clone().into_inner().next().unwrap().as_str().into(),
                }
                .into_node(&pair),
            )
            .into_node(&pair)),
            Rule::array => {
                let expr_list = pair.clone().into_inner().next().unwrap();
                let elements = self.parse_list(expr_list, Self::parse_expression)?;
                Ok(Literal::Array(Array { elements }.into_node(&pair)).into_node(&pair))
            }
            rule => unreachable!("{:?}", rule),
        }
    }

    fn parse_lvalue(&mut self, pair: Pair<Rule>) -> Result<Node<LValue>> {
        self.assert_rule(&pair, Rule::lvalue)?;
        let pair = pair.into_inner().next().unwrap();
        self.assert_rule(&pair, Rule::index)?;
        let lvalue = self.parse_index(pair.clone())?;
        match lvalue.v {
            Expression::Index(index) => Ok(LValue::Index(index).into_node(&pair)),
            Expression::Access(access) => Ok(LValue::Access(access).into_node(&pair)),
            Expression::Variable(var) => Ok(LValue::Variable(var).into_node(&pair)),
            _ => {
                return Err(IllegalLValue {
                    lvalue: lvalue.meta,
                })
            }
        }
    }

    fn parse_variable(&mut self, pair: Pair<Rule>) -> Result<Node<Variable>> {
        self.assert_rule(&pair, Rule::variable)?;
        Ok(Variable {
            ident: self.parse_ident(&pair)?,
        }
        .into_node(&pair))
    }

    fn parse_ident(&mut self, pair: &Pair<Rule>) -> Result<Node<Ident>> {
        Ok(Ident {
            name: pair.as_str().into(),
        }
        .into_node(&pair))
    }

    fn assert_rule(&self, pair: &Pair<Rule>, expected: Rule) -> Result<()> {
        let actual = pair.as_rule();
        if actual != expected {
            return Err(RuleMismatch { expected, actual });
        }
        Ok(())
    }
}
