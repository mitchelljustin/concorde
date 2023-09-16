use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::{Rc, Weak};

use crate::types::{intrinsic, Block, Node, Primitive, RcString};

pub type WeakObjectRef = Weak<RefCell<Object>>;
pub type ObjectRef = Rc<RefCell<Object>>;

pub struct Method {
    pub name: RcString,
    pub class: ObjectRef,
    pub params: Vec<RcString>,
    pub body: Node<Block>,
}

pub struct Object {
    pub class: ObjectRef,
    pub self_ref: WeakObjectRef,
    pub properties: HashMap<RcString, ObjectRef>,
    pub methods: HashMap<RcString, Method>,
    pub primitive: Option<Primitive>,
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.self_ref.ptr_eq(&other.self_ref)
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
        let ptr = self.self_ref.as_ptr();
        format!("#<{} {:p}>", class_name, ptr)
    }

    pub fn set_property(&mut self, name: RcString, value: ObjectRef) {
        self.properties.insert(name, value);
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("ptr", &self.self_ref.as_ptr())
            .field("class_ptr", &self.class.as_ptr())
            .field("primitive", &self.primitive)
            .finish()
    }
}

impl Object {
    pub fn new_of_class(class: &ObjectRef) -> ObjectRef {
        Rc::new_cyclic(|self_ref| {
            RefCell::new(Self {
                class: Rc::clone(class),
                primitive: None,
                self_ref: self_ref.clone(),
                properties: HashMap::new(),
                methods: HashMap::new(),
            })
        })
    }
}
