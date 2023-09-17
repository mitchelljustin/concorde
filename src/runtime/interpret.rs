use crate::runtime::object::{MethodBody, ObjectRef, Param};
use crate::runtime::Error::{ArityMismatch, NoSuchMethod};
use crate::runtime::Runtime;
use crate::runtime::{Result, StackFrame};
use crate::types::{Call, Expression, LValue, Literal, Node, Program, Statement};

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
                let class = self.class();
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
            }
            Statement::Assignment(assignment) => {
                let LValue::Variable(var) = assignment.v.target.v.clone() else {
                    unimplemented!();
                };
                let value = self.eval(assignment.v.value)?;
                self.assign(var.ident.name.clone(), value);
            }
            node => unimplemented!("{node:?}"),
        };
        Ok(())
    }

    pub fn eval(&mut self, expression: Node<Expression>) -> Result<ObjectRef> {
        match expression.v {
            Expression::Variable(var) => self.resolve(var.ident.name.as_ref()),
            Expression::Call(call) => self.perform_call(call),
            Expression::Literal(literal) => self.eval_literal(literal),

            node => unimplemented!("{node:?}"),
        }
    }

    fn perform_call(&mut self, call: Node<Call>) -> Result<ObjectRef> {
        let Call { target, arguments } = call.v;
        let Expression::Variable(method_name) = target.v else {
            unimplemented!();
        };
        let method_name = &method_name.ident.name;
        let receiver = self.receiver();
        let class = receiver.borrow().class();
        let class_borrowed = class.borrow();
        let Some(method) = class_borrowed.methods.get(method_name) else {
            return Err(NoSuchMethod {
                class_name: class.borrow().get_name(),
                method_name: method_name.clone(),
            });
        };
        let arguments = arguments
            .into_iter()
            .map(|argument| self.eval(argument))
            .collect::<Result<Vec<_>, _>>()?;
        match &method.body {
            MethodBody::User(body) => {
                if arguments.len() != method.params.len() {
                    return Err(ArityMismatch {
                        expected: method.params.len(),
                        actual: arguments.len(),
                        class_name: class.borrow().get_name(),
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
                self.stack.push_back(StackFrame {
                    receiver: Some(receiver),
                    method_name: Some(method_name.clone()),
                    variables,
                    ..StackFrame::default()
                });
                let mut retval = self.nil();
                for (i, statement) in body.statements.iter().enumerate() {
                    match &statement.v {
                        Statement::Expression(expression) if i == body.statements.len() - 1 => {
                            retval = self.eval(expression.clone())?;
                        }
                        _ => self.exec(statement.clone())?,
                    }
                }
                self.stack.pop_back();
                Ok(retval)
            }
            MethodBody::System(function) => Ok(function(self, receiver.clone(), arguments)),
        }
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
