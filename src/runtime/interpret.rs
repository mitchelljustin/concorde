use std::ops::ControlFlow;

use crate::runtime::bootstrap::builtin;
use crate::runtime::object::{MethodBody, MethodRef, ObjectRef, Param};
use crate::runtime::Error::{
    ArityMismatch, BadPath, IllegalAssignmentTarget, NoSuchMethod, NoSuchVariable, NotAClassMethod,
    NotCallable, UndefinedProperty,
};
use crate::runtime::{Error, Runtime};
use crate::runtime::{Result, StackFrame};
use crate::types::{
    Access, Block, Expression, LValue, Literal, MethodDefinition, Node, Path, Program, Statement,
};

macro handle_loop_control_flow($result:ident) {
    match $result {
        Err(Error::ControlFlow(ControlFlow::Break(()))) => {
            $result = Ok(());
            break;
        }
        Err(Error::ControlFlow(ControlFlow::Continue(()))) => {
            $result = Ok(());
            continue;
        }
        Err(error) => {
            $result = Err(error);
            break;
        }
        Ok(()) => {}
    }
}

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
                    result = self.eval_block(for_in.v.body.clone()).map(|_| ());
                    handle_loop_control_flow!(result);
                }
                self.stack.pop();
                return result;
            }
            Statement::WhileLoop(while_loop) => {
                self.stack.push(StackFrame::default());
                let mut result = Ok(());
                loop {
                    let condition = self.eval(while_loop.v.condition.clone())?;
                    if self.is_falsy(condition) {
                        break;
                    }
                    result = self.eval_block(while_loop.v.body.clone()).map(|_| ());
                    handle_loop_control_flow!(result);
                }
                return result;
            }
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
        class.borrow_mut().define_method(
            method_name,
            params,
            body,
            method_def.v.is_class_method,
        )?;
        Ok(())
    }

    pub fn eval(&mut self, expression: Node<Expression>) -> Result<ObjectRef> {
        match expression.v {
            Expression::Variable(var) => {
                let name = &var.v.ident.v.name;
                self.resolve_variable(name).ok_or(NoSuchVariable {
                    name: name.clone(),
                    node: var.meta,
                })
            }
            Expression::Call(call) => {
                let target = call.v.target;
                let (class_receiver, method_name) = match target.v {
                    Expression::Variable(var) => (None, var.v.ident.v.name),
                    Expression::Path(mut path) => {
                        let method_component = path.v.components.pop().unwrap();
                        let method_name = method_component.v.ident.v.name;
                        let receiver = self.resolve_class_from_path(path)?;
                        (Some(receiver), method_name)
                    }
                    _ => return Err(NotCallable { node: target.meta }),
                };
                let arguments = self.eval_expr_list(call.v.arguments)?;
                let receiver = class_receiver.unwrap_or_else(|| self.current_receiver());
                self.perform_call(receiver, &method_name, arguments)
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
            Expression::Path(path) => self.resolve_class_from_path(path),
        }
    }

    fn resolve_class_from_path(&self, path: Node<Path>) -> Result<ObjectRef> {
        let (start_class, class_components) = path.v.components.split_first().unwrap();
        let receiver_name = &start_class.v.ident.v.name;
        let mut receiver = self.resolve_variable(receiver_name).ok_or(NoSuchVariable {
            name: receiver_name.clone(),
            node: start_class.meta.clone(),
        })?;
        for component in class_components {
            let member = &component.v.ident.v.name;
            let child_receiver =
                receiver
                    .borrow()
                    .get_property(member)
                    .ok_or(UndefinedProperty {
                        target: receiver.borrow().__debug__(),
                        member: member.clone(),
                        node: path.meta.clone(),
                    })?;
            if !self.is_class(&child_receiver) {
                return Err(BadPath {
                    path: path.meta,
                    non_class: member.clone(),
                });
            }
            receiver = child_receiver;
        }
        Ok(receiver)
    }

    fn is_falsy(&self, condition: ObjectRef) -> bool {
        condition == self.builtins.bool_false || condition == self.builtins.nil
    }

    fn eval_access(&mut self, access: Node<Access>) -> Result<ObjectRef> {
        let target = self.eval(*access.v.target)?;
        match access.v.member.v {
            Expression::Variable(var) => {
                let member = &var.v.ident.v.name;
                let value = target.borrow().get_property(member);
                return value.ok_or(UndefinedProperty {
                    target: target.borrow().__debug__(),
                    member: member.clone(),
                    node: access.meta,
                });
            }
            Expression::Call(call) => {
                let arguments = self.eval_expr_list(call.v.arguments)?;
                let Expression::Variable(var) = call.v.target.v else {
                    return Err(NotCallable {
                        node: call.v.target.meta,
                    });
                };
                let method_name = &var.v.ident.v.name;
                self.perform_call(target, method_name, arguments)
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
        if let Some(class_var) = self.resolve_variable(method_name) && self.is_class(&class_var) {
            class = class_var;
            receiver = self.create_object(class.clone());
            let Some(init_method) = class.borrow().resolve_method(builtin::method::init) else {
                let arg_count = arguments.into_iter().count();
                if arg_count > 0 {
                    return Err(ArityMismatch {
                        method_name: builtin::method::init.into(),
                        class_name: class.borrow().__name__().unwrap(),
                        expected: 0,
                        actual: arg_count,
                    });
                }
                return Ok(receiver);
            };
            method = init_method;
            is_init = true;
        } else {
            let receiver_is_class = self.is_class(&receiver);
            if receiver_is_class {
                class = receiver.clone();
            } else {
                class = receiver.borrow().__class__();
            }
            method = self.resolve_method(&class, method_name).ok_or(NoSuchMethod {
                class_name: class.borrow().__name__().unwrap(),
                method_name: method_name.into(),
            })?;
            if receiver_is_class && !method.is_class_method {
                return Err(NotAClassMethod {
                    class_name: class.borrow().__name__().unwrap(),
                    method_name: method_name.into(),
                });
            }
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

    fn resolve_method(&mut self, class: &ObjectRef, method_name: &str) -> Option<MethodRef> {
        if let Some(method) = class.borrow().resolve_method(method_name) {
            return Some(method);
        }
        if let Some(method) = self.builtins.Main.borrow().resolve_method(method_name) {
            return Some(method);
        }
        None
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

    fn eval_expr_list(
        &mut self,
        exprs: impl IntoIterator<Item = Node<Expression>>,
    ) -> Result<Vec<ObjectRef>> {
        exprs.into_iter().map(|node| self.eval(node)).collect()
    }

    fn eval_literal(&mut self, literal: Node<Literal>) -> Result<ObjectRef> {
        match literal.v {
            Literal::String(string) => Ok(self.create_string(string.v.value)),
            Literal::Number(number) => Ok(self.create_number(number.v.value)),
            Literal::Boolean(boolean) => Ok(self.create_bool(boolean.v.value)),
            Literal::Array(array) => {
                let elements = self.eval_expr_list(array.v.elements)?;
                Ok(self.create_array(elements))
            }
            Literal::Nil(_) => Ok(self.nil()),
        }
    }
}
