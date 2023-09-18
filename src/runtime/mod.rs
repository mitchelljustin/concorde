use std::collections::HashMap;
use std::ops::ControlFlow;

use crate::runtime::bootstrap::{builtin, Builtins};
use crate::runtime::object::{Object, ObjectRef, WeakObjectRef};
use crate::runtime::Error::NoSuchVariable;
use crate::types::{NodeMeta, Primitive, RcString};

mod bootstrap;
mod interpret;
mod object;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("control flow")]
    ControlFlow(ControlFlow<()>),
    #[error("duplicate definition of method '{name}'")]
    DuplicateDefinition { class: ObjectRef, name: RcString },
    #[error("no such variable: '{name}'")]
    NoSuchVariable { name: RcString },
    #[error("no such method: '{class_name}::{method_name}'")]
    NoSuchMethod {
        class_name: RcString,
        method_name: RcString,
    },
    #[error("arity mismatch for '{class_name}::{method_name}()': expected {expected} args, got {actual}")]
    ArityMismatch {
        class_name: RcString,
        method_name: RcString,
        expected: usize,
        actual: usize,
    },
    #[error("object {target} has no property '{member}'\n    {access}")]
    UndefinedProperty {
        target: RcString,
        member: RcString,
        access: NodeMeta,
    },
    #[error("expression is not callable: {expr}")]
    NotCallable { expr: NodeMeta },
    #[error("illegal assignment target: '{target}.{member}'")]
    IllegalAssignmentTarget { target: RcString, member: RcString },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Default, Debug)]
pub struct StackFrame {
    receiver: Option<ObjectRef>,
    class: Option<ObjectRef>,
    method_name: Option<RcString>,
    variables: HashMap<RcString, ObjectRef>,
}

pub struct Runtime {
    all_objects: Vec<WeakObjectRef>,
    builtins: Builtins,
    stack: Vec<StackFrame>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut runtime = Self {
            all_objects: Vec::new(),
            stack: Vec::from([StackFrame::default()]),
            builtins: Builtins::default(),
        };
        runtime.bootstrap();
        runtime
    }

    fn find_closest(
        &self,
        finder: impl Fn(&StackFrame) -> Option<&ObjectRef>,
    ) -> Option<ObjectRef> {
        for frame in self.stack.iter().rev() {
            if let Some(found) = finder(&frame) {
                return Some(found.clone());
            }
        }
        None
    }

    fn current_class(&self) -> ObjectRef {
        self.find_closest(|frame| frame.class.as_ref())
            .expect("no class")
    }

    fn current_receiver(&self) -> ObjectRef {
        self.find_closest(|frame| frame.receiver.as_ref())
            .expect("no receiver")
    }

    pub fn create_string(&mut self, value: RcString) -> ObjectRef {
        let string_obj = self.create_object(self.builtins.String.clone());
        string_obj
            .borrow_mut()
            .set_primitive(Primitive::String(value));
        string_obj
    }

    pub fn create_bool(&mut self, value: bool) -> ObjectRef {
        if value {
            &self.builtins.bool_true
        } else {
            &self.builtins.bool_false
        }
        .clone()
    }

    pub fn create_number(&mut self, value: f64) -> ObjectRef {
        let number_obj = self.create_object(self.builtins.Number.clone());
        number_obj
            .borrow_mut()
            .set_primitive(Primitive::Number(value));
        number_obj
    }

    pub fn create_object(&mut self, class: ObjectRef) -> ObjectRef {
        let object = Object::new_of_class(class.clone());
        object
            .borrow_mut()
            .set_property(builtin::property::__class__.into(), class);
        self.all_objects.push(object.borrow().weak_self());
        object
    }

    pub fn create_class(&mut self, name: RcString) -> ObjectRef {
        let class = self.create_object(self.builtins.Class.clone());
        let name_obj = self.create_string(name.clone());
        class
            .borrow_mut()
            .set_property(builtin::property::__name__.into(), name_obj);
        self.assign_global(name, class.clone());
        class
    }

    pub fn assign_global(&mut self, name: RcString, object: ObjectRef) {
        self.stack[0].variables.insert(name, object);
    }

    pub fn resolve(&self, name: &str) -> Result<ObjectRef> {
        if name == builtin::SELF {
            return Ok(self.current_receiver());
        }
        self.find_closest(|frame| frame.variables.get(name))
            .ok_or(NoSuchVariable { name: name.into() })
    }

    pub fn assign(&mut self, name: RcString, object: ObjectRef) {
        self.stack
            .last_mut()
            .expect("no scope")
            .variables
            .insert(name, object);
    }

    fn nil(&self) -> ObjectRef {
        self.builtins.nil.clone()
    }
}
