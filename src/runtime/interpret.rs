use crate::runtime::bootstrap::builtin;
use crate::runtime::object::{MethodBody, ObjectRef, Param};
use crate::runtime::Error::{
    ArityMismatch, IllegalAssignmentTarget, NoSuchMethod, NotCallable, UndefinedProperty,
};
use crate::runtime::Runtime;
use crate::runtime::{Result, StackFrame};
use crate::types::{
    Access, Block, Call, Expression, LValue, Literal, MethodDefinition, Node, Program, RcString,
    Statement,
};

impl Runtime {
    pub fn exec_program(&mut self, program: Node<Program>) -> Result<()> {
        for statement in program.v.body.v.statements {
            self.exec(statement)?;
        }
        Ok(())
    }

    pub fn exec(&mut self, statement: Node<Statement>) -> Result<()> {
        match statement.v {
            Statement::Expression(expression) => {
                self.eval(expression)?;
            }
            Statement::MethodDefinition(method_def) => {
                self.exec_method_def(self.current_class(), method_def)?;
            }
            Statement::Assignment(assignment) => {
                let value = self.eval(assignment.v.value);
                match assignment.v.target.v {
                    LValue::Variable(var) => {
                        self.assign(var.ident.name.clone(), value?);
                    }
                    LValue::Access(access) => {
                        let target = self.eval(*access.target.clone())?;
                        let Expression::Variable(member) = access.v.member.v else {
                            return Err(IllegalAssignmentTarget {
                                target: access.target.meta.source.clone().into(),
                                member: access.member.meta.source.clone().into(),
                            });
                        };
                        target
                            .borrow_mut()
                            .set_property(member.ident.name.clone(), value?);
                    }
                }
            }
            Statement::ClassDefinition(class_def) => {
                let class = self.create_class(class_def.v.name.v.name);
                self.stack.push(StackFrame {
                    class: Some(class),
                    ..StackFrame::default()
                });
                let mut result = Ok(());
                for statement in class_def.v.body {
                    if let Err(error) = self.exec(statement) {
                        result = Err(error);
                        break;
                    };
                }
                self.stack.pop();
                return result;
            }
            node => unimplemented!("{node:?}"),
        };
        Ok(())
    }

    fn exec_method_def(
        &mut self,
        class: ObjectRef,
        method_def: Node<MethodDefinition>,
    ) -> Result<()> {
        let method_name = method_def.name.name.clone();
        let params = method_def
            .parameters
            .iter()
            .map(|param| Param::Positional(param.name.name.clone()))
            .collect();
        let body = MethodBody::User(method_def.v.body);
        class
            .borrow_mut()
            .define_method(method_name, params, body)?;
        Ok(())
    }

    pub fn eval(&mut self, expression: Node<Expression>) -> Result<ObjectRef> {
        match expression.v {
            Expression::Variable(var) => self.resolve(var.ident.name.as_ref()),
            Expression::Call(call) => {
                let (method_name, arguments) = self.eval_call(call)?;
                self.perform_call(self.current_receiver(), method_name, arguments)
            }
            Expression::Literal(literal) => self.eval_literal(literal),
            Expression::Access(access) => self.eval_access(access),
            Expression::IfElse(if_else) => {
                let condition = self.eval(*if_else.v.condition)?;
                if condition == self.builtins.bool_false || condition == self.builtins.nil {
                    return if let Some(else_body) = if_else.v.else_body {
                        self.eval_block(else_body)
                    } else {
                        Ok(self.nil())
                    };
                }
                self.eval_block(if_else.v.then_body)
            }
            Expression::Binary(binary) => {
                let lhs = self.eval(*binary.v.lhs)?;
                let rhs = self.eval(*binary.v.rhs)?;
                let method_name = builtin::op::method_for_binary_op(&binary.v.op);
                self.perform_call(lhs, method_name.into(), vec![rhs])
            }
        }
    }

    fn eval_call(&mut self, call: Node<Call>) -> Result<(RcString, Vec<ObjectRef>)> {
        let Call { target, arguments } = call.v;
        let arguments = arguments
            .into_iter()
            .map(|argument| self.eval(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let Expression::Variable(var) = target.v else {
            return Err(NotCallable { expr: target.meta });
        };
        let method_name = var.ident.name.clone();
        Ok((method_name, arguments))
    }

    fn eval_access(&mut self, access: Node<Access>) -> Result<ObjectRef> {
        let Access { target, member } = access.v;
        let target = self.eval(*target)?;
        match member.v {
            Expression::Variable(var) => {
                let member = &var.ident.name;
                let value = target.borrow().get_property(member);
                return value.ok_or(UndefinedProperty {
                    target: target.borrow().__debug__(),
                    member: member.clone(),
                    access: access.meta,
                });
            }
            Expression::Call(call) => {
                let (method_name, arguments) = self.eval_call(call)?;
                self.perform_call(target, method_name, arguments)
            }
            _ => unimplemented!(),
        }
    }

    fn perform_call(
        &mut self,
        receiver: ObjectRef,
        method_name: RcString,
        arguments: Vec<ObjectRef>,
    ) -> Result<ObjectRef> {
        if let Ok(class) = self.resolve(&method_name) && self.is_class(&class) {
            let new_instance = self.create_object(class.clone());
            let class_ref = class.borrow();
            if let Some(init_method) = class_ref.resolve_method(builtin::method::init) {
                self.perform_call(
                    new_instance.clone(),
                    init_method.name.clone(),
                    arguments
                )?;
            } else if !arguments.is_empty() {
                return Err(ArityMismatch {
                    method_name: builtin::method::init.into(),
                    class_name: class_ref.__name__().unwrap(),
                    actual: arguments.len(),
                    expected: 0,
                });
            }
            return Ok(new_instance);
        }
        let class = receiver.borrow().__class__();
        let class_ref = class.borrow();
        let main = self.builtins.Main.clone();
        let main_ref = main.borrow();
        let method = if let Some(method) = class_ref.resolve_method(&method_name) {
            method
        } else if let Some(method) = main_ref.resolve_method(&method_name) {
            method
        } else {
            return Err(NoSuchMethod {
                class_name: class_ref.__name__().unwrap_or("".into()),
                method_name: method_name.clone(),
            });
        };
        match &method.body {
            MethodBody::User(body) => {
                if arguments.len() != method.params.len() {
                    return Err(ArityMismatch {
                        expected: method.params.len(),
                        actual: arguments.len(),
                        class_name: class_ref.__name__().unwrap(),
                        method_name: method_name.clone(),
                    });
                }
                let variables = method
                    .params
                    .iter()
                    .zip(arguments.into_iter())
                    .map(|(param, arg)| {
                        let Param::Positional(name) = param else {
                            unimplemented!();
                        };
                        (name.clone(), arg)
                    })
                    .collect();
                self.stack.push(StackFrame {
                    receiver: Some(receiver),
                    method_name: Some(method_name.clone()),
                    variables,
                    ..StackFrame::default()
                });
                let result = self.eval_block(body.clone());
                self.stack.pop();
                result
            }
            MethodBody::System(function) => {
                function(self, receiver.clone(), method_name.clone(), arguments)
            }
        }
    }

    fn is_class(&self, object: &ObjectRef) -> bool {
        object.borrow().__class__() == self.builtins.Class
    }

    fn eval_block(&mut self, block: Node<Block>) -> Result<ObjectRef> {
        let mut retval = self.nil();
        let statement_count = block.statements.len();
        for (i, statement) in block.v.statements.into_iter().enumerate() {
            match statement.v {
                Statement::Expression(expression) if i == statement_count - 1 => {
                    retval = self.eval(expression.clone())?;
                }
                _ => self.exec(statement.clone())?,
            }
        }
        Ok(retval)
    }

    fn eval_literal(&mut self, literal: Node<Literal>) -> Result<ObjectRef> {
        match literal.v {
            Literal::String(string) => Ok(self.create_string(string.v.value)),
            Literal::Number(number) => Ok(self.create_number(number.value)),
            Literal::Boolean(boolean) => Ok(self.create_bool(boolean.value)),
            Literal::Nil(_) => Ok(self.nil()),
        }
    }
}
