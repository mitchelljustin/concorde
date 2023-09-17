use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::ops::ControlFlow;

use crate::runtime::bootstrap::{builtin, Builtins};
use crate::runtime::object::{Object, ObjectRef, WeakObjectRef};
use crate::runtime::Error::NoSuchObject;
use crate::types::{Primitive, RcString};

mod bootstrap;
mod interpret;
mod object;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    ControlFlow(ControlFlow<()>),
    DuplicateDefinition { name: RcString },
    NoSuchObject { name: RcString },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Default)]
pub struct Scope {
    receiver: Option<ObjectRef>,
    variables: HashMap<RcString, ObjectRef>,
}

pub struct Runtime {
    all_objects: Vec<WeakObjectRef>,
    builtins: Builtins,
    scope_stack: VecDeque<Scope>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut runtime = Self {
            all_objects: Vec::new(),
            scope_stack: VecDeque::from([Scope::default()]),
            builtins: Builtins::default(),
        };
        runtime.bootstrap();
        runtime
    }

    fn receiver(&self) -> ObjectRef {
        for scope in self.scope_stack.iter().rev() {
            if let Some(receiver) = &scope.receiver {
                return receiver.clone();
            }
        }
        panic!("no receiver")
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
        self.scope_stack
            .front_mut()
            .unwrap()
            .variables
            .insert(name, object);
    }

    pub fn resolve(&self, name: &str) -> Result<ObjectRef> {
        if name == builtin::SELF {
            return Ok(self.receiver());
        }
        for scope in self.scope_stack.iter().rev() {
            if let Some(object) = scope.variables.get(name) {
                return Ok(object.clone());
            };
        }
        Err(NoSuchObject { name: name.into() })
    }

    pub fn assign(&mut self, name: RcString, object: ObjectRef) {
        self.scope_stack
            .back_mut()
            .expect("no scope")
            .variables
            .insert(name, object);
    }

    fn nil(&self) -> ObjectRef {
        self.builtins.nil.clone()
    }
}
