use std::collections::HashMap;
use std::ops::ControlFlow;
use std::rc::{Rc, Weak};

use object::Primitive;

use crate::runtime::bootstrap::Builtins;
use crate::runtime::object::{MethodRef, Object, ObjectRef, WeakObjectRef};
use crate::types::{MaybeNodeMeta, NodeMeta};

mod bootstrap;
pub mod builtin;
mod interpret;
mod object;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("control flow")]
    ControlFlow(ControlFlow<()>),
    #[error("illegal return outside of function: {node}")]
    ReturnFromMethod {
        retval: Option<ObjectRef>,
        node: NodeMeta,
    },
    #[error("illegal return of value inside initializer: {node}")]
    ReturnFromInitializer { node: NodeMeta },
    #[error("duplicate definition of method '{class}::{name}'")]
    DuplicateMethodDefinition { class: String, name: String },
    #[error("duplicate definition of '{name}': {node}")]
    DuplicateClassDefinition { name: String, node: NodeMeta },
    #[error("no such variable '{name}': {node}")]
    NoSuchVariable { name: String, node: NodeMeta },
    #[error("no such property '{name}': {node}")]
    NoSuchProperty { name: String, node: NodeMeta },
    #[error("no such method '{search}': {node}")]
    NoSuchMethod { node: MaybeNodeMeta, search: String },
    #[error("arity mismatch for '{class_name}::{method_name}()': expected {expected} args, got {actual}")]
    ArityMismatch {
        class_name: String,
        method_name: String,
        expected: usize,
        actual: usize,
    },
    #[error("object {target} has no property '{member}': {node}")]
    UndefinedProperty {
        target: String,
        member: String,
        node: NodeMeta,
    },
    #[error("expression is not callable: {node}")]
    NotCallable { node: NodeMeta },
    #[error("illegal assignment target: {access}")]
    IllegalAssignmentTarget { access: NodeMeta },
    #[error("index error: {error}")]
    Index { error: &'static str },
    #[error("illegal constructor call: {class}")]
    IllegalConstructorCall { class: String },
    #[error("type error: expected {expected}, got {class}")]
    TypeMismatch { expected: String, class: String },
    #[error("bad path contains non-class '{non_class}': {path}")]
    BadPath { non_class: String, path: NodeMeta },
    #[error("bad iterator, {reason}: {node}")]
    BadIterator {
        reason: &'static str,
        node: NodeMeta,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Default, Debug)]
pub struct StackFrame {
    id: usize,
    instance: Option<ObjectRef>,
    class: Option<ObjectRef>,
    _method: Option<MethodRef>,
    open_classes: Vec<ObjectRef>,
    variables: HashMap<String, ObjectRef>,
}

#[derive(Default)]
pub struct Runtime {
    all_objects: Vec<WeakObjectRef>,
    builtins: Builtins,
    stack: Vec<StackFrame>,
    stack_id: usize,
    strings: HashMap<String, WeakObjectRef>,
    string_count_marker: usize,
}

pub const STRING_ALLOCATION_THRESHOLD: usize = 64;

impl Runtime {
    pub fn new() -> Self {
        let mut runtime = Self::default();
        runtime.bootstrap();
        runtime
    }

    fn find_closest_in_stack<T>(&self, finder: impl Fn(&StackFrame) -> Option<&T>) -> Option<&T> {
        for frame in self.stack.iter().rev() {
            if let Some(found) = finder(frame) {
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

    fn current_instance(&self) -> Option<ObjectRef> {
        self.find_closest_in_stack(|frame| frame.instance.as_ref())
            .cloned()
    }

    pub fn create_string(&mut self, value: impl AsRef<str> + Into<String>) -> ObjectRef {
        if let Some(string_obj) = self.strings.get(value.as_ref()).and_then(Weak::upgrade) {
            return string_obj.clone();
        }
        self.allocate_string(value)
    }

    fn allocate_string(&mut self, value: impl Into<String>) -> ObjectRef {
        let string_obj = self.create_object(self.builtins.String.clone());
        let string = value.into();
        string_obj
            .borrow_mut()
            .set_primitive(Primitive::String(string.clone()));
        self.strings.insert(string, Rc::downgrade(&string_obj));
        if self.strings.len() - self.string_count_marker >= STRING_ALLOCATION_THRESHOLD {
            self.cleanup_strings();
        }
        string_obj
    }

    pub fn cleanup_strings(&mut self) {
        let to_delete: Vec<_> = self
            .strings
            .iter()
            .filter_map(|(key, weak)| match weak.upgrade() {
                None => Some(key.clone()),
                Some(_) => None,
            })
            .collect();
        for key in to_delete {
            self.strings.remove(&key);
        }
        self.string_count_marker = self.strings.len();
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

    pub fn create_dictionary(&mut self, entries: Vec<(String, ObjectRef)>) -> ObjectRef {
        let dict_obj = self.create_object(self.builtins.Dictionary.clone());
        dict_obj
            .borrow_mut()
            .set_primitive(Primitive::Dictionary(entries.into_iter().collect()));
        dict_obj
    }

    pub fn create_object(&mut self, class: ObjectRef) -> ObjectRef {
        let object = Object::new_of_class(class.clone());
        object
            .borrow_mut()
            .set_property(builtin::property::__class__.into(), class);
        self.all_objects.push(object.borrow().weak_self());
        object
    }

    pub fn create_class(&mut self, name: String, superclass: Option<ObjectRef>) -> ObjectRef {
        let class = self.create_object(self.builtins.Class.clone());
        class.borrow_mut().superclass = superclass;
        let name_obj = self.create_string(name.clone());
        class
            .borrow_mut()
            .set_property(builtin::property::__name__.into(), name_obj);
        self.assign_global(name, class.clone());
        class
    }

    pub fn create_simple_class(&mut self, name: String) -> ObjectRef {
        self.create_class(name, Some(self.builtins.Object.clone()))
    }

    pub fn assign_global(&mut self, name: String, object: ObjectRef) {
        self.stack[0].variables.insert(name, object);
    }

    pub fn resolve_variable(&self, name: &str) -> Option<ObjectRef> {
        if name == builtin::SELF {
            return self.current_instance();
        }
        self.find_closest_in_stack(|frame| frame.variables.get(name))
            .cloned()
    }

    pub fn assign_variable(&mut self, name: String, object: ObjectRef) {
        for frame in self.stack.iter_mut().rev() {
            if frame.variables.contains_key(&name) {
                frame.variables.insert(name.clone(), object.clone());
                return;
            }
        }
        self.define_variable(name, object);
    }

    fn define_variable(&mut self, name: String, object: ObjectRef) {
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
