
use pantomime_parser::primitives::U1;

use pantomime_parser::components::{Attribute, CodeAttribute, Method};

use std::rc::Rc;

pub type InterpreterResult = Result<InterpreterAction, InterpreterError>;

pub enum InterpreterAction {
    EndOfMethod,
}

pub enum InterpreterError {
    UnknownOpcode(U1),
}

pub struct Interpreter {
    code_attribute: Rc<CodeAttribute>,
    code_position: usize,
}

impl Interpreter {
    pub fn new(method: Rc<Method>) -> Interpreter {
        debug!("Interpreting method: {}", method.name.to_string());

        let code_attribute = Self::resolve_code_attribute(&method.attributes)
            .expect("Method does not have a code attribute!");

        Interpreter {
            code_attribute: code_attribute,
            code_position: 0,
        }
    }

    pub fn step(&mut self) -> InterpreterResult {
        if let Some(opcode) = self.code_attribute.code.get(self.code_position) {
            match opcode {
                val @ _ => return Err(InterpreterError::UnknownOpcode(*val)),
            }
        }

        Ok(InterpreterAction::EndOfMethod)
    }

    fn resolve_code_attribute(attributes: &Vec<Rc<Attribute>>) -> Option<Rc<CodeAttribute>> {
        for attribute in attributes {
            match **attribute {
                Attribute::Code(ref val) => return Some(val.clone()),
                _ => (),
            }
        }

        None
    }
}
