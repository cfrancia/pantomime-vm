extern crate pantomime_parser;

#[macro_use]
extern crate log;

use interpreter::{Interpreter, InterpreterAction, InterpreterError, JavaType};
use loader::BaseClassLoader;

use pantomime_parser::{ClassFile, ParserError};
use pantomime_parser::components::{AccessFlags, Method};

use std::path::PathBuf;
use std::rc::Rc;

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
        self.loader.preload_classes();

        let main_class = self.loader.resolve_class(main_class).expect("Unable to load main class!");
        let main_method = main_class.maybe_resolve_main_method()
            .expect("Provided main class does not have a main method!");

        let mut stack = vec![];
        stack.push(Interpreter::new(main_class, main_method, vec![]));

        loop {
            let mut interpreter = stack.pop().expect("The stack is unexpectedly empty!");

            match interpreter.step() {
                Ok(action) => {
                    match action {
                        InterpreterAction::Continue => stack.push(interpreter),
                        InterpreterAction::EndOfMethod => {
                            debug!("Reached end of method");
                            break;
                        }
                        InterpreterAction::InvokeStaticMethod { class_name,
                                                                name,
                                                                descriptor,
                                                                args } => {
                            debug!("Invoking static method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());

                            let class =
                                self.loader.resolve_class(&class_name).expect("Unable to find class");
                            let method = class.maybe_resolve_method(&**name)
                                .expect("Unable to find method");

                            stack.push(interpreter);
                            Self::call_static_method(class, method, args, &mut stack);
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

    fn call_static_method(class: Rc<ClassFile>,
                          method: Rc<Method>,
                          args: Vec<JavaType>,
                          stack: &mut Vec<Interpreter>) {
        let mut args = args;
        {
            let access_flags = &method.access_flags;

            if AccessFlags::is_native(*access_flags) {
                debug!("Method is native");

                // TODO: Don't always assume it's going to be native println
                // with a single string argument
                let value = match args.pop().unwrap() {
                    JavaType::String { index } => {
                        class.constant_pool_resolver().resolve_string_constant(index).unwrap()
                    }
                };

                println!("OUT: {}", value);
                return;
            }
        }

        stack.push(Interpreter::new(class, method, args));
    }
}
