use std::collections::HashMap;

use crate::runtime::object::{MethodBody, MethodReceiver, Object, ObjectRef, Param, Primitive};
use crate::runtime::Error::{ArityMismatch, IllegalConstructorCall, Index, TypeMismatch};
use crate::runtime::{builtin, Result, Runtime, StackFrame};

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
    [runtime=$runtime:ident, method_name=$method_name:ident, this=$this:ident]
    $(
        impl $class:expr => {
            $(
                fn $name:ident($($param:ident),*) $body:tt
            )*
        }
    )+
) {
    $(
        #[allow(unreachable_code, unused_variables)]
        {
            let mut class_mut = $class.borrow_mut();
            $(
                let params = vec![$(
                    Param::Positional(stringify!($param).into()),
                )*];
                #[allow(dead_code)]
                fn $name() {}
                class_mut.define_method(
                    MethodReceiver::Instance,
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
    )+
}

define_builtins!(Builtins {
    Class,
    Object,
    String,
    NilClass,
    Bool,
    Number,
    Array,
    Tuple,
    Dictionary,
    DictionaryIter,
    IO,
    Main,
    bool_true,
    bool_false,
    nil,
});

fn object_list_to_string(
    runtime: &mut Runtime,
    objects: impl IntoIterator<Item = ObjectRef>,
) -> Result<String> {
    let strings: Vec<_> = objects
        .into_iter()
        .map(|object| {
            runtime
                .call_instance_method(object, builtin::method::to_s, None, None)
                .map(|string| string.borrow().string().unwrap().to_string())
        })
        .try_collect()?;
    Ok(strings.join(", "))
}

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
            .set_property(builtin::property::__name__, name_Class_obj);
        self.builtins
            .String
            .borrow_mut()
            .set_property(builtin::property::__name__, name_String_obj);

        let name_Object: String = builtin::class::Object.into();
        self.builtins.Object = self.create_class(name_Object.clone(), None);
        self.assign_global(name_Object, self.builtins.Object.clone());
        self.builtins.String.borrow_mut().superclass = Some(self.builtins.Object.clone());
        // now we can create simple classes

        // create nil
        self.builtins.NilClass = self.create_simple_class(builtin::class::NilClass);
        self.builtins.nil = self.create_object(self.builtins.NilClass.clone());

        // create Array
        self.builtins.Array = self.create_simple_class(builtin::class::Array);

        // create Tuple
        self.builtins.Tuple = self.create_simple_class(builtin::class::Tuple);

        // create Dictionary
        self.builtins.Dictionary = self.create_simple_class(builtin::class::Dictionary);
        self.builtins.DictionaryIter = self.create_simple_class(builtin::class::DictionaryIter);

        // create booleans
        self.builtins.Bool = self.create_simple_class(builtin::class::Bool);
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
        self.builtins.Number = self.create_simple_class(builtin::class::Number);

        // create main
        self.builtins.Main = self.create_simple_class(builtin::class::Main);
        let root_frame = &mut self.stack[0];
        root_frame.class = Some(self.builtins.Main.clone());
        root_frame.open_classes.push(self.builtins.Main.clone());

        self.builtins.IO = self.create_simple_class(builtin::class::IO);
    }

    fn bootstrap_stdlib(&mut self) {
        define_system_methods!(
            [runtime=runtime, method_name=method_name, this=this]

            impl self.builtins.Number => {
                fn init() {
                    this.borrow_mut().set_primitive(Primitive::Number(Default::default()));
                    this
                }

                fn __eq__(other) {
                    if other.borrow().__class__() != runtime.builtins.Number {
                        return Ok(runtime.create_bool(false));
                    }
                    let result = this.borrow().number().unwrap() == other.borrow().number().unwrap();
                    runtime.create_bool(result)
                }

                fn __neq__(other) {
                    if other.borrow().__class__() != runtime.builtins.Number {
                        return Ok(runtime.create_bool(true));
                    }
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
                    runtime.create_string(this.borrow().number().unwrap().to_string())
                }
            }
            impl self.builtins.String => {
                fn init() {
                    this.borrow_mut().set_primitive(Primitive::String("".into()));
                    this
                }

                fn trim() {
                    let this_ref = this.borrow();
                    let string = this_ref.string().unwrap();
                    let result = string.trim();
                    runtime.create_string(result)
                }

                fn __eq__(other) {
                    let this_ref = this.borrow();
                    if other.borrow().class != this_ref.class {
                        return Ok(runtime.builtins.bool_false.clone());
                    }
                    let result = this_ref.string().unwrap() == other.borrow().string().unwrap();
                    runtime.create_bool(result)
                }

                fn __neq__(other) {
                    let this_ref = this.borrow();
                    let result = this_ref.string().unwrap() != other.borrow().string().unwrap();
                    runtime.create_bool(result)
                }

                fn __add__(other) {
                    let other_string = runtime.call_instance_method(
                        other.clone(),
                        builtin::method::to_s,
                        None,
                        None,
                    )?;
                    let other_string_ref = other_string.borrow();
                    let other_string = other_string_ref.string().unwrap();
                    let mut result = this.borrow().string().unwrap().clone();
                    result.push_str(other_string);
                    runtime.create_string(result)
                }

                fn to_s() {
                    this
                }
            }
            impl self.builtins.Object => {
                fn __debug__() {
                    runtime.create_string(this.borrow().__debug__())
                }

                fn __eq__(other) {
                    runtime.create_bool(this == other)
                }

                fn __neq__(other) {
                    runtime.create_bool(this != other)
                }

                fn to_s() {
                    runtime.create_string("Object()")
                }
            }
            impl self.builtins.NilClass => {
                fn init() {
                    return Err(IllegalConstructorCall {
                        class: this.borrow().__class__().borrow().__name__().unwrap(),
                    });
                }

                fn to_s() {
                    runtime.create_string("nil")
                }
            }
            impl self.builtins.Bool => {
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
                    runtime.create_string(this.borrow().bool().unwrap().to_string())
                }
            }
            impl self.builtins.Array => {
                fn to_s() {
                    let this_ref = this.borrow();
                    let elements = this_ref.array().unwrap();
                    let inner = object_list_to_string(
                        runtime,
                        elements.iter().cloned(),
                    )?;
                    runtime.create_string(format!("[{inner}]"))
                }

                fn __index__(index) {
                    if index.borrow().__class__() != runtime.builtins.Number {
                        return Err(TypeMismatch {
                            class: index.borrow().__class__().borrow().__name__().unwrap(),
                            expected: builtin::class::Number.into(),
                        });
                    }
                    let this_ref = this.borrow();
                    let elements = this_ref.array().unwrap();
                    if elements.is_empty() {
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

                fn __add__(other) {
                    if other.borrow().__class__() != runtime.builtins.Array {
                        return Err(TypeMismatch {
                            class: other.borrow().__class__().borrow().__name__().unwrap(),
                            expected: builtin::class::Array.into(),
                        });
                    }
                    let [mut arr1, arr2] = [this, other].map(|obj| obj.borrow().array().unwrap().clone());
                    arr1.extend(arr2);
                    runtime.create_array(arr1)
                }

                fn init() {
                    this.borrow_mut().set_primitive(Primitive::Array(Default::default()));
                    this
                }

                fn push(element) {
                    this.borrow_mut().array_mut().unwrap().push(element);
                    runtime.nil()
                }

                fn pop() {
                    let mut this_ref = this.borrow_mut();
                    let elements = this_ref.array_mut().unwrap();
                    elements.pop().ok_or(Index {
                        error: "pop from empty list",
                    })?
                }

                fn len() {
                    let this_ref = this.borrow();
                    let elements = this_ref.array().unwrap();
                    runtime.create_number(elements.len() as _)
                }
            }

            impl self.builtins.Class => {
                fn to_s() {
                    runtime.create_string(this.borrow().__name__().unwrap())
                }
            }

            impl self.builtins.Dictionary => {
                fn init() {
                    this.borrow_mut().set_primitive(Primitive::Dictionary(HashMap::default()));
                    this
                }

                fn __index__(key) {
                    let key_ref = key.borrow();
                    let key_class = key_ref.__class__();
                    if key_class != runtime.builtins.String {
                        return Err(TypeMismatch {
                            class: key_class.borrow().__name__().unwrap(),
                            expected: "String".into(),
                        });
                    }
                    let key_string: &String = key_ref.string().unwrap();
                    let this_ref = this.borrow();
                    let dict = this_ref.dictionary().unwrap();
                    dict.get(key_string).cloned().unwrap_or_else(|| runtime.nil())
                }

                fn __set_index__(key, value) {
                    let key_ref = key.borrow();
                    let key_class = key_ref.__class__();
                    if key_class != runtime.builtins.String {
                        return Err(TypeMismatch {
                            class: key_class.borrow().__name__().unwrap(),
                            expected: "String".into(),
                        });
                    }
                    let key_string: &String = key_ref.string().unwrap();
                    let mut this_ref = this.borrow_mut();
                    let dict = this_ref.dictionary_mut().unwrap();
                    dict.insert(key_string.clone(), value);
                    runtime.nil()
                }

                fn to_s() {
                    let this_ref = this.borrow();
                    let dict = this_ref.dictionary().unwrap();
                    let entries: Vec<_> = dict
                        .iter()
                        .map(|(key, value)| {
                            let value_obj = runtime.call_instance_method(
                                value.clone(),
                                builtin::method::to_s,
                                None,
                                None,
                            )?;
                            let value_ref = value_obj.borrow();
                            let value = value_ref.string().unwrap();
                            Ok(format!("    {key}: {value},"))
                        })
                        .try_collect()?;
                    let inner = if entries.is_empty() { ":".to_string() } else { format!("\n{}\n", entries.join("\n")) };
                    runtime.create_string(format!("[{inner}]"))
                }
            }

            impl self.builtins.Tuple => {
                fn to_s() {
                    let this_ref = this.borrow();
                    let items = this_ref.array().unwrap();
                    let mut inner = object_list_to_string(
                        runtime,
                        items.iter().cloned(),
                    )?;
                    if items.len() == 1 {
                        inner.push_str(",");
                    }
                    runtime.create_string(format!("({inner})"))
                }
            }
        );

        self.builtins
            .IO
            .borrow_mut()
            .define_method(
                MethodReceiver::Class,
                "print".into(),
                vec![Param::Vararg("args".into())],
                MethodBody::System(|runtime, _this, _method_name, args| {
                    runtime.print_objects(args)?;
                    Ok(runtime.nil())
                }),
            )
            .unwrap();

        self.builtins
            .IO
            .borrow_mut()
            .define_method(
                MethodReceiver::Class,
                "println".into(),
                vec![Param::Vararg("args".into())],
                MethodBody::System(|runtime, _this, _method_name, args| {
                    runtime.print_objects(args)?;
                    println!();
                    Ok(runtime.nil())
                }),
            )
            .unwrap();

        self.builtins
            .IO
            .borrow_mut()
            .define_method(
                MethodReceiver::Class,
                "debug".into(),
                vec![Param::Vararg("args".into())],
                MethodBody::System(|runtime, _this, _method_name, args| {
                    print!(">>> ");
                    runtime.print_objects(args)?;
                    println!();
                    Ok(runtime.nil())
                }),
            )
            .unwrap();
    }

    fn print_objects(&mut self, args: Vec<ObjectRef>) -> Result<()> {
        let arg_count = args.len();
        for (i, arg) in args.into_iter().enumerate() {
            let string_obj = self.call_instance_method(arg, builtin::method::to_s, None, None)?;
            let string = string_obj.borrow().string().cloned().ok_or(TypeMismatch {
                class: string_obj.borrow().__class__().borrow().__name__().unwrap(),
                expected: builtin::class::String.into(),
            })?;
            print!("{}", string);
            if i < arg_count - 1 {
                print!(" ");
            }
        }
        Ok(())
    }
}
