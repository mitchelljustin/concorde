use std::ops::ControlFlow;

use crate::runtime::bootstrap::builtin;
use crate::runtime::object::{MethodBody, ObjectRef, Param};
use crate::runtime::Error::{
    ArityMismatch, IllegalAssignmentTarget, NoSuchMethod, NotCallable, UndefinedProperty,
};
use crate::runtime::{Error, Runtime};
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
                        self.assign_variable(var.v.ident.v.name.clone(), value?);
                    }
                    LValue::Access(access) => {
                        let target = self.eval(*access.v.target)?;
                        let Expression::Variable(member) = access.v.member.v else {
                            return Err(IllegalAssignmentTarget {
                                access: access.meta,
                            });
                        };
                        target
                            .borrow_mut()
                            .set_property(member.v.ident.v.name.clone(), value?);
                    }
                    LValue::Index(index) => {
                        let target = self.eval(*index.v.target)?;
                        let index = self.eval(*index.v.index)?;
                        let value = value?;
                        self.perform_call(target, builtin::op::__set_index__, [index, value])?;
                    }
                }
            }
            Statement::ClassDefinition(class_def) => {
                let class = self.create_simple_class(class_def.v.name.v.name);
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
            Statement::ForIn(for_in) => {
                let binding_name = for_in.v.binding.v.ident.v.name;
                let iterator = self.eval(for_in.v.iterator)?;
                self.stack.push(StackFrame::default());
                if iterator.borrow().__class__() != self.builtins.Array {
                    unimplemented!();
                }
                let elements = iterator.borrow().array().unwrap();
                let mut result = Ok(());
                for element in elements {
                    self.define_variable(binding_name.clone(), element);
                    if let Err(err) = self.eval_block(for_in.v.body.clone()) {
                        result = Err(err);
                        break;
                    }
                }
                self.stack.pop();
                return result;
            }
            Statement::WhileLoop(while_loop) => loop {
                let condition = self.eval(while_loop.v.condition.clone())?;
                if self.is_falsy(condition) {
                    break;
                }
                let result = self.eval_block(while_loop.v.body.clone());
                match result {
                    Err(Error::ControlFlow(ControlFlow::Break(()))) => {
                        break;
                    }
                    Err(Error::ControlFlow(ControlFlow::Continue(()))) => {
                        continue;
                    }
                    Err(error) => {
                        return Err(error);
                    }
                    Ok(_) => {}
                }
            },
            Statement::Break(_) => return Err(Error::ControlFlow(ControlFlow::Break(()))),
            Statement::Next(_) => return Err(Error::ControlFlow(ControlFlow::Continue(()))),
        };
        Ok(())
    }

    fn exec_method_def(
        &mut self,
        class: ObjectRef,
        method_def: Node<MethodDefinition>,
    ) -> Result<()> {
        let method_name = method_def.v.name.v.name.clone();
        let params = method_def
            .v
            .parameters
            .iter()
            .map(|param| Param::Positional(param.v.name.v.name.clone()))
            .collect();
        let body = MethodBody::User(method_def.v.body);
        class
            .borrow_mut()
            .define_method(method_name, params, body)?;
        Ok(())
    }

    pub fn eval(&mut self, expression: Node<Expression>) -> Result<ObjectRef> {
        match expression.v {
            Expression::Variable(var) => self.resolve(var.v.ident.v.name.as_ref()),
            Expression::Call(call) => {
                let (method_name, arguments) = self.eval_call_parts(call)?;
                self.perform_call(self.current_receiver(), &method_name, arguments)
            }
            Expression::Literal(literal) => self.eval_literal(literal),
            Expression::Access(access) => self.eval_access(access),
            Expression::IfElse(if_else) => {
                let condition = self.eval(*if_else.v.condition)?;
                if self.is_falsy(condition) {
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
                let method_name = builtin::op::method_for_binary_op(&binary.v.op.v);
                self.perform_call(lhs, method_name, [rhs])
            }
            Expression::Index(index) => {
                let target = self.eval(*index.v.target)?;
                let index = self.eval(*index.v.index)?;
                self.perform_call(target, builtin::op::__index__, [index])
            }
            Expression::Unary(unary) => {
                let rhs = self.eval(*unary.v.rhs)?;
                let method_name = builtin::op::method_for_unary_op(&unary.v.op.v).unwrap();
                self.perform_call(rhs, method_name, None)
            }
        }
    }

    fn is_falsy(&self, condition: ObjectRef) -> bool {
        condition == self.builtins.bool_false || condition == self.builtins.nil
    }

    fn eval_call_parts(&mut self, call: Node<Call>) -> Result<(RcString, Vec<ObjectRef>)> {
        let arguments = call
            .v
            .arguments
            .into_iter()
            .map(|argument| self.eval(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let Expression::Variable(var) = call.v.target.v else {
            return Err(NotCallable {
                expr: call.v.target.meta,
            });
        };
        let method_name = var.v.ident.v.name.clone();
        Ok((method_name, arguments))
    }

    fn eval_access(&mut self, access: Node<Access>) -> Result<ObjectRef> {
        let Access { target, member } = access.v;
        let target = self.eval(*target)?;
        match member.v {
            Expression::Variable(var) => {
                let member = &var.v.ident.v.name;
                let value = target.borrow().get_property(member);
                return value.ok_or(UndefinedProperty {
                    target: target.borrow().__debug__(),
                    member: member.clone(),
                    access: access.meta,
                });
            }
            Expression::Call(call) => {
                let (method_name, arguments) = self.eval_call_parts(call)?;
                self.perform_call(target, &method_name, arguments)
            }
            _ => unimplemented!(),
        }
    }

    pub fn perform_call(
        &mut self,
        mut receiver: ObjectRef,
        method_name: &str,
        arguments: impl IntoIterator<Item = ObjectRef>,
    ) -> Result<ObjectRef> {
        let method;
        let class;
        let is_init;
        if let Ok(class_var) = self.resolve(method_name) && self.is_class(&class_var) {
            class = class_var;
            receiver = self.create_object(class.clone());
            let Some(init_method) = class.borrow().resolve_method(builtin::method::init) else {
                return Ok(receiver);
            };
            method = init_method;
            is_init = true;
        } else {
            class = receiver.borrow().__class__();
            method = if let Some(method) = class.borrow().resolve_method(method_name) {
                method
            } else if let Some(method) = self.builtins.Main.borrow().resolve_method(method_name) {
                method
            } else {
                return Err(NoSuchMethod {
                    class_name: class.borrow().__name__().unwrap(),
                    method_name: method_name.into(),
                });
            };
            is_init = false;
        }
        let arguments: Vec<ObjectRef> = arguments.into_iter().collect();
        match &method.body {
            MethodBody::User(body) => {
                if arguments.len() != method.params.len() {
                    return Err(ArityMismatch {
                        expected: method.params.len(),
                        actual: arguments.len(),
                        class_name: class.borrow().__name__().unwrap(),
                        method_name: method_name.into(),
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
                    receiver: Some(receiver.clone()),
                    method_name: Some(method_name.into()),
                    variables,
                    ..StackFrame::default()
                });
                let result = self.eval_block(body.clone());
                self.stack.pop();
                if is_init {
                    result?;
                    Ok(receiver)
                } else {
                    result
                }
            }
            MethodBody::System(function) => {
                function(self, receiver.clone(), method_name, arguments)
            }
        }
    }

    fn is_class(&self, object: &ObjectRef) -> bool {
        object.borrow().__class__() == self.builtins.Class
    }

    fn eval_block(&mut self, block: Node<Block>) -> Result<ObjectRef> {
        let mut retval = self.nil();
        let statement_count = block.v.statements.len();
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
            Literal::Number(number) => Ok(self.create_number(number.v.value)),
            Literal::Boolean(boolean) => Ok(self.create_bool(boolean.v.value)),
            Literal::Array(array) => {
                let elements = array
                    .v
                    .elements
                    .into_iter()
                    .map(|node| self.eval(node))
                    .collect::<Result<_, _>>()?;
                Ok(self.create_array(elements))
            }
            Literal::Nil(_) => Ok(self.nil()),
        }
    }
}
