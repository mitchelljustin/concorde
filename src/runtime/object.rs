use crate::runtime::bootstrap::intrinsic;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::{Rc, Weak};

use crate::types::{Block, Node, Primitive, RcString};

pub type WeakObjectRef = Weak<RefCell<Object>>;
pub type ObjectRef = Rc<RefCell<Object>>;

pub enum MethodBody {}

pub struct Method {
    pub name: RcString,
    pub class: ObjectRef,
    pub params: Vec<RcString>,
    pub body: Node<Block>,
}

pub struct Object {
    pub class: ObjectRef,
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
    pub fn name(&self) -> Option<RcString> {
        let name_obj = self.properties.get(intrinsic::property::name)?.borrow();
        let Some(Primitive::String(name)) = &name_obj.primitive else {
            return None;
        };
        Some(name.clone())
    }

    pub fn debug(&self) -> String {
        let class_name = self.class.borrow().name().unwrap_or("???".into());
        let ptr = self.weak_self.as_ptr();
        format!("#<{} {:p}>", class_name, ptr)
    }

    pub fn set_property(&mut self, name: RcString, value: ObjectRef) {
        self.properties.insert(name, value);
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("ptr", &self.weak_self.as_ptr())
            .field("class_ptr", &self.class.as_ptr())
            .field("primitive", &self.primitive)
            .finish()
    }
}

impl Object {
    pub fn new_of_class(class: ObjectRef) -> ObjectRef {
        Rc::new_cyclic(|weak_self| {
            RefCell::new(Self {
                class,
                primitive: None,
                weak_self: weak_self.clone(),
                properties: HashMap::new(),
                methods: HashMap::new(),
            })
        })
    }
}
