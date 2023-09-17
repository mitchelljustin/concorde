use crate::runtime::object::{MethodBody, ObjectRef, Param};
use crate::runtime::Error::NoSuchMethod;
use crate::runtime::Result;
use crate::runtime::Runtime;
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
            Expression::Call(call) => {
                let Call { target, arguments } = call.v;
                let Expression::Variable(method_name) = target.v else {
                    unimplemented!();
                };
                let name = &method_name.ident.name;
                let receiver = self.receiver();
                let class = receiver.borrow().class();
                let class_borrowed = class.borrow();
                let Some(method) = class_borrowed.methods.get(name) else {
                    return Err(NoSuchMethod {
                        class: class.clone(),
                        name: name.clone(),
                    });
                };
                let arguments = arguments
                    .into_iter()
                    .map(|argument| self.eval(argument))
                    .collect::<Result<_, _>>()?;
                match &method.body {
                    MethodBody::User(_) => unimplemented!(),
                    MethodBody::System(function) => Ok(function(self, receiver.clone(), arguments)),
                }
            }
            Expression::Literal(literal) => self.eval_literal(literal),

            node => unimplemented!("{node:?}"),
        }
    }

    fn eval_literal(&mut self, literal: Node<Literal>) -> Result<ObjectRef> {
        match literal.v {
            Literal::String(string) => Ok(self.create_string(string.value.clone())),
            Literal::Number(_) => unimplemented!(),
            Literal::Boolean(_) => unimplemented!(),
            Literal::Nil(_) => Ok(self.nil()),
        }
    }
}
