use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::ops::ControlFlow;

use crate::runtime::bootstrap::{builtin, Builtins};
use crate::runtime::object::{Object, ObjectRef, WeakObjectRef};
use crate::runtime::Error::NoSuchVariable;
use crate::types::{Primitive, RcString};

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
    #[error("no such method on {class:?}: '{name}'")]
    NoSuchMethod { class: ObjectRef, name: RcString },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Default)]
pub struct StackFrame {
    receiver: Option<ObjectRef>,
    class: Option<ObjectRef>,
    variables: HashMap<RcString, ObjectRef>,
}

pub struct Runtime {
    all_objects: Vec<WeakObjectRef>,
    builtins: Builtins,
    stack: VecDeque<StackFrame>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut runtime = Self {
            all_objects: Vec::new(),
            stack: VecDeque::from([StackFrame::default()]),
            builtins: Builtins::default(),
        };
        runtime.bootstrap();
        runtime
    }

    fn find_closest<T: Clone>(&self, finder: impl Fn(&StackFrame) -> Option<&T>) -> Option<T> {
        for frame in self.stack.iter().rev() {
            if let Some(found) = finder(&frame) {
                return Some(found.clone());
            }
        }
        None
    }

    fn class(&self) -> ObjectRef {
        self.find_closest(|frame| frame.class.as_ref())
            .expect("no class")
    }

    fn receiver(&self) -> ObjectRef {
        self.find_closest(|frame| frame.receiver.as_ref())
            .expect("no receiver")
    }

    pub fn create_string(&mut self, value: RcString) -> ObjectRef {
        let string_obj = self.create_object(self.builtins.String.clone());
        string_obj.borrow_mut().primitive = Some(Primitive::String(value));
        string_obj
    }

    pub fn create_object(&mut self, class: ObjectRef) -> ObjectRef {
        let object = Object::new_of_class(class);
        self.all_objects.push(object.borrow().weak_self.clone());
        object
    }

    pub fn create_class(&mut self, name: RcString) -> ObjectRef {
        let class = self.create_object(self.builtins.Class.clone());
        let name_obj = self.create_string(name.clone());
        class
            .borrow_mut()
            .set_property(builtin::property::name.into(), name_obj);
        self.assign_global(name, class.clone());
        class
    }

    pub fn assign_global(&mut self, name: RcString, object: ObjectRef) {
        self.stack
            .front_mut()
            .unwrap()
            .variables
            .insert(name, object);
    }

    pub fn resolve(&self, name: &str) -> Result<ObjectRef> {
        if name == builtin::SELF {
            return Ok(self.receiver());
        }
        self.find_closest(|frame| frame.variables.get(name))
            .ok_or(NoSuchVariable { name: name.into() })
    }

    pub fn assign(&mut self, name: RcString, object: ObjectRef) {
        self.stack
            .back_mut()
            .expect("no scope")
            .variables
            .insert(name, object);
    }

    fn nil(&self) -> ObjectRef {
        self.builtins.nil.clone()
    }
}
