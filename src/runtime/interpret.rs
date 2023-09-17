use crate::runtime::object::{Method, MethodBody, ObjectRef, Param};
use crate::runtime::Error::DuplicateDefinition;
use crate::runtime::Result;
use crate::runtime::Runtime;
use crate::types::{Call, Expression, LValue, Literal, Node, Primitive, Program, Statement};

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
                let receiver = self.receiver();
                let method_name = method_def.name.name.clone();
                if receiver.borrow_mut().methods.contains_key(&method_name) {
                    return Err(DuplicateDefinition {
                        name: method_name.clone(),
                    });
                }
                let params = method_def
                    .parameters
                    .iter()
                    .map(|param| Param::Positional(param.name.name.clone()))
                    .collect();
                let body = MethodBody::User(method_def.v.body);
                let method = Method {
                    name: method_name.clone(),
                    class: receiver.borrow().class.clone().unwrap(),
                    params,
                    body,
                };
                receiver.borrow_mut().methods.insert(method_name, method);
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
                let Call { arguments, target } = call.v;
                let Expression::Variable(fn_name) = target.v else {
                    unimplemented!();
                };
                match fn_name.ident.name.as_ref() {
                    "debug" => {
                        for argument in arguments {
                            let argument = self.eval(argument)?;
                            println!("{}", argument.borrow().debug());
                        }
                        Ok(self.nil())
                    }
                    "print" => {
                        let to_print = arguments
                            .into_iter()
                            .map(|argument| self.eval(argument))
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .filter_map(|argument| {
                                if let Some(Primitive::String(string)) =
                                    argument.borrow().primitive.clone()
                                {
                                    Some(string.to_string())
                                } else if argument == self.builtins.nil {
                                    Some("nil".to_string())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("{}", to_print);
                        Ok(self.nil())
                    }
                    _ => unimplemented!(),
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
