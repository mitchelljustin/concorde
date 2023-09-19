use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::{Rc, Weak};

use crate::runtime::bootstrap::builtin;
use crate::runtime::Error::DuplicateDefinition;
use crate::runtime::{Result, Runtime};
use crate::types::{Block, Node, Primitive, RcString};

pub type WeakObjectRef = Weak<RefCell<Object>>;
pub type ObjectRef = Rc<RefCell<Object>>;
pub type MethodRef = Rc<Method>;

pub type SystemMethod = fn(
    runtime: &mut Runtime,
    this: ObjectRef,
    method_name: &str,
    arguments: Vec<ObjectRef>,
) -> Result<ObjectRef>;

#[derive(Debug)]
pub enum MethodBody {
    User(Node<Block>),
    System(SystemMethod),
}

#[derive(Debug)]
pub enum Param {
    Positional(RcString),
    Vararg(RcString),
}

#[derive(Debug)]
pub struct Method {
    pub name: RcString,
    pub class: WeakObjectRef,
    pub params: Vec<Param>,
    pub body: MethodBody,
}

pub struct Object {
    pub(super) class: Option<ObjectRef>,
    pub(super) superclass: Option<ObjectRef>,
    weak_self: WeakObjectRef,
    properties: HashMap<RcString, ObjectRef>,
    methods: HashMap<RcString, MethodRef>,
    primitive: Option<Primitive>,
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.weak_self.ptr_eq(&other.weak_self)
    }
}

const DEFAULT_NAME: &str = "(anonymous)";

impl Object {
    pub fn __name__(&self) -> Option<RcString> {
        Some(
            self.properties
                .get(builtin::property::__name__)?
                .borrow()
                .string()?
                .clone(),
        )
    }

    pub fn __debug__(&self) -> RcString {
        let class_name = self
            .class
            .as_ref()
            .unwrap()
            .borrow()
            .__name__()
            .unwrap_or(DEFAULT_NAME.into());
        let ptr = self.weak_self.as_ptr();
        format!("#<{} {:p}>", class_name, ptr).into()
    }

    pub fn __class__(&self) -> ObjectRef {
        self.class.as_ref().unwrap().clone()
    }

    pub fn number(&self) -> Option<f64> {
        let Primitive::Number(value) = self.primitive.clone().unwrap() else {
            return None;
        };
        Some(value)
    }

    pub fn string(&self) -> Option<RcString> {
        let Primitive::String(value) = self.primitive.clone().unwrap() else {
            return None;
        };
        Some(value)
    }

    pub fn bool(&self) -> Option<bool> {
        let Primitive::Boolean(value) = self.primitive.clone().unwrap() else {
            return None;
        };
        Some(value)
    }

    pub fn set_property(&mut self, name: RcString, value: ObjectRef) {
        self.properties.insert(name, value);
    }

    pub fn get_property(&self, name: &str) -> Option<ObjectRef> {
        self.properties.get(name).cloned()
    }

    pub fn weak_self(&self) -> WeakObjectRef {
        self.weak_self.clone()
    }

    pub fn set_primitive(&mut self, primitive: Primitive) {
        self.primitive = Some(primitive);
    }

    pub fn define_method(
        &mut self,
        method_name: RcString,
        params: Vec<Param>,
        body: MethodBody,
    ) -> Result<()> {
        if self.methods.contains_key(&method_name) {
            return Err(DuplicateDefinition {
                class: self.weak_self.upgrade().expect("help i dont exist"),
                name: method_name.clone(),
            });
        }
        let method = Method {
            name: method_name.clone(),
            class: self.weak_self.clone(),
            params,
            body,
        };
        self.methods.insert(method_name, Rc::new(method));
        Ok(())
    }

    pub fn resolve_method(&self, name: &str) -> Option<MethodRef> {
        if let Some(method) = self.methods.get(name) {
            return Some(method.clone());
        };
        if let Some(superclass) = self.superclass.as_ref() {
            return superclass.borrow().resolve_method(name);
        }
        None
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("ptr", &self.weak_self.as_ptr())
            .field("class_ptr", &self.class.as_ref().map(Rc::as_ptr))
            .field(
                "class_name",
                &self.class.as_ref().map(|class| class.borrow().__name__()),
            )
            .field("primitive", &self.primitive)
            .finish()
    }
}

impl Object {
    pub fn new_dummy() -> ObjectRef {
        Rc::new_cyclic(|weak_self| {
            RefCell::new(Self {
                class: None,
                superclass: None,
                primitive: None,
                weak_self: weak_self.clone(),
                properties: HashMap::new(),
                methods: HashMap::new(),
            })
        })
    }

    pub fn new_of_class(class: ObjectRef) -> ObjectRef {
        Rc::new_cyclic(|weak_self| {
            RefCell::new(Self {
                class: Some(class),
                superclass: None,
                primitive: None,
                weak_self: weak_self.clone(),
                properties: HashMap::new(),
                methods: HashMap::new(),
            })
        })
    }
}
