use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ptr;
use std::rc::Rc;

use crate::runtime::object::{Object, ObjectRef};
use crate::runtime::Runtime;
use crate::types::{intrinsic, Primitive};

#[allow(non_snake_case)]
impl Runtime {
    fn define_class_class(&mut self) -> ObjectRef {
        let garbage_object_ref =
            Rc::new(RefCell::new(unsafe { MaybeUninit::uninit().assume_init() }));
        let class_Class = Object::new_of_class(&garbage_object_ref);
        unsafe {
            ptr::copy(
                &class_Class as *const ObjectRef,
                &mut class_Class.borrow_mut().class as *mut ObjectRef,
                1,
            );
        }
        debug_assert_eq!(class_Class.borrow().class, class_Class);
        self.all_objects.push(class_Class.borrow().self_ref.clone());
        self.assign_global(intrinsic::class::Class.into(), class_Class.clone());
        class_Class
    }

    pub(crate) fn init(&mut self) {
        let class_Class = self.define_class_class();
        let class_String = self.create_class(intrinsic::class::String.into());
        let string_Class = self.create_object(&class_String);
        string_Class.borrow_mut().primitive =
            Some(Primitive::String(intrinsic::class::Class.into()));
        class_Class
            .borrow_mut()
            .set_property(intrinsic::property::name.into(), string_Class);
    }
}
