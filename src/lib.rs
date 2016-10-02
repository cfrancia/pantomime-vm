extern crate pantomime_parser;
extern crate regex;

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use frame::{Frame, StepAction, StepError, JavaType};
use loader::BaseClassLoader;

use pantomime_parser::{ClassFile, ParserError};
use pantomime_parser::components::{AccessFlags, Field, Method, Utf8Info};

use std::collections::HashMap;
use std::ops::{Index, IndexMut};
use std::path::PathBuf;
use std::rc::Rc;

mod frame;
mod loader;

macro_rules! resolve_class {
    ($loader:ident$(.$additional_ident:ident)*, $class_name:ident) =>
    {
        $loader$(.$additional_ident)*
            .resolve_class(&$class_name)
            .expect("Unable to find class")
    }
}

macro_rules! load_class {
    ($loader:ident$(.$additional_ident:ident)*, $class_name:ident) =>
    {
        $loader$(.$additional_ident)*
            .resolve_class(&$class_name)
            .or_else(|_| $loader$(.$additional_ident)*.load_class(&$class_name))
            .expect("Unable to find class");
    }
}

const STRING_CLASS: &'static str = "java/lang/String";

pub type VirtualMachineResult<T> = Result<T, VirtualMachineError>;

#[derive(Debug)]
pub enum VirtualMachineError {
    InvalidClassFile(ParserError),
    ClassNotFound(String),
}

pub type DataStoreResult<T> = Result<T, DataStoreError>;

#[derive(Debug)]
pub enum DataStoreError {
    InvalidPointer(u64),
    UnexpectedHeapType,
    UninitializedClass(String),
    StaticFieldNotFound(String),
    FieldNotFound(String),
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

            if stack.len() > 255 {
                panic!("Stack overflow");
            }

            let mut frame = stack.pop().unwrap();

            match frame.step(&mut self.data_store) {
                Ok(action) => {
                    match action {
                        StepAction::EndOfMethod => debug!("Reached end of method"),
                        StepAction::ReturnValue(value) => {
                            let mut previous_frame = stack.pop()
                                .expect("Tried to return value with an empty stack");
                            previous_frame.push_operand_stack_value(value);
                            stack.push(previous_frame);
                        }
                        StepAction::InitializeClass(class_name) => {
                            debug!("Initializing class: {}", class_name.to_string());
                            let class = resolve_class!(self.loader, class_name);

                            stack.push(frame);
                            Self::initialize_class(class_name,
                                                   &class,
                                                   &mut self.data_store,
                                                   &mut stack);
                        }
                        StepAction::AllocateString(contents) => {
                            debug!("Allocating string: {}", contents);
                            let class = load_class!(self.loader, STRING_CLASS);

                            let value_array_pointer = self.data_store
                                .heap()
                                .allocate_array(contents.chars().count() as i32);
                            {
                                let mut value_array = self.data_store
                                    .heap()
                                    .get_array_mut(&JavaType::Reference {
                                        value: value_array_pointer,
                                    })
                                    .expect("Unable to reference newly created Array");

                                for (i, character) in contents.chars().enumerate() {
                                    value_array.store[i] = JavaType::Char { value: character };
                                }
                            }

                            let string_pointer = self.data_store.heap().allocate_object(&class);
                            let mut string_object = self.data_store
                                .heap()
                                .get_object_mut(&JavaType::Reference { value: string_pointer })
                                .expect("Unable to reference newly created String");

                            // TODO: Work out a better way of manually referencing field names.
                            let value_field = Rc::new(Utf8Info {
                                tag: 0,
                                length: 0,
                                value: "value".to_string(),
                            });
                            string_object.instance_variables.insert(value_field,
                                                                    JavaType::Reference {
                                                                        value: value_array_pointer,
                                                                    });

                            frame.push_operand_stack_value(JavaType::Reference {
                                value: string_pointer,
                            });

                            stack.push(frame);
                        }
                        StepAction::AllocateClass(class_name) => {
                            debug!("Allocating class: {}", class_name.to_string());
                            let class = resolve_class!(self.loader, class_name);

                            if !self.data_store.has_class_statics(&class_name) {
                                Self::initialize_class(class_name,
                                                       &class,
                                                       &mut self.data_store,
                                                       &mut stack);
                            }

                            let pointer = self.data_store.heap().allocate_object(&class);
                            frame.push_operand_stack_value(JavaType::Reference { value: pointer });

                            stack.push(frame);
                        }
                        StepAction::AllocateArray(count) => {
                            debug!("Allocating array of size: {}", count);

                            let pointer = self.data_store.heap().allocate_array(count);
                            frame.push_operand_stack_value(JavaType::Reference { value: pointer });

                            stack.push(frame);
                        }
                        StepAction::InvokeVirtualMethod { class_name, name, descriptor, args } |
                        StepAction::InvokeSpecialMethod { class_name, name, descriptor, args } => {
                            debug!("Invoking virtual method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());

                            let class = load_class!(self.loader, class_name);
                            let method = class.maybe_resolve_method(&**name)
                                .expect("Unable to find method");

                            stack.push(frame);
                            stack.push(Frame::new(class, method, args));
                        }
                        StepAction::InvokeStaticMethod { class_name, name, descriptor, args } => {
                            debug!("Invoking static method: {}#{}({})",
                                   class_name.to_string(),
                                   name.to_string(),
                                   descriptor.to_string());

                            let class = resolve_class!(self.loader, class_name);
                            let method = class.maybe_resolve_method(&**name)
                                .expect("Unable to find method");

                            stack.push(frame);
                            Self::call_static_method(class,
                                                     method,
                                                     args,
                                                     &self.data_store.heap(),
                                                     &mut stack);
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
            StepError::DataStore(val) => {
                panic!("Data store error: {:?}", val);
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
                          heap: &ObjectHeap,
                          stack: &mut Vec<Frame>) {
        let mut args = args;
        {
            let access_flags = &method.access_flags;

            if AccessFlags::is_native(*access_flags) {
                debug!("Method is native");

                // TODO: Don't always assume it's going to be native println
                // with a single argument
                match args.pop().unwrap() {
                    reference @ JavaType::Reference { .. } => {
                        let object = heap.get_object(&reference)
                            .expect("Unable to retrieve referenced object");
                        if object.class_name != "java/lang/String" {
                            panic!("Unexpected class provided to print: {}", object.class_name);
                        }

                        let value_field = Rc::new(Utf8Info {
                            tag: 0,
                            length: 0,
                            value: "value".to_string(),
                        });
                        let value_reference = object.instance_variables
                            .get(&value_field)
                            .expect("Unable to retrieve array reference from String");

                        let value_array = heap.get_array(&value_reference)
                            .expect("Unable to retrieve referenced array");
                        let mut string_value = String::new();

                        for java_value in &value_array.store {
                            match java_value {
                                &JavaType::Char { value } => {
                                    string_value.push(value);
                                }
                                java_type @ _ => {
                                    panic!("Unexpected Java type: {}", java_type.to_friendly_name())
                                }
                            }
                        }

                        println!("OUT: {}", string_value);
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
    objects: HashMap<u64, HeapAllocation>,
}

impl ObjectHeap {
    pub fn new() -> ObjectHeap {
        ObjectHeap {
            current_pointer: 0,
            objects: HashMap::new(),
        }
    }

    pub fn allocate_object(&mut self, class: &Rc<ClassFile>) -> u64 {
        let pointer = self.current_pointer;

        let class_name = class.classname()
            .expect("Unable to resolve provided class name")
            .to_string();

        let mut object = AllocatedObject::new(class_name);

        let instance_fields: Vec<&Rc<Field>> = class.fields
            .iter()
            .filter(|val| !AccessFlags::is_static(val.access_flags))
            .collect();

        for instance_field in instance_fields {
            let default_value = match instance_field.descriptor.as_str().chars().next().unwrap() {
                'I' => JavaType::Int { value: 0 },
                'L' | '[' => JavaType::Null,
                d @ _ => panic!("Unexpected field type: {}", d),
            };

            object.instance_variables.insert(instance_field.name.clone(), default_value);
        }

        self.objects.insert(pointer, HeapAllocation::Object(object));

        self.current_pointer += 1;
        pointer
    }

    pub fn allocate_array(&mut self, count: i32) -> u64 {
        let pointer = self.current_pointer;
        self.objects.insert(pointer, HeapAllocation::Array(AllocatedArray::new(count)));

        self.current_pointer += 1;
        pointer
    }

    pub fn get_mut(&mut self, pointer: &JavaType) -> DataStoreResult<&mut HeapAllocation> {
        let pointer_value = Self::resolve_pointer(pointer);
        return match self.objects.get_mut(&pointer_value) {
            Some(val) => Ok(val),
            None => Err(DataStoreError::InvalidPointer(pointer_value)),
        };
    }

    pub fn get_object_mut(&mut self, pointer: &JavaType) -> DataStoreResult<&mut AllocatedObject> {
        match try!(self.get_mut(pointer)) {
            &mut HeapAllocation::Object(ref mut object) => Ok(object),
            _ => Err(DataStoreError::UnexpectedHeapType),
        }
    }

    pub fn get_array_mut(&mut self, pointer: &JavaType) -> DataStoreResult<&mut AllocatedArray> {
        match try!(self.get_mut(pointer)) {
            &mut HeapAllocation::Array(ref mut array) => Ok(array),
            _ => Err(DataStoreError::UnexpectedHeapType),
        }
    }

    pub fn get(&self, pointer: &JavaType) -> DataStoreResult<&HeapAllocation> {
        let pointer_value = Self::resolve_pointer(pointer);
        return match self.objects.get(&pointer_value) {
            Some(val) => Ok(val),
            None => Err(DataStoreError::InvalidPointer(pointer_value)),
        };
    }

    pub fn get_object(&self, pointer: &JavaType) -> DataStoreResult<&AllocatedObject> {
        match try!(self.get(pointer)) {
            &HeapAllocation::Object(ref object) => Ok(object),
            _ => Err(DataStoreError::UnexpectedHeapType),
        }
    }

    pub fn get_array(&self, pointer: &JavaType) -> DataStoreResult<&AllocatedArray> {
        match try!(self.get(pointer)) {
            &HeapAllocation::Array(ref array) => Ok(array),
            _ => Err(DataStoreError::UnexpectedHeapType),
        }
    }

    pub fn get_field(&self,
                     pointer: &JavaType,
                     field_name: &Rc<Utf8Info>)
                     -> DataStoreResult<&JavaType> {
        let object = try!(self.get_object(pointer));
        object.instance_variables
            .get(field_name)
            .map(|val| Ok(val))
            .unwrap_or_else(|| Err(DataStoreError::FieldNotFound(field_name.to_string())))
    }

    pub fn set_field(&mut self, pointer: &JavaType, field_name: Rc<Utf8Info>, value: JavaType) {
        let object = self.get_object_mut(pointer).expect("Unable to find instance");
        object.instance_variables.insert(field_name, value);
    }

    fn resolve_pointer(pointer: &JavaType) -> u64 {
        match pointer {
            &JavaType::Reference { value } => value,
            item @ _ => panic!("Unexpected JavaType: {}", item.to_friendly_name()),
        }
    }
}

pub enum HeapAllocation {
    Object(AllocatedObject),
    Array(AllocatedArray),
}

pub struct AllocatedObject {
    pub class_name: String,
    pub instance_variables: HashMap<Rc<Utf8Info>, JavaType>,
}

impl AllocatedObject {
    pub fn new(class_name: String) -> AllocatedObject {
        AllocatedObject {
            class_name: class_name,
            instance_variables: HashMap::new(),
        }
    }
}

pub struct AllocatedArray {
    pub count: i32,
    pub store: Vec<JavaType>,
}

impl AllocatedArray {
    pub fn new(count: i32) -> AllocatedArray {
        let mut store = Vec::with_capacity(count as usize);

        // TODO: This should be the default value of the type.
        for _ in 0..count {
            store.push(JavaType::Null);
        }

        AllocatedArray {
            count: count,
            store: store,
        }
    }
}

impl Index<i32> for AllocatedArray {
    type Output = JavaType;

    fn index(&self, _index: i32) -> &JavaType {
        self.store.index(_index as usize)
    }
}

impl IndexMut<i32> for AllocatedArray {
    fn index_mut(&mut self, _index: i32) -> &mut JavaType {
        self.store.index_mut(_index as usize)
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
                            -> DataStoreResult<&JavaType> {
        let static_class = match self.class_statics.get(class_name) {
            Some(val) => val,
            None => return Err(DataStoreError::UninitializedClass(class_name.to_string())),
        };

        return match static_class.static_fields.get(field_name) {
            Some(val) => Ok(val),
            None => Err(DataStoreError::StaticFieldNotFound(field_name.to_string())),
        };
    }
}
