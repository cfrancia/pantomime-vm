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
                        StepAction::ReturnValue(value) => {
                            let mut previous_frame = stack.pop()
                                .expect("Tried to return value with an empty stack");
                            previous_frame.push_operand_stack_value(value);
                            stack.push(previous_frame);
                        }
                        StepAction::InitializeClass(class_name) => {
                            debug!("Initializing class: {}", class_name.to_string());

                            let class = self.loader
                                .resolve_class(&class_name)
                                .expect("Unable to find class");

                            stack.push(frame);
                            Self::initialize_class(class_name,
                                                   &class,
                                                   &mut self.data_store,
                                                   &mut stack);
                        }
                        StepAction::AllocateClass(class_name) => {
                            debug!("Allocating class: {}", class_name.to_string());

                            let class = self.loader
                                .resolve_class(&class_name)
                                .expect("Unable to find class");

                            if !self.data_store.has_class_statics(&class_name) {
                                Self::initialize_class(class_name,
                                                       &class,
                                                       &mut self.data_store,
                                                       &mut stack);
                            }

                            let pointer = self.data_store.heap().allocate_class(&class);
                            frame.push_operand_stack_value(JavaType::Reference { value: pointer });

                            stack.push(frame);
                        }
                        StepAction::InvokeVirtualMethod { class_name, name, descriptor, args } => {
                            debug!("Invoking virtual method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());

                            let class = self.loader
                                .resolve_class(&class_name)
                                .or(self.loader.load_class(&class_name))
                                .expect("Unable to find class");

                            let method = class.maybe_resolve_method(&**name)
                                .expect("Unable to find method");

                            stack.push(frame);
                            Self::call_static_method(class, method, args, &mut stack);
                        }
                        StepAction::InvokeSpecialMethod { class_name, name, descriptor, args } => {
                            debug!("Invoking special method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());

                            let class = self.loader
                                .resolve_class(&class_name)
                                .or(self.loader.load_class(&class_name))
                                .expect("Unable to find class");

                            let method = class.maybe_resolve_method(&**name)
                                .expect("Unable to find method");

                            stack.push(frame);
                            Self::call_static_method(class, method, args, &mut stack);
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

    fn initialize_class(class_name: Rc<Utf8Info>,
                        class: &Rc<ClassFile>,
                        data_store: &mut CommonDataStore,
                        stack: &mut Vec<Frame>) {
        data_store.register_class(class_name);

        let init_method = class.maybe_resolve_method("<clinit>");
        if init_method.is_some() {
            stack.push(Frame::new(class.clone(), init_method.unwrap(), vec![]));
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
                // with a single argument
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

pub struct ObjectHeap {
    current_pointer: u64,
    objects: HashMap<u64, AllocatedObject>,
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap {
            current_pointer: 0,
            objects: HashMap::new(),
        }
    }

    pub fn allocate_class(&mut self, class: &Rc<ClassFile>) -> u64 {
        let pointer = self.current_pointer;

        let object = AllocatedObject::new();
        self.objects.insert(pointer, object);

        self.current_pointer += 1;
        pointer
    }
}

pub struct AllocatedObject {
    instance_variables: HashMap<Rc<Utf8Info>, JavaType>,
}

impl AllocatedObject {
    pub fn new() -> AllocatedObject {
        AllocatedObject { instance_variables: HashMap::new() }
    }
}

pub struct CommonDataStore {
    pub class_statics: HashMap<Rc<Utf8Info>, ClassStaticInfo>,
    pub object_heap: ObjectHeap,
}

impl CommonDataStore {
    pub fn new() -> CommonDataStore {
        CommonDataStore {
            class_statics: HashMap::new(),
            object_heap: ObjectHeap::new(),
        }
    }

    pub fn heap(&mut self) -> &mut ObjectHeap {
        &mut self.object_heap
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
