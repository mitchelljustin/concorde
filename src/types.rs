use std::fmt::{Debug, Display, Formatter};
use std::io;

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

#[derive(Debug, Clone)]
pub struct NodeMeta {
    pub source: String,
    pub rule: Rule,
    pub line_col: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct MaybeNodeMeta(Option<NodeMeta>);

impl Display for MaybeNodeMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(node) => write!(f, "{node}"),
            None => write!(f, "(no AST)"),
        }
    }
}

impl From<NodeMeta> for MaybeNodeMeta {
    fn from(value: NodeMeta) -> Self {
        Some(value).into()
    }
}

impl From<Option<NodeMeta>> for MaybeNodeMeta {
    fn from(value: Option<NodeMeta>) -> Self {
        Self(value)
    }
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
pub struct Node<Variant: NodeVariant> {
    pub meta: NodeMeta,
    pub v: Variant,
}

impl From<&Pair<'_, Rule>> for NodeMeta {
    fn from(pair: &Pair<Rule>) -> Self {
        Self {
            source: pair.as_str().to_string(),
            rule: pair.as_rule(),
            line_col: pair.line_col(),
        }
    }
}

pub trait NodeVariant: Sized + Debug + Clone {
    fn into_node(self, pair: &Pair<Rule>) -> Node<Self> {
        Node {
            meta: pair.into(),
            v: self,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Operator {
    Equal,
    EqualEqual,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Plus,
    Minus,
    Star,
    Percent,
    Slash,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    LogicalAnd,
    LogicalOr,
    LogicalNot,
}

impl NodeVariant for Operator {}

define_node_types! {
    Ident {
        name: String,
    }
    Number {
        value: f64,
    }
    Boolean {
        value: bool,
    }
    StringLit {
        value: String,
    }
    Array {
        elements: Vec<Node<Expression>>,
    }
    Tuple {
        items: Vec<Node<Expression>>,
    }
    Dictionary {
        entries: Vec<(Node<Ident>, Node<Expression>)>,
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
        binding: Vec<Node<Variable>>,
        iterable: Node<Expression>,
        body: Node<Block>,
    }
    Break {}
    Continue {}
    Nil {}
    Return {
        retval: Option<Node<Expression>>,
    }
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
    Closure {
        binding: Vec<Node<Variable>>,
        body: Node<Block>,
    }
    Assignment {
        target: Node<LValue>,
        op: Node<Operator>,
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
    Binding {
        variables: Vec<Node<Variable>>,
    }
    Variable {
        ident: Node<Ident>,
    }
    Path {
        components: Vec<Node<Variable>>,
    }
    Use {
        path: Node<Path>,
    }
    Block {
        statements: Vec<Node<Statement>>,
    }
    ClassDefinition {
        name: Node<Ident>,
        fields: Vec<Node<Parameter>>,
        body: Node<Block>,
    }
    Parameter {
        name: Node<Ident>,
        default: Option<Node<Expression>>,
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
        Continue,
        Return,
        Assignment,
        Expression,
        MethodDefinition,
        ClassDefinition,
        Use,
    }
    Expression {
        Index,
        Access,
        Call,
        Literal,
        Path,
        IfElse,
        Binary,
        Unary,
        Closure,
        Variable,
    }
    Literal {
        Array,
        Tuple,
        Dictionary,
        StringLit,
        Number,
        Boolean,
        Nil,
    }
    LValue {
        Access,
        Index,
        Binding,
    }
}

macro define_node_types(
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
