use crate::runtime::object::{MethodBody, Object, ObjectRef, Param};
use crate::runtime::Error::ArityMismatch;
use crate::runtime::Runtime;
use crate::types::{Primitive, RcString};
use std::ops::Add;

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
        pub const __name__: &str = "__name__";
        pub const __class__: &str = "__class__";
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

macro replace_expr($_t:tt $sub:expr) {
    $sub
}

macro count($($tts:tt)*) {0usize $(+ replace_expr!($tts 1usize))*}

macro define_system_methods(
    [class = $class:expr, runtime = $runtime:ident, method_name=$method_name:ident, this = $this:ident]
    $(
        fn $name:ident($($param:ident),*) $body:tt
    )+
) {
    {
        let mut class_mut = $class.borrow_mut();
        $(
            let params = vec![$(
                Param::Positional(stringify!($param).into()),
            )*];
            class_mut.define_method(
                stringify!($name).into(),
                params,
                MethodBody::System(|$runtime, $this, $method_name, args| {
                    let arg_count = args.len();
                    let Ok([$($param,)*]) = <[ObjectRef; count!($($param)*)]>::try_from(args) else {
                        return Err(ArityMismatch {
                            class_name: $this.borrow().__class__().borrow().__name__().unwrap(),
                            method_name: $method_name,
                            expected: count!($($param)*),
                            actual: arg_count,
                        });
                    };
                    Ok($body)
                })
            ).unwrap();
        )+
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
        self.bootstrap_stdlib();
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
            .set_property(builtin::property::__name__.into(), name_Class_obj);
        self.builtins
            .String
            .borrow_mut()
            .set_property(builtin::property::__name__.into(), name_String_obj);
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
        let root_frame = &mut self.stack[0];
        root_frame.receiver = Some(main);
        root_frame.class = Some(self.builtins.Main.clone());
    }

    fn bootstrap_stdlib(&mut self) {
        define_system_methods!(
            [class=self.builtins.Number, runtime=runtime, method_name=method_name, this=this]
            fn round() {
                let result = this.borrow().number().unwrap().round();
                runtime.create_number(result)
            }

            fn ceil() {
                let result = this.borrow().number().unwrap().ceil();
                runtime.create_number(result)
            }

            fn floor() {
                let result = this.borrow().number().unwrap().floor();
                runtime.create_number(result)
            }

            fn pow(power) {
                let power = power.borrow().number().unwrap();
                let result = this.borrow().number().unwrap().powf(power);
                runtime.create_number(result)
            }
        );
        define_system_methods!(
            [class=self.builtins.String, runtime=runtime, method_name=method_name, this=this]
            fn trim() {
                let result = this.borrow().string().unwrap().trim().into();
                runtime.create_string(result)
            }

            fn concat(other) {
                let me = this.borrow().string().unwrap();
                let other: RcString = other.borrow().string().unwrap();
                let result = String::from(me.as_ref()).add(other.as_ref()).into();
                runtime.create_string(result)
            }
        );
        self.builtins
            .Main
            .borrow_mut()
            .define_method(
                "print".into(),
                vec![Param::Vararg("args".into())],
                MethodBody::System(|runtime, _this, _method_name, args| {
                    let arg_count = args.len();
                    for (i, arg) in args.into_iter().enumerate() {
                        let arg_borrowed = arg.borrow();
                        let arg_class = arg_borrowed.__class__();
                        if arg_class == runtime.builtins.String {
                            print!("{}", arg_borrowed.string().unwrap());
                        } else if arg_class == runtime.builtins.Number {
                            print!("{}", arg_borrowed.number().unwrap());
                        } else if arg_class == runtime.builtins.Bool {
                            print!("{}", arg_borrowed.bool().unwrap());
                        } else if arg == runtime.builtins.nil {
                            print!("nil");
                        } else if arg_class == runtime.builtins.Class {
                            print!("{}", arg_borrowed.__name__().unwrap());
                        }
                        if i < arg_count - 1 {
                            print!(" ");
                        }
                    }
                    println!();
                    Ok(runtime.nil())
                }),
            )
            .unwrap();
    }
}
