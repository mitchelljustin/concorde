use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::ops::ControlFlow;

use crate::runtime::object::{Object, ObjectRef, WeakObjectRef};
use crate::runtime::Error::NoSuchObject;
use crate::types::{intrinsic, RcString};

mod bootstrap;
mod object;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    ControlFlow(ControlFlow<()>),
    NoSuchObject { name: RcString },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type Result<T = ObjectRef, E = Error> = std::result::Result<T, E>;

pub struct Builtins {
    class_class: ObjectRef,
    class_string: ObjectRef,
}

pub struct Runtime {
    all_objects: Vec<WeakObjectRef>,
    scope_stack: VecDeque<HashMap<RcString, ObjectRef>>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut runtime = Self {
            all_objects: Vec::new(),
            scope_stack: VecDeque::from([HashMap::new()]),
        };
        runtime.init();
        runtime
    }

    fn class_class(&self) -> ObjectRef {
        self.scope_stack
            .front()
            .unwrap()
            .get(intrinsic::class::Class)
            .unwrap()
            .clone()
    }

    pub fn create_object(&mut self, class: &ObjectRef) -> ObjectRef {
        let object = Object::new_of_class(class);
        self.all_objects.push(object.borrow().self_ref.clone());
        object
    }

    pub fn create_class(&mut self, name: RcString) -> ObjectRef {
        let class = self.create_object(&self.class_class());
        self.scope_stack
            .front_mut()
            .unwrap()
            .insert(name, class.clone());
        class
    }

    pub fn resolve(&self, name: &str) -> Result {
        for scope in self.scope_stack.iter().rev() {
            if let Some(object) = scope.get(name) {
                return Ok(object.clone());
            };
        }
        Err(NoSuchObject { name: name.into() })
    }

    pub fn assign(&mut self, name: RcString, object: ObjectRef) {
        self.scope_stack
            .back_mut()
            .expect("no scope")
            .insert(name, object);
    }
}
