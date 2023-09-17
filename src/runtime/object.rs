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

pub enum MethodBody {
    User(Node<Block>),
    System(fn(&Runtime, ObjectRef, Vec<ObjectRef>) -> ObjectRef),
}

pub enum Param {
    Positional(RcString),
    Vararg(RcString),
}

pub struct Method {
    pub name: RcString,
    pub class: WeakObjectRef,
    pub params: Vec<Param>,
    pub body: MethodBody,
}

pub struct Object {
    pub class: Option<ObjectRef>,
    pub weak_self: WeakObjectRef,
    pub properties: HashMap<RcString, ObjectRef>,
    pub methods: HashMap<RcString, Method>,
    pub primitive: Option<Primitive>,
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.weak_self.ptr_eq(&other.weak_self)
    }
}

impl Object {
    pub fn get_name(&self) -> Option<RcString> {
        let name_obj = self.properties.get(builtin::property::name)?.borrow();
        let Some(Primitive::String(name)) = &name_obj.primitive else {
            return None;
        };
        Some(name.clone())
    }

    pub fn debug(&self) -> String {
        let class_name = self
            .class
            .as_ref()
            .unwrap()
            .borrow()
            .get_name()
            .expect("class has no __name__");
        let ptr = self.weak_self.as_ptr();
        format!("#<{} {:p}>", class_name, ptr)
    }

    pub fn set_property(&mut self, name: RcString, value: ObjectRef) {
        self.properties.insert(name, value);
    }

    pub fn class(&self) -> ObjectRef {
        self.class.clone().unwrap()
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
        self.methods.insert(method_name, method);
        Ok(())
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("ptr", &self.weak_self.as_ptr())
            .field("class_ptr", &self.class.as_ref().map(Rc::as_ptr))
            .field("primitive", &self.primitive)
            .finish()
    }
}

impl Object {
    pub fn new_dummy() -> ObjectRef {
        Rc::new_cyclic(|weak_self| {
            RefCell::new(Self {
                class: None,
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
                primitive: None,
                weak_self: weak_self.clone(),
                properties: HashMap::new(),
                methods: HashMap::new(),
            })
        })
    }
}
