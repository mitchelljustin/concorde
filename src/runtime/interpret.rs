use std::ops::ControlFlow;

use crate::runtime::builtin;
use crate::runtime::object::{MethodBody, MethodRef, ObjectRef, Param};
use crate::runtime::Error::{
    ArityMismatch, BadPath, IllegalAssignmentTarget, NoSuchMethod, NoSuchVariable, NotCallable,
    UndefinedProperty,
};
use crate::runtime::{Error, Runtime};
use crate::runtime::{Result, StackFrame};
use crate::types::{
    Access, Block, Call, Expression, LValue, Literal, MethodDefinition, Node, NodeMeta, Path,
    Program, Statement,
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
                    LValue::Index(index_node) => {
                        let target = self.eval(*index_node.v.target)?;
                        let index = self.eval(*index_node.v.index)?;
                        let value = value?;
                        self.call_instance_method(
                            target,
                            builtin::op::__set_index__,
                            [index, value],
                            Some(index_node.meta),
                        )?;
                    }
                }
            }
            Statement::ClassDefinition(class_def) => {
                let class = self.create_simple_class(class_def.v.name.v.name);
                self.push_stack_frame(StackFrame {
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
                self.pop_stack_frame();
                return result;
            }
            Statement::ForIn(for_in) => {
                let binding_name = for_in.v.binding.v.ident.v.name;
                let iterator = self.eval(for_in.v.iterator)?;
                self.push_stack_frame(StackFrame::default());
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
                self.pop_stack_frame();
                return result;
            }
            Statement::WhileLoop(while_loop) => {
                self.push_stack_frame(StackFrame::default());
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
            Statement::Use(use_stmt) => {
                let class = self.resolve_class_from_path(use_stmt.v.path)?;
                self.stack.last_mut().unwrap().open_classes.push(class);
            }
        };
        Ok(())
    }

    fn pop_stack_frame(&mut self) -> Option<StackFrame> {
        self.stack.pop()
    }

    fn push_stack_frame(&mut self, stack_frame: StackFrame) {
        self.stack.push(stack_frame)
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
            Expression::Variable(var) => {
                let name = &var.v.ident.v.name;
                self.resolve_variable(name).ok_or(NoSuchVariable {
                    name: name.clone(),
                    node: var.meta,
                })
            }
            Expression::Call(call) => self.eval_call_expr(call),
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
                self.call_instance_method(lhs, method_name, [rhs], Some(binary.meta))
            }
            Expression::Index(index_node) => {
                let target = self.eval(*index_node.v.target)?;
                let index = self.eval(*index_node.v.index)?;
                self.call_instance_method(
                    target,
                    builtin::op::__index__,
                    [index],
                    Some(index_node.meta),
                )
            }
            Expression::Unary(unary) => {
                let rhs = self.eval(*unary.v.rhs)?;
                let method_name = builtin::op::method_for_unary_op(&unary.v.op.v).unwrap();
                self.call_instance_method(rhs, method_name, None, Some(unary.meta))
            }
            Expression::Path(path) => self.resolve_class_from_path(path),
        }
    }

    fn eval_call_expr(&mut self, call: Node<Call>) -> Result<ObjectRef> {
        let target = call.v.target;
        let receiver;
        let method;
        match target.v {
            Expression::Variable(var) => {
                let method_name = var.v.ident.v.name;
                if let Some(class_var) = self.resolve_variable(&method_name) && self.is_class(&class_var) {
                    receiver = class_var.clone();
                    method = class_var.borrow().get_init_method();
                } else {
                    if let Some((current_receiver, instance_method)) = self
                        .current_instance()
                        .map(|receiver|
                            receiver
                                .borrow()
                                .__class__()
                                .borrow()
                                .resolve_own_method(&method_name)
                                .map(|method| (receiver.clone(), method)))
                        .flatten() {
                        receiver = current_receiver;
                        method = instance_method;
                    } else {
                        let mut search_classes = self.stack
                            .iter()
                            .map(|frame| frame.open_classes.iter())
                            .flatten()
                            .rev();
                        let (found_class, found_method) = search_classes
                            .find_map(|class| {
                                class
                                    .borrow()
                                    .resolve_own_method(&method_name)
                                    .map(|method| (class, method))
                            })
                            .ok_or(NoSuchMethod {
                                node: call.meta.into(),
                                search: method_name,
                            })?;
                        receiver = found_class.clone();
                        method = found_method;
                    }
                }
            }
            Expression::Path(mut path) => {
                let method_component = path.v.components.pop().unwrap();
                let method_name = method_component.v.ident.v.name;
                receiver = self.resolve_class_from_path(path)?;
                if let Some(class) = receiver.borrow().get_property(&method_name) {
                    method = class.borrow().get_init_method();
                } else {
                    method =
                        receiver
                            .borrow()
                            .resolve_own_method(&method_name)
                            .ok_or(NoSuchMethod {
                                node: call.meta.into(),
                                search: format!(
                                    "{}::{method_name}",
                                    receiver.borrow().__name__().unwrap()
                                ),
                            })?;
                }
            }
            _ => return Err(NotCallable { node: target.meta }),
        };
        let arguments = self.eval_expr_list(call.v.arguments)?;
        self.execute_method(receiver, method, arguments)
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
                self.call_instance_method(target, method_name, arguments, Some(call.meta))
            }
            _ => unimplemented!(),
        }
    }

    pub fn execute_method(
        &mut self,
        receiver: ObjectRef,
        method: MethodRef,
        arguments: impl IntoIterator<Item = ObjectRef>,
    ) -> Result<ObjectRef> {
        let class = method.class.upgrade().expect("method's class was dropped");
        let method_name = method.name.clone();
        let arguments: Vec<ObjectRef> = arguments.into_iter().collect();
        match &method.body {
            MethodBody::User(body) => {
                if arguments.len() != method.params.len() {
                    return Err(ArityMismatch {
                        expected: method.params.len(),
                        actual: arguments.len(),
                        class_name: class.borrow().__name__().unwrap(),
                        method_name,
                    });
                }
                let variables = method
                    .params
                    .iter()
                    .zip(arguments.into_iter())
                    .map(|(param, arg)| {
                        let Param::Positional(name) = param else {
                            todo!();
                        };
                        (name.clone(), arg)
                    })
                    .collect();
                self.push_stack_frame(StackFrame {
                    instance: Some(receiver.clone()),
                    method_name: Some(method_name.clone()),
                    variables,
                    ..StackFrame::default()
                });
                let result = self.eval_block(body.clone());
                self.pop_stack_frame();
                if method_name == builtin::method::init {
                    result?;
                    Ok(receiver)
                } else {
                    result
                }
            }
            MethodBody::System(function) => function(self, receiver, method_name, arguments),
        }
    }

    pub fn call_instance_method(
        &mut self,
        receiver: ObjectRef,
        method_name: &str,
        arguments: impl IntoIterator<Item = ObjectRef>,
        node: Option<NodeMeta>,
    ) -> Result<ObjectRef> {
        let class = receiver.borrow().__class__();
        let class_name = class.borrow().__name__().unwrap();
        let method = class
            .borrow()
            .resolve_own_method(method_name)
            .ok_or(NoSuchMethod {
                search: format!("{class_name}::{method_name}"),
                node: node.into(),
            })?;
        self.execute_method(receiver, method, arguments)
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
            Literal::StringLit(string) => Ok(self.create_string(string.v.value)),
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
