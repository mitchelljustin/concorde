use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::{Rc, Weak};

use crate::runtime::builtin;
use crate::runtime::Error::DuplicateMethodDefinition;
use crate::runtime::{Result, Runtime};
use crate::types::{Block, Node};

pub type WeakObjectRef = Weak<RefCell<Object>>;
pub type ObjectRef = Rc<RefCell<Object>>;
pub type MethodRef = Rc<Method>;

pub type SystemMethod = fn(
    runtime: &mut Runtime,
    this: ObjectRef,
    method_name: String,
    arguments: Vec<ObjectRef>,
) -> Result<ObjectRef>;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MethodReceiver {
    Instance,
    Class,
}

#[derive(Debug)]
pub enum MethodBody {
    User(Node<Block>),
    System(SystemMethod),
}

#[derive(Debug)]
pub enum Param {
    Positional(String),
    Vararg(String),
}

#[derive(Debug, Clone)]
pub enum Primitive {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<ObjectRef>),
    Dictionary(HashMap<String, ObjectRef>),
}

#[derive(Debug)]
pub struct Method {
    pub name: String,
    pub class: WeakObjectRef,
    pub params: Vec<Param>,
    pub body: MethodBody,
    pub receiver: MethodReceiver,
}

pub struct Object {
    pub(super) class: Option<ObjectRef>,
    pub(super) superclass: Option<ObjectRef>,
    weak_self: WeakObjectRef,
    properties: HashMap<String, ObjectRef>,
    methods: HashMap<String, MethodRef>,
    primitive: Option<Primitive>,
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.weak_self.ptr_eq(&other.weak_self)
    }
}

pub const DEFAULT_NAME: &str = "(anonymous)";

impl Object {
    pub fn __name__(&self) -> Option<String> {
        Some(
            self.properties
                .get(builtin::property::__name__)?
                .borrow()
                .string()?
                .clone(),
        )
    }

    pub fn __debug__(&self) -> String {
        let class_name = self
            .class
            .as_ref()
            .unwrap()
            .borrow()
            .__name__()
            .unwrap_or(DEFAULT_NAME.into());
        let ptr = self.weak_self.as_ptr();
        format!("#<{} {:p}>", class_name, ptr)
    }

    pub fn __class__(&self) -> ObjectRef {
        self.class.clone().unwrap()
    }

    pub fn get_init_method(&self) -> MethodRef {
        self.resolve_own_method(builtin::method::init)
            .unwrap_or_else(|| {
                Rc::new(Method {
                    receiver: MethodReceiver::Instance,
                    class: self.weak_self(),
                    name: builtin::method::init.into(),
                    body: MethodBody::System(|runtime, class, _, _| {
                        Ok(runtime.create_object(class))
                    }),
                    params: Vec::new(),
                })
            })
    }

    pub fn number(&self) -> Option<f64> {
        let Some(Primitive::Number(value)) = self.primitive.clone() else {
            return None;
        };
        Some(value)
    }

    pub fn bool(&self) -> Option<bool> {
        let Some(Primitive::Boolean(value)) = self.primitive.clone() else {
            return None;
        };
        Some(value)
    }

    pub fn string(&self) -> Option<&String> {
        let Some(Primitive::String(value)) = &self.primitive else {
            return None;
        };
        Some(value)
    }

    pub fn array(&self) -> Option<&Vec<ObjectRef>> {
        let Some(Primitive::Array(value)) = &self.primitive else {
            return None;
        };
        Some(value)
    }

    pub fn dictionary(&self) -> Option<&HashMap<String, ObjectRef>> {
        let Some(Primitive::Dictionary(value)) = &self.primitive else {
            return None;
        };
        Some(value)
    }

    pub fn array_mut(&mut self) -> Option<&mut Vec<ObjectRef>> {
        let Some(Primitive::Array(value)) = &mut self.primitive else {
            return None;
        };
        Some(value)
    }

    pub fn dictionary_mut(&mut self) -> Option<&mut HashMap<String, ObjectRef>> {
        let Some(Primitive::Dictionary(value)) = &mut self.primitive else {
            return None;
        };
        Some(value)
    }

    pub fn set_property(&mut self, name: impl Into<String>, value: ObjectRef) {
        self.properties.insert(name.into(), value);
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
        receiver: MethodReceiver,
        method_name: String,
        params: Vec<Param>,
        body: MethodBody,
    ) -> Result<()> {
        if self.methods.contains_key(&method_name) {
            return Err(DuplicateMethodDefinition {
                class: self
                    .weak_self
                    .upgrade()
                    .expect("help i dont exist")
                    .borrow()
                    .__name__()
                    .unwrap(),
                name: method_name.clone(),
            });
        }
        let method = Method {
            name: method_name.clone(),
            class: self.weak_self.clone(),
            receiver,
            params,
            body,
        };
        self.methods.insert(method_name, Rc::new(method));
        Ok(())
    }

    pub fn resolve_own_method(&self, name: &str) -> Option<MethodRef> {
        if let Some(method) = self.methods.get(name) {
            return Some(method.clone());
        };
        if let Some(superclass) = self.superclass.as_ref() {
            return superclass.borrow().resolve_own_method(name);
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
