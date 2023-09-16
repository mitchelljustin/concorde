use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::rc::{Rc, Weak};

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
pub struct Node<Variant: NodeVariant = AnyNodeVariant> {
    pub(crate) source: RcString,
    pub(crate) line_col: (usize, usize),
    pub(crate) v: Variant,
}

pub trait NodeVariant: Sized + Debug + Clone {
    fn into_node(self, pair: &Pair<Rule>) -> Node<Self> {
        Node {
            source: pair.as_str().into(),
            line_col: pair.line_col(),
            v: self,
        }
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
    WhileLoop {
        condition: Node<Expression>,
        body: Node<Block>,
    }
    Access {
        target: Box<Node<Expression>>,
        member: Node<Ident>,
    }
    Assignment {
        target: Node<LValue>,
        value: Node<Expression>,
    }
    Call {
        target: Node<LValue>,
        arguments: Vec<Node<Expression>>,
    }
    VarDefinition {
        name: Node<Ident>,
        value: Node<Expression>,
    }
    VarReference {
        name: Node<Ident>,
    }
    Block {
        statements: Vec<Node<Statement>>,
    }
    ClassDefinition {
        name: Node<Ident>,
        fields: Vec<RcString>,
        methods: Vec<Node<MethodDefinition>>,
    }
    MethodDefinition {
        name: Node<Ident>,
        parameters: Vec<RcString>,
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
        Expression,
    }
    Expression {
        Ident,
        Access,
        Call,
        Literal,
        VarReference,
    }
    Literal {
        String,
        Number,
        Boolean,
    }
    LValue {
        Ident,
        Access,
    }
}

pub enum Primitive {
    String(RcString),
    Number(f64),
    Boolean(bool),
}

pub type WeakObjectRef = Weak<RefCell<Object>>;
pub type ObjectRef = Rc<RefCell<Object>>;

pub struct Method {
    pub name: RcString,
    pub class: ObjectRef,
    pub params: Vec<RcString>,
    pub body: Node<Block>,
}

pub struct Object {
    pub class: ObjectRef,
    pub properties: HashMap<RcString, ObjectRef>,
    pub methods: HashMap<RcString, Method>,
    pub primitive: Option<Primitive>,
}

impl Object {
    fn name(&self) -> Option<RcString> {
        let class = self.class.borrow();
        let Some(Primitive::String(name)) = &class.primitive else {
            return None;
        };
        Some(name.clone())
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "#<>")
    }
}

impl Object {
    pub fn new_of_class(class: &ObjectRef) -> Object {
        Self {
            class: Rc::clone(class),
            primitive: None,
            properties: HashMap::new(),
            methods: HashMap::new(),
        }
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
