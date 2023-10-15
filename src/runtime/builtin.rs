#![allow(non_upper_case_globals)]

pub const SELF: &str = "self";

macro define_string_consts($($name:ident,)+) {
    $(
        pub const $name: &str = stringify!($name);
    )+
}

pub mod class {
    use crate::runtime::builtin::define_string_consts;

    define_string_consts![
        Class,
        Object,
        String,
        NilClass,
        Main,
        IO,
        Bool,
        Number,
        Closure,
        Dictionary,
        DictionaryIter,
        Array,
        Tuple,
    ];
}

pub mod property {
    use crate::runtime::builtin::define_string_consts;

    define_string_consts![__name__, __class__, __binding__,];
}

pub mod method {
    use crate::runtime::builtin::define_string_consts;

    define_string_consts![init, to_s, iter, next,];
}

pub mod op {
    use crate::runtime::builtin::define_string_consts;
    use crate::types::Operator;

    define_string_consts![
        __add__,
        __sub__,
        __mul__,
        __div__,
        __gt__,
        __gte__,
        __lt__,
        __lte__,
        __eq__,
        __neq__,
        __neg__,
        __not__,
        __index__,
        __set_index__,
        __call__,
    ];

    pub fn method_for_assignment_op(op: &Operator) -> Option<&str> {
        Some(match op {
            Operator::PlusEqual => __add__,
            Operator::MinusEqual => __sub__,
            Operator::StarEqual => __mul__,
            Operator::SlashEqual => __div__,
            _ => return None,
        })
    }

    pub fn method_for_binary_op(op: &Operator) -> Option<&str> {
        Some(match op {
            Operator::EqualEqual => __eq__,
            Operator::NotEqual => __neq__,
            Operator::Greater => __gt__,
            Operator::GreaterEqual => __gte__,
            Operator::Less => __lt__,
            Operator::LessEqual => __lte__,
            Operator::Plus => __add__,
            Operator::Minus => __sub__,
            Operator::Star => __mul__,
            Operator::Slash => __div__,
            Operator::LogicalNot => __not__,
            _ => return None,
        })
    }

    pub fn method_for_unary_op(op: &Operator) -> Option<&str> {
        Some(match op {
            Operator::Minus => __neg__,
            Operator::LogicalNot => __not__,
            _ => return None,
        })
    }
}
