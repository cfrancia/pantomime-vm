extern crate pantomime_parser;
extern crate regex;

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use frame::{Frame, StepAction, StepError, JavaType};
use loader::BaseClassLoader;

use pantomime_parser::{ClassFile, ParserError};
use pantomime_parser::components::{AccessFlags, Method, Utf8Info};

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

mod frame;
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
    pub data_store: CommonDataStore,
}

impl VirtualMachine {
    pub fn new() -> VirtualMachine {
        VirtualMachine {
            loader: BaseClassLoader::new(),
            data_store: CommonDataStore::new(),
        }
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
        stack.push(Frame::new(main_class, main_method, vec![]));

        loop {
            if stack.len() == 0 {
                debug!("Reached the end of the stack");
                break;
            }

            let mut frame = stack.pop().unwrap();

            match frame.step(&mut self.data_store) {
                Ok(action) => {
                    match action {
                        StepAction::Continue => stack.push(frame),
                        StepAction::EndOfMethod => debug!("Reached end of method"),
                        StepAction::InitializeClass(class_name) => {
                            debug!("Initializing class: {}", class_name.to_string());

                            let class = self.loader
                                .resolve_class(&class_name)
                                .expect("Unable to find class");

                            self.data_store.register_class(class_name);
                            stack.push(frame);

                            let init_method = class.maybe_resolve_method("<clinit>");
                            if init_method.is_some() {
                                stack.push(Frame::new(class, init_method.unwrap(), vec![]));
                            }
                        }
                        StepAction::InvokeStaticMethod { class_name, name, descriptor, args } => {
                            debug!("Invoking static method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());

                            let class = self.loader
                                .resolve_class(&class_name)
                                .expect("Unable to find class");

                            let method = class.maybe_resolve_method(&**name)
                                .expect("Unable to find method");

                            stack.push(frame);
                            Self::call_static_method(class, method, args, &mut stack);
                        }
                    }
                }
                Err(error) => {
                    Self::handle_step_error(error);
                }
            }
        }
    }

    fn handle_step_error(error: StepError) {
        match error {
            StepError::Parser(val) => {
                panic!("Parser error: {:?}", val);
            }
            StepError::CodeIndexOutOfBounds(val) => {
                panic!("Code index out of bounds: {:?}", val);
            }
            StepError::UnexpectedEmptyVec => {
                panic!("Referenced vector was unexpectedly empty");
            }
            StepError::UnexpectedConstantPoolItem(item) => {
                panic!("Unexpected ConstantPoolItem: {}", item);
            }
            StepError::UnexpectedJavaType(item) => {
                panic!("Unexpected JavaType on locals/operand stack: {}", item);
            }
            StepError::UnknownOpcode(val) => {
                panic!("Unknown opcode: {}", val);
            }
        }
    }

    fn call_static_method(class: Rc<ClassFile>,
                          method: Rc<Method>,
                          args: Vec<JavaType>,
                          stack: &mut Vec<Frame>) {
        let mut args = args;
        {
            let access_flags = &method.access_flags;

            if AccessFlags::is_native(*access_flags) {
                debug!("Method is native");

                // TODO: Don't always assume it's going to be native println
                // with a single string argument
                match args.pop().unwrap() {
                    JavaType::String { index } => {
                        let value =
                            class.constant_pool_resolver().resolve_string_constant(index).unwrap();
                        println!("OUT: {}", value);
                    }
                    JavaType::Int { value } => println!("OUT: {}", value),
                    JavaType::Byte { value } => println!("OUT: {}", value),
                    JavaType::Long { value } => println!("OUT: {}", value),
                    item @ _ => panic!("Unexpected variable: {:?}", item),
                }

                return;
            }
        }

        stack.push(Frame::new(class, method, args));
    }
}

pub struct ClassStaticInfo {
    pub static_fields: HashMap<Rc<Utf8Info>, JavaType>,
}

impl ClassStaticInfo {
    pub fn new() -> ClassStaticInfo {
        ClassStaticInfo { static_fields: HashMap::new() }
    }
}

pub struct CommonDataStore {
    pub class_statics: HashMap<Rc<Utf8Info>, ClassStaticInfo>,
}

impl CommonDataStore {
    pub fn new() -> CommonDataStore {
        CommonDataStore { class_statics: HashMap::new() }
    }

    pub fn has_class_statics(&self, class_name: &Rc<Utf8Info>) -> bool {
        self.class_statics.contains_key(class_name)
    }

    pub fn register_class(&mut self, class_name: Rc<Utf8Info>) {
        self.class_statics.insert(class_name, ClassStaticInfo::new());
    }

    pub fn set_class_static(&mut self,
                            class_name: &Rc<Utf8Info>,
                            field_name: Rc<Utf8Info>,
                            value: JavaType) {
        self.class_statics
            .get_mut(class_name)
            .expect("Unable to find initialized class statics")
            .static_fields
            .insert(field_name, value);
    }

    pub fn get_class_static(&self,
                            class_name: &Rc<Utf8Info>,
                            field_name: &Rc<Utf8Info>)
                            -> &JavaType {
        self.class_statics
            .get(class_name)
            .expect("Unable to find initialized class statics")
            .static_fields
            .get(field_name)
            .expect("Unable to find static field on class")
    }
}
