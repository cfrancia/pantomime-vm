
use pantomime_parser::primitives::{U1, U2};

use pantomime_parser::{ClassFile, ParserError};
use pantomime_parser::components::{Attribute, CodeAttribute, ConstantPoolItem,
                                   FieldOrMethodOrInterfaceMethodInfo, Method, Utf8Info};

use std::rc::Rc;

macro_rules! retrieve_and_advance {
    ($index:ident, $vec:ident$(.$additional_ident:ident)*) => {
        {
            let temp_var = match $vec$(.$additional_ident)*.get($index.get_and_increment()) {
                Some(val) => val,
                None => return Err(InterpreterError::CodeIndexOutOfBounds($index.current())),
            };

            *temp_var as U2
        }
    }
}

struct Codepoint {
    position: usize,
}

impl Codepoint {
    pub fn new() -> Codepoint {
        Codepoint { position: 0 }
    }

    pub fn get_and_increment(&mut self) -> usize {
        let current_position = self.position;
        self.position += 1;
        current_position
    }

    pub fn current(&self) -> usize {
        self.position as usize
    }
}

pub type InterpreterResult<T> = Result<T, InterpreterError>;

#[derive(Debug)]
pub enum InterpreterAction {
    InvokeStaticMethod {
        class_name: Rc<Utf8Info>,
        name: Rc<Utf8Info>,
        descriptor: Rc<Utf8Info>,
        args: Vec<JavaType>,
    },
    Continue,
    EndOfMethod,
}

#[derive(Debug)]
pub enum InterpreterError {
    CodeIndexOutOfBounds(usize),
    Parser(ParserError),
    UnexpectedConstantPoolItem(&'static str),
    UnknownOpcode(U1),
}

impl From<ParserError> for InterpreterError {
    fn from(error: ParserError) -> InterpreterError {
        InterpreterError::Parser(error)
    }
}

#[derive(Debug)]
pub enum JavaType {
    String { index: U2 },
}

pub struct Interpreter {
    classfile: Rc<ClassFile>,
    code_attribute: Rc<CodeAttribute>,
    code_position: Codepoint,
    stack: Vec<JavaType>,
    variables: Vec<JavaType>,
}

impl Interpreter {
    pub fn new(classfile: Rc<ClassFile>,
               method: Rc<Method>,
               variables: Vec<JavaType>)
               -> Interpreter {
        debug!("Interpreting method: {}", method.name.to_string());

        let code_attribute = Self::resolve_code_attribute(&method.attributes)
            .expect("Method does not have a code attribute!");

        Interpreter {
            classfile: classfile,
            code_attribute: code_attribute,
            code_position: Codepoint::new(),
            stack: vec![],
            variables: variables,
        }
    }

    pub fn step(&mut self) -> InterpreterResult<InterpreterAction> {
        let constant_pool = &self.classfile.constant_pool;
        let ref mut code_position = self.code_position;

        if let Some(opcode) = self.code_attribute.code.get(code_position.current()) {
            code_position.get_and_increment();

            match *opcode {
                // ldc
                18 => {
                    let index = retrieve_and_advance!(code_position, self.code_attribute.code);

                    let stack_val = match try!(Self::retrieve_constant_pool_item(index,
                                                                                 constant_pool)) {
                        &ConstantPoolItem::String(..) => JavaType::String { index: index },
                        item @ _ => {
                            return Err(InterpreterError::UnexpectedConstantPoolItem(
                                    item.to_friendly_name()));
                        }
                    };

                    self.stack.push(stack_val);
                }
                // aload_0
                42 => {
                    let var = self.variables.remove(0);
                    self.stack.push(var);
                }
                // return
                177 => return Ok(InterpreterAction::EndOfMethod),
                // invokestatic
                184 => {
                    let index_one = retrieve_and_advance!(code_position, self.code_attribute.code);
                    let index_two = retrieve_and_advance!(code_position, self.code_attribute.code);

                    let index = (index_one << 8) | index_two;

                    match try!(Self::retrieve_constant_pool_item(index, constant_pool)) {
                        &ConstantPoolItem::Method(ref val) => {
                            let method = try!(Resolver::resolve_method_info(val, constant_pool));

                            // TODO: Actually work out the number of arguments
                            let mut args = vec![];
                            args.push(self.stack
                                .pop()
                                .expect("Should have already had an argument on the stack"));

                            return Ok(InterpreterAction::InvokeStaticMethod {
                                class_name: method.class_name,
                                name: method.name,
                                descriptor: method.descriptor,
                                args: args,
                            });
                        }
                        item @ _ => return Err(InterpreterError::UnexpectedConstantPoolItem(
                                item.to_friendly_name())),
                    }
                }
                val @ _ => return Err(InterpreterError::UnknownOpcode(val)),
            }

            return Ok(InterpreterAction::Continue);
        }

        Ok(InterpreterAction::EndOfMethod)
    }

    fn retrieve_constant_pool_item<'r>(index: U2,
                                       constant_pool: &'r Vec<ConstantPoolItem>)
                                       -> InterpreterResult<&'r ConstantPoolItem> {
        Ok(try!(ConstantPoolItem::retrieve_item(index as usize, constant_pool)))
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

macro_rules! generate_field_method_interface_method_struct {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name {
            pub class_name: Rc<Utf8Info>,
            pub name: Rc<Utf8Info>,
            pub descriptor: Rc<Utf8Info>,
        }
    }
}

generate_field_method_interface_method_struct!(InitializedFieldInfo);
generate_field_method_interface_method_struct!(InitializedMethodInfo);
generate_field_method_interface_method_struct!(InitializedInterfaceMethodInfo);

struct Resolver;

impl Resolver {
    pub fn resolve_method_info(info: &FieldOrMethodOrInterfaceMethodInfo,
                               constant_pool: &Vec<ConstantPoolItem>)
                               -> InterpreterResult<InitializedMethodInfo> {
        let class_index = info.class_index;
        let name_and_type_index = info.name_and_type_index;

        let class = try!(ConstantPoolItem::retrieve_class_info(class_index, constant_pool));
        let name_and_type =
            try!(ConstantPoolItem::retrieve_name_and_type_info(name_and_type_index, constant_pool));

        let class_name = try!(ConstantPoolItem::retrieve_utf8_info(class.name_index,
                                                                   constant_pool));
        let name = try!(ConstantPoolItem::retrieve_utf8_info(name_and_type.name_index,
                                                             constant_pool));
        let descriptor = try!(ConstantPoolItem::retrieve_utf8_info(name_and_type.descriptor_index,
                                                                   constant_pool));

        Ok(InitializedMethodInfo {
            class_name: class_name,
            name: name,
            descriptor: descriptor,
        })
    }
}
