use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ops::DerefMut;
use std::ptr;
use std::rc::Rc;

use crate::runtime::Runtime;
use crate::types::Object;

#[allow(non_upper_case_globals)]
mod builtin_class {
    pub const Class: &str = "Class";
}

impl Runtime {
    fn define_class_class(&mut self) {
        let garbage_object = unsafe { MaybeUninit::uninit().assume_init() };
        let mut class_class_object = Rc::new(RefCell::new(garbage_object));
        let initialized_class_object = Object::new_of_class(&class_class_object);
        unsafe {
            ptr::write(
                class_class_object.borrow_mut().deref_mut() as *mut Object,
                initialized_class_object,
            );
        }
        self.all_objects.push(Rc::downgrade(&class_class_object));
        self.define(builtin_class::Class.into(), class_class_object);
    }

    pub(crate) fn init(&mut self) {
        self.define_class_class();
    }
}
