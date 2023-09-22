#![allow(non_upper_case_globals)]

pub const SELF: &str = "self";

pub mod class {
    pub const Class: &str = "Class";
    pub const Object: &str = "Object";
    pub const String: &str = "String";
    pub const NilClass: &str = "NilClass";
    pub const Main: &str = "Main";
    pub const IO: &str = "IO";
    pub const Bool: &str = "Bool";
    pub const Number: &str = "Number";
    pub const Array: &str = "Array";
    pub const ArrayIter: &str = "ArrayIter";
}

pub mod property {
    pub const __name__: &str = "__name__";
    pub const __class__: &str = "__class__";
}

pub mod method {
    pub const init: &str = "init";
    pub const to_s: &str = "to_s";
    pub const iter: &str = "iter";
    pub const next: &str = "next";
}

pub mod op {
    use crate::types::Operator;

    pub const __add__: &str = "__add__";
    pub const __sub__: &str = "__sub__";
    pub const __mul__: &str = "__mul__";
    pub const __div__: &str = "__div__";
    pub const __gt__: &str = "__gt__";
    pub const __gte__: &str = "__gte__";
    pub const __lt__: &str = "__lt__";
    pub const __lte__: &str = "__lte__";
    pub const __eq__: &str = "__eq__";
    pub const __neq__: &str = "__neq__";
    pub const __neg__: &str = "__neg__";
    pub const __not__: &str = "__not__";
    pub const __index__: &str = "__index__";
    pub const __set_index__: &str = "__set_index__";

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
