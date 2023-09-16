use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ptr;
use std::rc::Rc;

use crate::runtime::object::{Object, ObjectRef};
use crate::runtime::Runtime;
use crate::types::{intrinsic, Primitive};

impl Runtime {
    fn define_class_class(&mut self) -> ObjectRef {
        let garbage_object_ref =
            Rc::new(RefCell::new(unsafe { MaybeUninit::uninit().assume_init() }));
        let class_class_object = Object::new_of_class(&garbage_object_ref);
        unsafe {
            ptr::copy(
                &class_class_object as *const ObjectRef,
                &mut class_class_object.borrow_mut().class as *mut ObjectRef,
                1,
            );
        }
        debug_assert_eq!(class_class_object.borrow().class, class_class_object);
        self.all_objects
            .push(class_class_object.borrow().self_ref.clone());
        self.assign(intrinsic::class::Class.into(), class_class_object.clone());
        class_class_object
    }

    #[allow(non_snake_case)]
    pub(crate) fn init(&mut self) {
        let class_Class = self.define_class_class();
        let class_String = self.create_class(intrinsic::class::String.into());
        let class_Class_name_obj = self.create_object(&class_String);
        class_Class_name_obj.borrow_mut().primitive =
            Some(Primitive::String(intrinsic::class::Class.into()));
        class_Class
            .borrow_mut()
            .set_property(intrinsic::property::name.into(), class_Class_name_obj);
    }
}
