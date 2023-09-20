use std::collections::HashMap;
use std::ops::ControlFlow;

use object::Primitive;

use crate::runtime::bootstrap::{builtin, Builtins};
use crate::runtime::object::{Object, ObjectRef, WeakObjectRef};
use crate::types::{NodeMeta, RcString};

mod bootstrap;
mod interpret;
mod object;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("control flow")]
    ControlFlow(ControlFlow<()>),
    #[error("duplicate definition of method '{name}'")]
    DuplicateDefinition { class: ObjectRef, name: RcString },
    #[error("no such variable: '{name}': {node}")]
    NoSuchVariable { name: RcString, node: NodeMeta },
    #[error("no such method: '{class_name}::{method_name}'")]
    NoSuchMethod {
        class_name: RcString,
        method_name: RcString,
    },
    #[error("not a class method: '{class_name}::{method_name}'")]
    NotAClassMethod {
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
    #[error("object {target} has no property '{member}': {node}")]
    UndefinedProperty {
        target: RcString,
        member: RcString,
        node: NodeMeta,
    },
    #[error("expression is not callable: {node}")]
    NotCallable { node: NodeMeta },
    #[error("illegal assignment target: {access}")]
    IllegalAssignmentTarget { access: NodeMeta },
    #[error("index error: {error}")]
    IndexError { error: &'static str },
    #[error("illegal constructor call: {class}")]
    IllegalConstructorCall { class: RcString },
    #[error("type error: expected {expected}, got {class}")]
    TypeError { expected: RcString, class: RcString },
    #[error("bad path contains non-class '{non_class}': {path}")]
    BadPath { non_class: RcString, path: NodeMeta },
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

    fn find_closest_in_stack<T>(&self, finder: impl Fn(&StackFrame) -> Option<&T>) -> Option<&T> {
        for frame in self.stack.iter().rev() {
            if let Some(found) = finder(&frame) {
                return Some(found);
            }
        }
        None
    }

    fn current_class(&self) -> ObjectRef {
        self.find_closest_in_stack(|frame| frame.class.as_ref())
            .cloned()
            .expect("no class")
    }

    fn current_receiver(&self) -> ObjectRef {
        self.find_closest_in_stack(|frame| frame.receiver.as_ref())
            .cloned()
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

    pub fn create_array(&mut self, elements: Vec<ObjectRef>) -> ObjectRef {
        let array_obj = self.create_object(self.builtins.Array.clone());
        array_obj
            .borrow_mut()
            .set_primitive(Primitive::Array(elements));
        array_obj
    }

    pub fn create_object(&mut self, class: ObjectRef) -> ObjectRef {
        let object = Object::new_of_class(class.clone());
        object
            .borrow_mut()
            .set_property(builtin::property::__class__.into(), class);
        self.all_objects.push(object.borrow().weak_self());
        object
    }

    pub fn create_class(&mut self, name: RcString, superclass: Option<ObjectRef>) -> ObjectRef {
        let class = self.create_object(self.builtins.Class.clone());
        class.borrow_mut().superclass = superclass;
        let name_obj = self.create_string(name.clone());
        class
            .borrow_mut()
            .set_property(builtin::property::__name__.into(), name_obj);
        self.assign_global(name, class.clone());
        class
    }

    pub fn create_simple_class(&mut self, name: RcString) -> ObjectRef {
        self.create_class(name, Some(self.builtins.Object.clone()))
    }

    pub fn assign_global(&mut self, name: RcString, object: ObjectRef) {
        self.stack[0].variables.insert(name, object);
    }

    pub fn resolve_variable(&self, name: &str) -> Option<ObjectRef> {
        if name == builtin::SELF {
            return Some(self.current_receiver());
        }
        self.find_closest_in_stack(|frame| frame.variables.get(name))
            .cloned()
    }

    pub fn assign_variable(&mut self, name: RcString, object: ObjectRef) {
        for frame in self.stack.iter_mut().rev() {
            if frame.variables.contains_key(&name) {
                frame.variables.insert(name.clone(), object.clone());
                return;
            }
        }
        self.define_variable(name, object);
    }

    fn define_variable(&mut self, name: RcString, object: ObjectRef) {
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
