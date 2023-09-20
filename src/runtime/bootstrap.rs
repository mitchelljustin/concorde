use std::ops::Add;

use crate::runtime::object::{MethodBody, Object, ObjectRef, Param, Primitive};
use crate::runtime::Error::{ArityMismatch, IllegalConstructorCall, IndexError, TypeError};
use crate::runtime::{builtin, Runtime, StackFrame};

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
                            method_name: $method_name.into(),
                            expected: count!($($param)*),
                            actual: arg_count,
                        });
                    };
                    Ok($body)
                }),
            ).unwrap();
        )+
    }
}

define_builtins!(Builtins {
    Class,
    Object,
    String,
    NilClass,
    Bool,
    Number,
    Array,
    IO,
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
        self.stack.push(StackFrame::default());
        self.builtins.Class = Object::new_dummy();
        let Class = self.builtins.Class.clone();
        Class.borrow_mut().class = Some(Class.clone());
        // now we can create classes
        self.all_objects.push(Class.borrow().weak_self());
        let name_Class: String = builtin::class::Class.into();
        self.assign_global(name_Class.clone(), Class.clone());

        self.builtins.String = self.create_object(Class.clone());
        let name_String: String = builtin::class::String.into();
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

        let name_Object: String = builtin::class::Object.into();
        self.builtins.Object = self.create_class(name_Object.clone(), None);
        self.assign_global(name_Object, self.builtins.Object.clone());
        self.builtins.String.borrow_mut().superclass = Some(self.builtins.Object.clone());
        // now we can create simple classes

        // create nil
        self.builtins.NilClass = self.create_simple_class(builtin::class::NilClass.into());
        self.builtins.nil = self.create_object(self.builtins.NilClass.clone());

        // create Array
        self.builtins.Array = self.create_simple_class(builtin::class::Array.into());

        // create booleans
        self.builtins.Bool = self.create_simple_class(builtin::class::Bool.into());
        self.builtins.bool_true = self.create_object(self.builtins.Bool.clone());
        self.builtins
            .bool_true
            .borrow_mut()
            .set_primitive(Primitive::Boolean(true));
        self.builtins.bool_false = self.create_object(self.builtins.Bool.clone());
        self.builtins
            .bool_false
            .borrow_mut()
            .set_primitive(Primitive::Boolean(false));

        // create number
        self.builtins.Number = self.create_simple_class(builtin::class::Number.into());

        // create main
        self.builtins.Main = self.create_simple_class(builtin::class::Main.into());
        self.stack[0].class = Some(self.builtins.Main.clone());

        self.builtins.IO = self.create_simple_class(builtin::class::IO.into());
    }

    fn bootstrap_stdlib(&mut self) {
        define_system_methods!(
            [class=self.builtins.Number, runtime=runtime, method_name=method_name, this=this]
            fn init() {
                this.borrow_mut().set_primitive(Primitive::Number(Default::default()));
                this
            }

            fn __eq__(other) {
                let result = this.borrow().number().unwrap() == other.borrow().number().unwrap();
                runtime.create_bool(result)
            }

            fn __neq__(other) {
                let result = this.borrow().number().unwrap() != other.borrow().number().unwrap();
                runtime.create_bool(result)
            }

            fn __lt__(other) {
                let result = this.borrow().number().unwrap() < other.borrow().number().unwrap();
                runtime.create_bool(result)
            }

            fn __lte__(other) {
                let result = this.borrow().number().unwrap() <= other.borrow().number().unwrap();
                runtime.create_bool(result)
            }

            fn __gt__(other) {
                let result = this.borrow().number().unwrap() > other.borrow().number().unwrap();
                runtime.create_bool(result)
            }

            fn __gte__(other) {
                let result = this.borrow().number().unwrap() >= other.borrow().number().unwrap();
                runtime.create_bool(result)
            }

            fn __add__(other) {
                let result = this.borrow().number().unwrap() + other.borrow().number().unwrap();
                runtime.create_number(result)
            }

            fn __sub__(other) {
                let result = this.borrow().number().unwrap() - other.borrow().number().unwrap();
                runtime.create_number(result)
            }

            fn __mul__(other) {
                let result = this.borrow().number().unwrap() * other.borrow().number().unwrap();
                runtime.create_number(result)
            }

            fn __div__(other) {
                let result = this.borrow().number().unwrap() / other.borrow().number().unwrap();
                runtime.create_number(result)
            }

            fn __neg__() {
                let result = - this.borrow().number().unwrap();
                runtime.create_number(result)
            }

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

            fn to_s() {
                runtime.create_string(this.borrow().number().unwrap().to_string().into())
            }
        );
        define_system_methods!(
            [class=self.builtins.String, runtime=runtime, method_name=method_name, this=this]
            fn init() {
                this.borrow_mut().set_primitive(Primitive::String("".into()));
                this
            }

            fn trim() {
                let result = this.borrow().string().unwrap().trim().into();
                runtime.create_string(result)
            }

            fn __eq__(other) {
                let result = this.borrow().string().unwrap() == other.borrow().string().unwrap();
                runtime.create_bool(result)
            }

            fn __neq__(other) {
                let result = this.borrow().string().unwrap() != other.borrow().string().unwrap();
                runtime.create_bool(result)
            }

            fn __add__(other) {
                if other.borrow().__class__() != runtime.builtins.String {
                    return Err(TypeError {
                        expected: builtin::class::String.into(),
                        class: other.borrow().__class__().borrow().__name__().unwrap(),
                    });
                }
                let mut me = this.borrow().string().unwrap();
                let other = other.borrow().string().unwrap();
                let result = me.add(&other);
                runtime.create_string(result)
            }

            fn push(other) {
                if other.borrow().__class__() != runtime.builtins.String {
                    return Err(TypeError {
                        expected: builtin::class::String.into(),
                        class: other.borrow().__class__().borrow().__name__().unwrap(),
                    });
                }
                let Some(Primitive::String(string)) = &mut this.borrow_mut().primitive else {
                    unreachable!();
                };
                let other = other.borrow().string().unwrap();
                string.push_str(&other);
                runtime.nil()
            }

            fn to_s() {
                this
            }
        );
        define_system_methods!(
            [class=self.builtins.Object, runtime=runtime, method_name=method_name, this=this]
            fn __debug__() {
                runtime.create_string(this.borrow().__debug__())
            }

            fn to_s() {
                runtime.create_string("Object()".into())
            }
        );
        define_system_methods!(
            [class=self.builtins.NilClass, runtime=runtime, method_name=method_name, this=this]
            fn init() {
                return Err(IllegalConstructorCall {
                    class: this.borrow().__class__().borrow().__name__().unwrap(),
                });
            }

            fn to_s() {
                runtime.create_string("nil".into())
            }
        );
        define_system_methods!(
            [class=self.builtins.Bool, runtime=runtime, method_name=method_name, this=this]
            fn __not__() {
                if this == runtime.builtins.bool_false {
                    runtime.builtins.bool_true.clone()
                } else {
                    runtime.builtins.bool_false.clone()
                }
            }

            fn init() {
                this.borrow_mut().set_primitive(Primitive::Boolean(Default::default()));
                this
            }

            fn to_s() {
                runtime.create_string(this.borrow().bool().unwrap().to_string().into())
            }
        );
        define_system_methods!(
            [class=self.builtins.Array, runtime=runtime, method_name=method_name, this=this]
            fn to_s() {
                let elements = this.borrow().array().unwrap();
                let strings = elements
                    .into_iter()
                    .map(|object|
                        runtime
                            .call_instance_method(
                                object,
                                builtin::method::to_s,
                                None,
                                None,
                            )
                            .map(|string| string.borrow().string().unwrap().to_string()))
                    .collect::<Result<Vec<String>, _>>()?;
                let inner = strings.join(", ");
                runtime.create_string(format!("[{inner}]").into())
            }

            fn __index__(index) {
                if index.borrow().__class__() != runtime.builtins.Number {
                    return Err(TypeError {
                        class: index.borrow().__class__().borrow().__name__().unwrap(),
                        expected: builtin::class::Number.into(),
                    });
                }
                let elements = this.borrow().array().unwrap();
                if elements.len() == 0 {
                    return Ok(runtime.nil());
                }
                let index = index.borrow().number().unwrap() as isize;
                let index = if index < 0 {
                    index.rem_euclid(elements.len() as isize)
                } else {
                    index
                } as usize;
                if index >= elements.len() {
                    return Ok(runtime.nil());
                }
                elements[index].clone()
            }

            fn init() {
                this.borrow_mut().set_primitive(Primitive::Array(Default::default()));
                this
            }

            fn push(element) {
                let mut this_ref = this.borrow_mut();
                let Some(Primitive::Array(elements)) = &mut this_ref.primitive else {
                    unreachable!();
                };
                elements.push(element);
                runtime.nil()
            }

            fn pop() {
                let mut this_ref = this.borrow_mut();
                let Some(Primitive::Array(elements)) = &mut this_ref.primitive else {
                    unreachable!();
                };
                elements.pop().ok_or(IndexError {
                    error: "pop from empty list",
                })?
            }
        );

        define_system_methods!(
            [class=self.builtins.Class, runtime=runtime, method_name=method_name, this=this]
            fn to_s() {
                runtime.create_string(this.borrow().__name__().unwrap())
            }
        );

        self.builtins
            .IO
            .borrow_mut()
            .define_method(
                "print".into(),
                vec![Param::Vararg("args".into())],
                MethodBody::System(|runtime, _this, _method_name, args| {
                    let arg_count = args.len();
                    for (i, arg) in args.into_iter().enumerate() {
                        let string_obj =
                            runtime.call_instance_method(arg, builtin::method::to_s, None, None)?;
                        print!("{}", string_obj.borrow().string().unwrap());
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
