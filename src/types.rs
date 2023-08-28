use std::io;

use crate::{compiler, parser, runtime};

#[derive(thiserror::Error, Debug)]
pub enum TopError {
    #[error("compiler error: {0}")]
    Compiler(#[from] compiler::Error),
    #[error("runtime error: {0}")]
    Runtime(#[from] runtime::Error),

    #[error("parser error: {0}")]
    Parser(#[from] parser::Error),
    #[error("I/O error: {0}")]
    IO(#[from] io::Error),
}

pub struct SyntaxNode<Variant = AnyNode> {
    pub(crate) source: String,
    pub(crate) variant: Variant,
}

define_node_types! {
    [AnyNode]

    Type {
        name: String,
    }
    Ident {
        name: String,
    }
    Number {
        value: f64,
    }
    Boolean {
        value: bool,
    }
    LiteralString {
        value: String,
    }

    Program {
        body: Block,
    }
    IfElse {
        condition: Expression,
        then_body: Block,
        else_body: Option<Block>,
    }
    ForLoop {
        iterator: Ident,
        target: Expression,
        body: Block,
    }
    WhileLoop {
        condition: Expression,
        body: Block,
    }
    Access {
        target: Box<Expression>,
        member: Ident,
    }
    Assignment {
        target: LValue,
        value: Expression,
    }
    Call {
        target: LValue,
        arguments: Vec<Expression>,
    }
    VarDefinition {
        name: Ident,
        ty: Type,
        value: Expression,
    }
    Block {
        stmts: Vec<Statement>,
    }
    ClassDefinition {
        name: Ident,
        fields: Vec<Parameter>,
        methods: Vec<MethodDefinition>,
    }
    MethodDefinition {
        name: Ident,
        parameters: Vec<Parameter>,
        return_ty: Type,
        body: Block,
    }
    Parameter {
        name: Ident,
        ty: Type,
    }
}

define_collector_enums! {
    Statement {
        IfElse,
        ForLoop,
        WhileLoop,
        Block,
    }
    Expression {
        Ident,
        Access,
        Call,
        Literal,
    }
    Literal {
        LiteralString,
        Number,
        Boolean,
    }
    LValue {
        Ident,
        Access,
    }
}

pub struct Executable {
    instructions: Vec<Instruction>,
}

#[derive(Debug)]
pub enum Instruction {
    New {
        class: ClassId,
    },
    Call {
        receiver: ObjectId,
        method: MethodId,
        arguments: Vec<ObjectId>,
    },
}

create_id_types![ClassId, ObjectId, MethodId,];

pub struct Method {
    name: String,
}

pub struct Class {
    methods: Vec<Method>,
}

pub struct Object {
    class: ClassId,
}

macro define_node_types(
    [$any_node_name:ident]
$(
    $name:ident {
        $(
            $field:ident : $ty:ty,
        )+
    }
)+) {
    $(
        #[derive(Debug, Clone)]
        pub struct $name {
            $(
                pub $field : $ty,
            )+
        }
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
                $variant($variant),
            )+
        }
    )+
}

macro create_id_types($($name:ident,)+) {
$(
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $name(usize);
    )+
}
