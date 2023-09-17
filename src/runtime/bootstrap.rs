use std::ptr;

use crate::runtime::object::{Object, ObjectRef};
use crate::runtime::Runtime;
use crate::types::RcString;

#[allow(non_upper_case_globals)]
pub mod builtin {
    pub mod class {
        pub const Class: &str = "Class";
        pub const String: &str = "String";
    }

    pub mod property {
        pub const name: &str = "__name__";
    }
}

#[allow(non_snake_case)]
pub struct Builtins {
    pub Class: ObjectRef,
    pub String: ObjectRef,
}

impl Default for Builtins {
    fn default() -> Self {
        Self {
            Class: Object::new_dummy(),
            String: Object::new_dummy(),
        }
    }
}

#[allow(non_snake_case)]
impl Runtime {
    pub(crate) fn initialize(&mut self) {
        self.builtins.Class = Object::new_dummy();
        let Class = self.builtins.Class.clone();
        Class.borrow_mut().class = Some(Class.clone());
        // now we can create classes
        self.all_objects.push(Class.borrow().weak_self.clone());
        let name_Class: RcString = builtin::class::Class.into();
        self.assign_global(name_Class.clone(), Class.clone());

        self.builtins.String = self.create_object(Class.clone());
        let name_String: RcString = builtin::class::String.into();
        self.assign_global(name_String.clone(), self.builtins.String.clone());
        // now we can create strings
        let name_Class_obj = self.create_string(name_Class);
        let name_String_obj = self.create_string(name_String);
        Class
            .borrow_mut()
            .set_property(builtin::property::name.into(), name_Class_obj);
        self.builtins
            .String
            .borrow_mut()
            .set_property(builtin::property::name.into(), name_String_obj);
    }
}
