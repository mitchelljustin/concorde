use crate::runtime::object::{MethodBody, Object, ObjectRef, Param};
use crate::runtime::Runtime;
use crate::types::{Primitive, RcString};

#[allow(non_upper_case_globals)]
pub mod builtin {
    pub const SELF: &str = "self";

    pub mod class {
        pub const Class: &str = "Class";
        pub const String: &str = "String";
        pub const NilClass: &str = "NilClass";
        pub const Main: &str = "Main";
        pub const Bool: &str = "Bool";
        pub const Number: &str = "Number";
    }

    pub mod property {
        pub const name: &str = "__name__";
    }
}

macro define_builtins(
    $Builtins:ident {
        $(
            $name:ident,
        )+
    }
) {
    #[allow(non_snake_case)]
    pub struct $Builtins {
        $(
            pub $name: ObjectRef,
        )+
    }

    impl Default for $Builtins {
        fn default() -> Self {
            Self {
                $(
                    $name: Object::new_dummy(),
                )+
            }
        }
    }
}

define_builtins!(Builtins {
    Class,
    String,
    NilClass,
    Bool,
    Number,
    Main,
    bool_true,
    bool_false,
    nil,
});

#[allow(non_snake_case)]
impl Runtime {
    pub(crate) fn bootstrap(&mut self) {
        self.bootstrap_classes_and_objects();
        self.bootstrap_std_methods();
    }

    fn bootstrap_classes_and_objects(&mut self) {
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

        // create nil
        self.builtins.NilClass = self.create_class(builtin::class::NilClass.into());
        self.builtins.nil = self.create_object(self.builtins.NilClass.clone());

        // create booleans
        self.builtins.Bool = self.create_class(builtin::class::Bool.into());
        self.builtins.bool_true = self.create_object(self.builtins.Bool.clone());
        self.builtins.bool_true.borrow_mut().primitive = Some(Primitive::Boolean(true));
        self.builtins.bool_false = self.create_object(self.builtins.Bool.clone());
        self.builtins.bool_false.borrow_mut().primitive = Some(Primitive::Boolean(false));

        // create number
        self.builtins.Number = self.create_class(builtin::class::Number.into());

        // create main
        self.builtins.Main = self.create_class(builtin::class::Main.into());
        let main = self.create_object(self.builtins.Main.clone());
        let global_scope = self.stack.front_mut().unwrap();
        global_scope.receiver = Some(main);
        global_scope.class = Some(self.builtins.Main.clone());
    }

    fn bootstrap_std_methods(&mut self) {
        self.builtins
            .Main
            .borrow_mut()
            .define_method(
                "print".into(),
                vec![Param::Vararg("args".into())],
                MethodBody::System(|runtime, _this, args| {
                    let arg_count = args.len();
                    for (i, arg) in args.into_iter().enumerate() {
                        let arg_borrowed = arg.borrow();
                        let arg_class = arg_borrowed.class.as_ref().unwrap();
                        if arg_class == &runtime.builtins.String {
                            let Some(Primitive::String(string)) = &arg_borrowed.primitive else {
                                unreachable!();
                            };
                            print!("{string}")
                        } else if arg_class == &runtime.builtins.Number {
                            let Some(Primitive::Number(value)) = &arg_borrowed.primitive else {
                                unreachable!();
                            };
                            print!("{value}")
                        } else if arg == runtime.builtins.nil {
                            print!("nil")
                        } else {
                            unimplemented!();
                        }
                        if i < arg_count - 1 {
                            print!(" ");
                        }
                    }
                    println!();
                    runtime.nil()
                }),
            )
            .unwrap();
    }
}
