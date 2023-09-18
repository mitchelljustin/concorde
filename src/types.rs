use std::fmt::{Debug, Display};
use std::io;
use std::ops::Deref;
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

impl<V: NodeVariant> Deref for Node<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}

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

    Program {
        body: Node<Block>,
    }
    IfElse {
        condition: Node<Expression>,
        then_body: Node<Block>,
        else_body: Option<Node<Block>>,
    }
    ForLoop {
        iterator: Node<Ident>,
        target: Node<Expression>,
        body: Node<Block>,
    }
    Break {}
    Continue {}
    Nil {}
    WhileLoop {
        condition: Node<Expression>,
        body: Node<Block>,
    }
    Access {
        target: Box<Node<Expression>>,
        member: Box<Node<Expression>>,
    }
    Assignment {
        target: Node<LValue>,
        value: Node<Expression>,
    }
    Call {
        target: Box<Node<Expression>>,
        arguments: Vec<Node<Expression>>,
    }
    Variable {
        ident: Node<Ident>,
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
        name: Node<Ident>,
        parameters: Vec<Node<Parameter>>,
        body: Node<Block>,
    }
}

define_collector_enums! {
    Statement {
        IfElse,
        ForLoop,
        WhileLoop,
        Break,
        Continue,
        Block,
        Assignment,
        Expression,
        MethodDefinition,
        ClassDefinition,
    }
    Expression {
        Access,
        Call,
        Literal,
        Variable,
    }
    Literal {
        String,
        Number,
        Boolean,
        Nil,
    }
    LValue {
        Variable,
        Access,
    }
}

#[derive(Debug, Clone)]
pub enum Primitive {
    String(RcString),
    Number(f64),
    Boolean(bool),
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
