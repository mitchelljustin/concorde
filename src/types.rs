use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::rc::Rc;

use pest::iterators::Pair;

use crate::parse::Rule;
use crate::{parse, runtime};

#[derive(thiserror::Error, Debug)]
pub enum TopError {
    #[error("runtime error: {0}")]
    Runtime(#[from] runtime::Error),
    #[error("parse error: {0}")]
    Parse(#[from] parse::Error),

    #[error("I/O error: {0}")]
    IO(#[from] io::Error),
}

pub type RcString = Rc<str>;

#[derive(Debug, Clone)]
pub struct NodeMeta {
    pub source: std::string::String,
    pub rule: Rule,
    pub line_col: (usize, usize),
}

impl Display for NodeMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let NodeMeta {
            source,
            rule,
            line_col: (line, col),
        } = self;
        write!(f, "'{source}' at {line}:{col} ({rule:?})")
    }
}

#[derive(Debug, Clone)]
pub struct Node<Variant: NodeVariant = AnyNodeVariant> {
    pub meta: NodeMeta,
    pub v: Variant,
}

pub trait NodeVariant: Sized + Debug + Clone {
    fn into_node(self, pair: &Pair<Rule>) -> Node<Self> {
        Node {
            meta: NodeMeta {
                source: pair.as_str().to_string(),
                rule: pair.as_rule(),
                line_col: pair.line_col(),
            },
            v: self,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Operator {
    EqualEqual,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Plus,
    Minus,
    Star,
    Slash,
    LogicalAnd,
    LogicalOr,
    LogicalNot,
}

impl NodeVariant for Operator {}

define_node_types! {
    [AnyNodeVariant]

    Ident {
        name: RcString,
    }
    Number {
        value: f64,
    }
    Boolean {
        value: bool,
    }
    String {
        value: RcString,
    }
    Array {
        elements: Vec<Node<Expression>>,
    }

    Program {
        body: Node<Block>,
    }
    IfElse {
        condition: Box<Node<Expression>>,
        then_body: Node<Block>,
        else_body: Option<Node<Block>>,
    }
    ForIn {
        binding: Node<Variable>,
        iterator: Node<Expression>,
        body: Node<Block>,
    }
    Break {}
    Next {}
    Nil {}
    WhileLoop {
        condition: Node<Expression>,
        body: Node<Block>,
    }
    Binary {
        lhs: Box<Node<Expression>>,
        op: Node<Operator>,
        rhs: Box<Node<Expression>>,
    }
    Unary {
        op: Node<Operator>,
        rhs: Box<Node<Expression>>,
    }
    Access {
        target: Box<Node<Expression>>,
        member: Box<Node<Expression>>,
    }
    Assignment {
        target: Node<LValue>,
        value: Node<Expression>,
    }
    Index {
        target: Box<Node<Expression>>,
        index: Box<Node<Expression>>,
    }
    Call {
        target: Box<Node<Expression>>,
        arguments: Vec<Node<Expression>>,
    }
    Variable {
        ident: Node<Ident>,
    }
    Path {
        components: Vec<Node<Variable>>,
    }
    Block {
        statements: Vec<Node<Statement>>,
    }
    ClassDefinition {
        name: Node<Ident>,
        // fields: Vec<RcString>,
        body: Vec<Node<Statement>>,
    }
    Parameter {
        name: Node<Ident>,
    }
    MethodDefinition {
        is_class_method: bool,
        name: Node<Ident>,
        parameters: Vec<Node<Parameter>>,
        body: Node<Block>,
    }
}

define_collector_enums! {
    Statement {
        ForIn,
        WhileLoop,
        Break,
        Next,
        Assignment,
        Expression,
        MethodDefinition,
        ClassDefinition,
    }
    Expression {
        Index,
        Access,
        Call,
        Literal,
        Variable,
        Path,
        IfElse,
        Binary,
        Unary,
    }
    Literal {
        Array,
        String,
        Number,
        Boolean,
        Nil,
    }
    LValue {
        Variable,
        Access,
        Index,
    }
}

macro define_node_types(
    [$any_node_name:ident]
$(
    $name:ident {
        $(
            $field:ident : $ty:ty,
        )*
    }
)+) {
    $(
        #[derive(Debug, Clone)]
        pub struct $name {
            $(
                pub $field : $ty,
            )*
        }

        impl NodeVariant for $name {}
    )+
    define_collector_enums! {
        $any_node_name {
        $(
            $name,
        )+
        }
    }
}

macro define_collector_enums(
    $(
        $collector_name:ident {
            $($variant:ident,)+
        }
    )+
) {
$(
        #[derive(Debug, Clone)]
        pub enum $collector_name {
            $(
                $variant(Node<$variant>),
            )+
        }

        impl NodeVariant for $collector_name {}
    )+
}
