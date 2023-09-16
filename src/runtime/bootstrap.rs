use std::mem::MaybeUninit;
use std::ptr;

use crate::runtime::object::{Object, ObjectRef};
use crate::runtime::Runtime;

macro define_string_consts($($name:ident,)+) {
$(
        pub const $name: &str = stringify!($name);
    )+
}

#[allow(non_upper_case_globals)]
pub mod intrinsic {
    pub mod class {
        use crate::runtime::bootstrap::define_string_consts;
        use crate::runtime::object::ObjectRef;
        use crate::runtime::Runtime;
        use crate::types::RcString;

        macro define_classes(
            StdClasses = $StdClasses:ident,
            Class = $Class:ident,
            create_builtin_classes = $create_builtin_classes:ident,
            builtin_classes = [$($builtin_class:ident,)+]
        ) {
            define_string_consts!($Class, $($builtin_class,)+);

            pub struct $StdClasses {
                pub $Class: ObjectRef,
                $(
                    pub $builtin_class: ObjectRef,
                )+
            }

            pub fn $create_builtin_classes(runtime: &mut Runtime, class_class: ObjectRef) {
                runtime.std_classes = Some($StdClasses {
                    $(
                        $builtin_class: runtime.create_object(class_class.clone()),
                    )+
                    $Class: class_class,
                });
                let property_name: RcString = $crate::runtime::bootstrap::intrinsic::property::name.into();
                let class_name_obj = runtime.create_string($Class.into());
                runtime.classes().$Class.borrow_mut().set_property(
                    property_name.clone(),
                    class_name_obj,
                );
                $(
                    let class = runtime.classes().$builtin_class.clone();
                    let class_name: RcString = $builtin_class.into();
                    let class_name_obj = runtime.create_string(class_name.clone());
                    runtime.assign_global(
                        class_name,
                        class,
                    );
                    runtime.classes().$builtin_class.borrow_mut().set_property(
                        property_name.clone(),
                        class_name_obj,
                    );
                )+
            }
        }

        define_classes![
            StdClasses = StdClasses,
            Class = Class,
            create_builtin_classes = create_builtin_classes,
            builtin_classes = [String, Number, NilClass,]
        ];
    }

    pub mod property {
        use crate::runtime::bootstrap::define_string_consts;

        define_string_consts![name,];
    }
}

#[allow(non_snake_case)]
impl Runtime {
    fn create_Class(&mut self) -> ObjectRef {
        // this is super illegal but it's the only way to create a strong cyclical Rc reference
        let garbage_object_ref: ObjectRef = unsafe { MaybeUninit::zeroed().assume_init() };
        let Class = Object::new_of_class(garbage_object_ref);
        unsafe {
            let Class_copy = ptr::read(&Class as *const _);
            ptr::write(&mut Class.borrow_mut().class as *mut _, Class_copy);
        }
        debug_assert_eq!(Class.borrow().class, Class);
        self.all_objects.push(Class.borrow().weak_self.clone());
        self.assign_global(intrinsic::class::Class.into(), Class.clone());
        Class
    }

    pub(crate) fn initialize(&mut self) {
        let Class = self.create_Class();
        intrinsic::class::create_builtin_classes(self, Class);
    }
}
