use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::rc::Rc;

use crate::runtime::object::{Object, ObjectRef};
use crate::runtime::Runtime;
use crate::types::{intrinsic, Primitive, RcString};

impl Runtime {
    fn define_class_class(&mut self) -> ObjectRef {
        let garbage_object = unsafe { MaybeUninit::uninit().assume_init() };
        let class_class_object = Rc::new(RefCell::new(garbage_object));
        let initialized_class_object = Object::new_of_class(&class_class_object);
        unsafe {
            ptr::copy(
                initialized_class_object.borrow().deref() as *const Object,
                class_class_object.borrow_mut().deref_mut() as *mut Object,
                1,
            );
        }
        self.all_objects
            .push(class_class_object.borrow().self_ref.clone());
        self.assign(intrinsic::class::Class.into(), class_class_object.clone());
        class_class_object
    }

    pub(crate) fn init(&mut self) {
        let class_class = self.define_class_class();
        let string_class = self.create_object(&class_class);
        let class_class_name_string: RcString = intrinsic::class::String.into();
        let class_class_name = self.create_object(&string_class);
        class_class_name.borrow_mut().primitive =
            Some(Primitive::String(class_class_name_string.clone()));
        class_class
            .borrow_mut()
            .properties
            .insert(intrinsic::property::name.into(), class_class_name.clone());
        dbg!(
            &class_class,
            &string_class,
            &class_class_name_string,
            &class_class_name,
        );
        self.assign(class_class_name_string.clone(), string_class);
    }
}
