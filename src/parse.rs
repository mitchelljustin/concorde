use std::fs;
use std::num::ParseFloatError;

use pest::iterators::{Pair, Pairs};
use pest::{Parser, RuleType};
use pest_derive::Parser;

use crate::parse::Error::{ClassHasTwoInitializers, IllegalBinding, IllegalLValue, RuleMismatch};
use crate::runtime::builtin;
use crate::types::{
    Access, Array, Assignment, Binary, Binding, Block, Boolean, Break, Call, ClassDefinition,
    Closure, Continue, Dictionary, Expression, ForIn, Ident, IfElse, Index, LValue, Literal,
    MethodDefinition, Nil, Node, NodeMeta, NodeVariant, Number, Operator, Parameter, Path, Program,
    Return, Statement, StringLit, TopError, Tuple, Unary, Use, Variable, WhileLoop,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("pest error: {0}")]
    Pest(#[from] Box<pest::error::Error<Rule>>),
    #[error("parse float error: {0}")]
    ParseFloat(#[from] ParseFloatError),
    #[error("illegal lvalue for assignment: {lvalue}")]
    IllegalLValue { lvalue: NodeMeta },
    #[error("rule mismatch: expected '{expected:?}', got '{actual:?}'")]
    RuleMismatch { expected: Rule, actual: Rule },
    #[error("class cannot both have fields and an initializer method: '{class}'")]
    ClassHasTwoInitializers { class: String },
    #[error("syntax error, illegal multi-variable binding expression: '{node}'")]
    IllegalBinding { node: NodeMeta },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Parser)]
#[grammar = "concorde.pest"]
struct ConcordeParser;

trait PairsExt<'a, R: RuleType> {
    fn next_if_rule(&mut self, rule: R) -> Option<Pair<'a, R>>;
}

impl<'a> PairsExt<'a, Rule> for Pairs<'a, Rule> {
    fn next_if_rule(&mut self, rule: Rule) -> Option<Pair<'a, Rule>> {
        self.peek()
            .and_then(|pair| (pair.as_rule() == rule).then(|| self.next()))
            .flatten()
    }
}

pub fn parse_file(path: impl AsRef<std::path::Path>) -> Result<Node<Program>, TopError> {
    let source = fs::read_to_string(path)?;
    parse_source(&source)
}

pub fn pretty_print_pair<R: RuleType>(pair: Pair<R>) {
    fn pp<R: RuleType>(mut pair: Pair<R>, indent_level: usize) {
        let source = pair.as_str();
        let first_rule = pair.as_rule();
        let indent = ". ".repeat(indent_level);
        let mut rules = vec![first_rule];
        let mut inner: Vec<Pair<R>>;
        loop {
            inner = pair.into_inner().collect();
            let [single_inner_pair] = &inner[..] else {
                break;
            };
            if single_inner_pair.as_str() != source {
                break;
            }
            rules.push(single_inner_pair.as_rule());
            pair = single_inner_pair.clone();
        }
        let rule_prefix = match &rules[..] {
            [] => unreachable!(),
            [rule] => format!("{rule:?}"),
            [first_rule, second_rule] => format!("{first_rule:?} > {second_rule:?}"),
            [first_rule, .., last_rule] => format!("{first_rule:?} > .. > {last_rule:?}"),
        };
        let source_no_newlines = source.replace('\n', "\\n");
        println!("{indent}{rule_prefix} '{source_no_newlines}'");
        for sub_pair in inner {
            pp(sub_pair, indent_level + 1);
        }
    }
    pp(pair, 0);
}

pub fn parse_source(source: &str) -> Result<Node<Program>, TopError> {
    let pair = ConcordeParser::parse(Rule::program, source)
        .map_err(|err| Error::Pest(Box::new(err)))?
        .next()
        .unwrap();
    // pretty_print_pair(pair.clone());
    let body = parse_block(pair.clone().into_inner().next().unwrap())?;
    Ok(Program { body }.into_node(&pair))
}

fn parse_block(pair: Pair<Rule>) -> Result<Node<Block>> {
    let statements = parse_list(pair.clone(), parse_statement)?;
    Ok(Block { statements }.into_node(&pair))
}

fn parse_list<T: NodeVariant>(
    pair: Pair<Rule>,
    parse_one: fn(Pair<Rule>) -> Result<Node<T>>,
) -> Result<Vec<Node<T>>> {
    pair.into_inner().map(parse_one).collect()
}

fn parse_statement(pair: Pair<Rule>) -> Result<Node<Statement>> {
    match pair.as_rule() {
        Rule::stmt => parse_statement(pair.into_inner().next().unwrap()),
        Rule::assignment => {
            let [target, op, value] = pair.clone().into_inner().next_chunk().unwrap();
            let target = parse_lvalue(target)?;
            let op = parse_operator(&op);
            let value = parse_expression(value)?;
            Ok(
                Statement::Assignment(Assignment { target, op, value }.into_node(&pair))
                    .into_node(&pair),
            )
        }
        Rule::class_def => {
            let mut inner = pair.clone().into_inner();
            let name_pair = inner.next().unwrap();
            let name = parse_ident(&name_pair)?;
            let param_list = inner.next_if_rule(Rule::param_list);
            let body = inner.next().unwrap();
            let mut body = parse_block(body)?;
            let fields;
            if let Some(param_list) = param_list {
                fields = parse_list(param_list.clone(), parse_param)?;
                let has_init_method = body.v.statements.iter().any(|stmt| {
                    let Statement::MethodDefinition(method_def) = &stmt.v else {
                        return false;
                    };
                    method_def.v.name.v.name == builtin::method::init
                });
                if has_init_method {
                    return Err(ClassHasTwoInitializers {
                        class: name.v.name.clone(),
                    });
                }

                let init_source = fields
                    .iter()
                    .map(|field| {
                        let name = field.v.name.v.name.clone();
                        let value = field
                            .v
                            .default
                            .as_ref()
                            .map(|default| default.meta.source.clone())
                            .unwrap_or(name.clone());
                        format!("self.{name} = {value}\n")
                    })
                    .collect::<Vec<String>>()
                    .join("");
                let block = ConcordeParser::parse(Rule::stmts, &init_source)
                    .unwrap()
                    .next()
                    .unwrap();
                let init_body = parse_block(block)?;
                let parameters = fields
                    .iter()
                    .filter(|field| field.v.default.is_none())
                    .cloned()
                    .collect();
                body.v.statements.push(
                    Statement::MethodDefinition(
                        MethodDefinition {
                            name: Ident {
                                name: builtin::method::init.into(),
                            }
                            .into_node(&name_pair),
                            is_class_method: false,
                            parameters,
                            body: init_body,
                        }
                        .into_node(&param_list),
                    )
                    .into_node(&param_list),
                )
            } else {
                fields = Vec::new();
            }
            Ok(
                Statement::ClassDefinition(ClassDefinition { name, fields, body }.into_node(&pair))
                    .into_node(&pair),
            )
        }
        Rule::method_def => {
            Ok(Statement::MethodDefinition(parse_method_def(pair.clone())?).into_node(&pair))
        }
        Rule::for_in => {
            let [binding, iterable, body] = pair.clone().into_inner().next_chunk().unwrap();
            let binding = parse_binding(binding)?;
            let iterable = parse_expression(iterable)?;
            let body = parse_stmts_or_short_stmt(body)?;
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
            let condition = parse_expression(condition)?;
            let body = parse_stmts_or_short_stmt(body)?;
            Ok(
                Statement::WhileLoop(WhileLoop { condition, body }.into_node(&pair))
                    .into_node(&pair),
            )
        }
        Rule::loop_break => Ok(Statement::Break(Break {}.into_node(&pair)).into_node(&pair)),
        Rule::loop_continue => {
            Ok(Statement::Continue(Continue {}.into_node(&pair)).into_node(&pair))
        }
        Rule::return_stmt => {
            let retval = pair
                .clone()
                .into_inner()
                .next()
                .map(|pair| parse_expression(pair))
                .transpose()?;
            Ok(Statement::Return(Return { retval }.into_node(&pair)).into_node(&pair))
        }
        Rule::expr => Ok(Statement::Expression(parse_expression(pair.clone())?).into_node(&pair)),
        Rule::use_stmt => {
            let path = pair.clone().into_inner().next().unwrap();
            let components = parse_list(path.clone(), parse_variable)?;
            let path = Path { components }.into_node(&path);
            Ok(Statement::Use(Use { path }.into_node(&pair)).into_node(&pair))
        }
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_binding(pair: Pair<Rule>) -> Result<Vec<Node<Variable>>, Error> {
    parse_list(pair, parse_variable)
}

fn parse_param(pair: Pair<Rule>) -> Result<Node<Parameter>> {
    let mut inner = pair.clone().into_inner();
    let name = inner.next_if_rule(Rule::ident).unwrap();
    let default = inner.next_if_rule(Rule::expr);
    Ok(Parameter {
        name: parse_ident(&name)?,
        default: default.map(|pair| parse_expression(pair)).transpose()?,
    }
    .into_node(&pair))
}

fn parse_method_def(pair: Pair<Rule>) -> Result<Node<MethodDefinition>> {
    let mut inner = pair.clone().into_inner();
    let is_class_method = inner.next_if_rule(Rule::class_method_spec).is_some();
    let [name, param_list, body] = inner.next_chunk().unwrap();
    let name = parse_ident(&name)?;
    let parameters = parse_list(param_list, parse_param)?;
    let body = parse_stmts_or_short_stmt(body)?;
    Ok(MethodDefinition {
        is_class_method,
        name,
        body,
        parameters,
    }
    .into_node(&pair))
}

fn parse_stmts_or_short_stmt(body: Pair<Rule>) -> Result<Node<Block>, Error> {
    Ok(match body.as_rule() {
        Rule::stmts => parse_block(body)?,
        _ => parse_short_stmt_into_block(body)?,
    })
}

fn parse_short_stmt_into_block(body: Pair<Rule>) -> Result<Node<Block>> {
    Ok(Block {
        statements: vec![parse_statement(body.clone())?],
    }
    .into_node(&body))
}

fn parse_operator(pair: &Pair<Rule>) -> Node<Operator> {
    match pair.as_rule() {
        Rule::op_eq => Operator::Equal,
        Rule::op_eq_eq => Operator::EqualEqual,
        Rule::op_neq => Operator::NotEqual,
        Rule::op_gt => Operator::Greater,
        Rule::op_gte => Operator::GreaterEqual,
        Rule::op_lt => Operator::Less,
        Rule::op_lte => Operator::LessEqual,
        Rule::op_minus => Operator::Minus,
        Rule::op_plus => Operator::Plus,
        Rule::op_star => Operator::Star,
        Rule::op_percent => Operator::Percent,
        Rule::op_slash => Operator::Slash,
        Rule::op_minus_eq => Operator::MinusEqual,
        Rule::op_plus_eq => Operator::PlusEqual,
        Rule::op_star_eq => Operator::StarEqual,
        Rule::op_slash_eq => Operator::SlashEqual,
        Rule::op_not => Operator::LogicalNot,
        Rule::op_or => Operator::LogicalOr,
        Rule::op_and => Operator::LogicalAnd,
        rule => unreachable!("{:?}", rule),
    }
    .into_node(pair)
}

fn parse_left_assoc(pair: Pair<Rule>) -> Result<Node<Expression>> {
    let mut inner = pair.clone().into_inner();
    let mut lhs = parse_expression(inner.next().unwrap())?;
    for [op, rhs] in inner.array_chunks() {
        let rhs = parse_expression(rhs)?;
        let op = parse_operator(&op);
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

fn parse_expression(pair: Pair<Rule>) -> Result<Node<Expression>> {
    match pair.as_rule() {
        Rule::expr | Rule::primary | Rule::grouping => {
            parse_expression(pair.into_inner().next().unwrap())
        }
        Rule::binding => {
            let pairs = pair.clone().into_inner().collect::<Vec<_>>();
            if pairs.len() > 1 {
                return Err(IllegalBinding {
                    node: (&pair).into(),
                });
            }
            let var_pair = pairs[0].clone();
            Ok(Expression::Variable(parse_variable(var_pair)?).into_node(&pair))
        }
        Rule::logical_or
        | Rule::logical_and
        | Rule::equality
        | Rule::comparison
        | Rule::term
        | Rule::factor => parse_left_assoc(pair),
        Rule::logical_not | Rule::unary_minus => {
            let mut inner = pair.clone().into_inner().rev();
            let mut expr = parse_expression(inner.next().unwrap())?;
            for operator in inner {
                expr = Expression::Unary(
                    Unary {
                        rhs: Box::new(expr),
                        op: parse_operator(&operator),
                    }
                    .into_node(&pair),
                )
                .into_node(&pair);
            }
            Ok(expr)
        }
        Rule::closure => parse_closure(pair),
        Rule::index => parse_index(pair),
        Rule::access => parse_access(pair),
        Rule::call => parse_call(&pair),
        Rule::literal => {
            let literal = parse_literal(pair.clone())?;
            Ok(Expression::Literal(literal).into_node(&pair))
        }
        Rule::path => {
            let mut components = parse_list(pair.clone(), parse_variable)?;
            if components.len() == 1 {
                Ok(Expression::Variable(components.pop().unwrap()).into_node(&pair))
            } else {
                Ok(Expression::Path(Path { components }.into_node(&pair)).into_node(&pair))
            }
        }
        Rule::if_else => {
            let mut inner = pair.clone().into_inner();
            let [condition, then_body] = inner.next_chunk().unwrap();
            let condition = Box::new(parse_expression(condition)?);
            let then_body = parse_stmts_or_short_stmt(then_body)?;
            let else_body = inner
                .next()
                .map(|else_body| parse_stmts_or_short_stmt(else_body))
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

fn parse_call(pair: &Pair<Rule>) -> Result<Node<Expression>> {
    assert_rule(pair, Rule::call)?;
    let mut inner = pair.clone().into_inner();
    let mut expr = parse_expression(inner.next().unwrap())?;
    for arg_list in inner {
        let arguments = if let Some(expr_list) = arg_list.into_inner().next() {
            parse_list(expr_list, parse_expression)?
        } else {
            Vec::new()
        };
        expr = Expression::Call(
            Call {
                target: Box::new(expr),
                arguments,
            }
            .into_node(pair),
        )
        .into_node(pair)
    }
    Ok(expr)
}

fn parse_index(pair: Pair<Rule>) -> Result<Node<Expression>> {
    assert_rule(&pair, Rule::index)?;
    let mut inner = pair.clone().into_inner();
    let mut expr = parse_expression(inner.next().unwrap())?;
    for index in inner {
        let index = parse_expression(index)?;
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

fn parse_access(pair: Pair<Rule>) -> Result<Node<Expression>> {
    assert_rule(&pair, Rule::access)?;
    let mut inner = pair.clone().into_inner();
    let mut expr = parse_expression(inner.next().unwrap())?;
    for member in inner {
        let member = parse_expression(member)?;
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

fn parse_literal(pair: Pair<Rule>) -> Result<Node<Literal>> {
    assert_rule(&pair, Rule::literal)?;
    let pair = pair.into_inner().next().unwrap();
    let rule = pair.as_rule();
    match rule {
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
            let elements = pair
                .clone()
                .into_inner()
                .next()
                .map(|expr_list| parse_list(expr_list, parse_expression))
                .transpose()?
                .unwrap_or_default();
            Ok(Literal::Array(Array { elements }.into_node(&pair)).into_node(&pair))
        }
        Rule::dict => {
            let entries: Vec<_> = pair
                .clone()
                .into_inner()
                .array_chunks()
                .map(|[key, value]| {
                    let key = parse_ident(&key)?;
                    let value = parse_expression(value)?;
                    Ok::<_, Error>((key, value))
                })
                .try_collect()?;
            Ok(Literal::Dictionary(Dictionary { entries }.into_node(&pair)).into_node(&pair))
        }
        Rule::tuple => {
            let items = parse_list(pair.clone(), parse_expression)?;
            Ok(Literal::Tuple(Tuple { items }.into_node(&pair)).into_node(&pair))
        }
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_lvalue(pair: Pair<Rule>) -> Result<Node<LValue>> {
    assert_rule(&pair, Rule::lvalue)?;
    let pair = pair.into_inner().next().unwrap();
    match pair.as_rule() {
        Rule::tuple => {
            let variables = parse_list(pair.clone(), parse_expression)?
                .into_iter()
                .map(|expr| {
                    let Expression::Variable(var) = expr.v else {
                        return Err(IllegalBinding {
                            node: (&pair).into(),
                        });
                    };
                    Ok(var)
                })
                .try_collect()?;

            Ok(LValue::Binding(Binding { variables }.into_node(&pair)).into_node(&pair))
        }
        Rule::index => {
            let lvalue = parse_index(pair.clone())?;
            match lvalue.v {
                Expression::Index(index) => Ok(LValue::Index(index).into_node(&pair)),
                Expression::Access(access) => Ok(LValue::Access(access).into_node(&pair)),
                Expression::Variable(var) => Ok(LValue::Binding(
                    Binding {
                        variables: vec![var],
                    }
                    .into_node(&pair),
                )
                .into_node(&pair)),
                _ => Err(IllegalLValue {
                    lvalue: lvalue.meta,
                }),
            }
        }
        _ => unreachable!(),
    }
}

fn parse_variable(pair: Pair<Rule>) -> Result<Node<Variable>> {
    assert_rule(&pair, Rule::variable)?;
    Ok(Variable {
        ident: parse_ident(&pair)?,
    }
    .into_node(&pair))
}

fn parse_ident(pair: &Pair<Rule>) -> Result<Node<Ident>> {
    Ok(Ident {
        name: pair.as_str().into(),
    }
    .into_node(pair))
}

fn assert_rule(pair: &Pair<Rule>, expected: Rule) -> Result<()> {
    let actual = pair.as_rule();
    if actual != expected {
        return Err(RuleMismatch { expected, actual });
    }
    Ok(())
}

fn parse_closure(pair: Pair<Rule>) -> Result<Node<Expression>> {
    let [binding, body] = pair.clone().into_inner().next_chunk().unwrap();
    let binding = parse_list(binding, parse_variable)?;
    let body = parse_stmts_or_short_stmt(body)?;
    Ok(Expression::Closure(Closure { binding, body }.into_node(&pair)).into_node(&pair))
}
