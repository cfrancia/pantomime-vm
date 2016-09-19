extern crate pantomime_parser;

#[macro_use]
extern crate log;

use interpreter::{Interpreter, InterpreterAction, InterpreterError};
use loader::BaseClassLoader;

use pantomime_parser::ParserError;

use std::path::PathBuf;

mod interpreter;
mod loader;

pub type VirtualMachineResult<T> = Result<T, VirtualMachineError>;

#[derive(Debug)]
pub enum VirtualMachineError {
    InvalidClassFile(ParserError),
    ClassNotFound(String),
}

impl From<ParserError> for VirtualMachineError {
    fn from(error: ParserError) -> VirtualMachineError {
        VirtualMachineError::InvalidClassFile(error)
    }
}

pub struct VirtualMachine {
    pub loader: BaseClassLoader,
}

impl VirtualMachine {
    pub fn new() -> VirtualMachine {
        VirtualMachine { loader: BaseClassLoader::new() }
    }

    pub fn add_classfile_path(&mut self, path: PathBuf) {
        if !path.exists() {
            panic!("Provided classfile path <{:?}> does not exist", path);
        }

        self.loader.add_classfile_path(path);
    }

    pub fn start(&mut self, main_class: &str) {
        let main_class = self.loader.load_class(main_class).expect("Unable to load main class!");
        let main_method = main_class.maybe_resolve_main_method()
            .expect("Provided main class does not have a main method!");

        let mut interpreter = Interpreter::new(main_class, main_method);

        loop {
            match interpreter.step() {
                Ok(action) => {
                    match action {
                        InterpreterAction::Continue => (),
                        InterpreterAction::EndOfMethod => break,
                        InterpreterAction::InvokeStaticMethod { class_name, name, descriptor } => {
                            debug!("Invoking static method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());
                        }
                    }
                }
                Err(error) => {
                    Self::handle_interpreter_error(error);
                }
            }
        }
    }

    fn handle_interpreter_error(error: InterpreterError) {
        match error {
            InterpreterError::Parser(val) => {
                panic!("Parser error: {:?}", val);
            }
            InterpreterError::CodeIndexOutOfBounds(val) => {
                panic!("Code index out of bounds: {:?}", val);
            }
            InterpreterError::UnexpectedConstantPoolItem(item) => {
                panic!("Unexpected ConstantPoolItem: {}", item);
            }
            InterpreterError::UnknownOpcode(val) => {
                panic!("Unknown opcode: {}", val);
            }
        }
    }
}
