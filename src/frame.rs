
use pantomime_parser::primitives::{U1, U2, U4};

use pantomime_parser::{ClassFile, ParserError};
use pantomime_parser::components::{Attribute, CodeAttribute, ConstantPoolItem,
                                   FieldOrMethodOrInterfaceMethodInfo, Method, Utf8Info};

use regex::Regex;

use std::rc::Rc;

lazy_static ! {
    static ref DESCRIPTOR_REGEX: Regex =
        Regex::new(r"^\((?P<arguments>[A-Za-z/\[;]+)\)(?P<return>[A-Za-z\[;]+)$")
        .unwrap();
}

macro_rules! retrieve_and_advance {
    ($index:ident, $vec:ident$(.$additional_ident:ident)*) => {
        {
            let temp_var = match $vec$(.$additional_ident)*.get($index.get_and_increment()) {
                Some(val) => val,
                None => return Err(StepError::CodeIndexOutOfBounds($index.current())),
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

pub type StepResult<T> = Result<T, StepError>;

#[derive(Debug)]
pub enum StepAction {
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
pub enum StepError {
    CodeIndexOutOfBounds(usize),
    UnexpectedEmptyVec,
    Parser(ParserError),
    UnexpectedConstantPoolItem(&'static str),
    UnknownOpcode(U1),
    UnexpectedJavaType(&'static str),
}

impl From<ParserError> for StepError {
    fn from(error: ParserError) -> StepError {
        StepError::Parser(error)
    }
}

#[derive(Debug)]
pub enum JavaType {
    String { index: U2 },
    Int { value: U4 },
    Byte { value: U1 },
    Empty,
}

impl JavaType {
    pub fn to_friendly_name(&self) -> &'static str {
        return match self {
            &JavaType::String { .. } => "String",
            &JavaType::Int { .. } => "Int",
            &JavaType::Byte { .. } => "Byte",
            &JavaType::Empty => "Empty",
        };
    }

    pub fn take(index: usize, variables: &mut Vec<JavaType>) -> JavaType {
        let removing_last = index >= variables.len();
        let removed_element = variables.remove(index);

        if removing_last {
            variables.push(JavaType::Empty);
        } else {
            variables.insert(index, JavaType::Empty);
        }

        removed_element
    }

    pub fn pop_int(item_vec: &mut Vec<JavaType>) -> StepResult<U4> {
        return match item_vec.pop() {
            Some(item) => {
                match item {
                    JavaType::Int { value } => Ok(value),
                    unexpected @ _ => {
                        Err(StepError::UnexpectedJavaType(unexpected.to_friendly_name()))
                    }
                }
            }
            None => Err(StepError::UnexpectedEmptyVec),
        };
    }
}

pub struct Frame {
    classfile: Rc<ClassFile>,
    code_attribute: Rc<CodeAttribute>,
    code_position: Codepoint,
    operand_stack: Vec<JavaType>,
    variables: Vec<JavaType>,
}

impl Frame {
    pub fn new(classfile: Rc<ClassFile>,
               method: Rc<Method>,
               provided_variables: Vec<JavaType>)
               -> Frame {
        debug!("Interpreting method: {}", method.name.to_string());

        let code_attribute = Self::resolve_code_attribute(&method.attributes)
            .expect("Method does not have a code attribute!");

        let mut variables = vec![];
        for _ in 0..code_attribute.max_locals {
            variables.push(JavaType::Empty);
        }

        let mut provided_variables = provided_variables;
        for (i, item) in provided_variables.drain(..).enumerate() {
            variables[i] = item;
        }

        Frame {
            classfile: classfile,
            code_attribute: code_attribute,
            code_position: Codepoint::new(),
            operand_stack: vec![],
            variables: variables,
        }
    }

    pub fn step(&mut self) -> StepResult<StepAction> {
        let constant_pool = &self.classfile.constant_pool;
        let ref mut code_position = self.code_position;

        if let Some(opcode) = self.code_attribute.code.get(code_position.current()) {
            code_position.get_and_increment();

            match *opcode {
                // iconst_2
                5 => self.operand_stack.push(JavaType::Int { value : 5}),
                // iconst_5
                8 => self.operand_stack.push(JavaType::Int { value: 5 }),
                // bipush
                16 => {
                    let entry = try!(Self::next_opcode_entry_u1(code_position,
                                                                &self.code_attribute));
                    self.operand_stack.push(JavaType::Int { value: entry as U4 });
                }
                // ldc
                18 => {
                    let index = try!(Self::next_opcode_entry_u1(code_position,
                                                                &self.code_attribute));
                    let stack_val = match try!(ConstantPoolItem::retrieve_item(index as usize,
                                                                               constant_pool)) {
                        &ConstantPoolItem::String(..) => JavaType::String { index: index },
                        &ConstantPoolItem::Integer(ref info) => JavaType::Int { value: info.bytes },
                        item @ _ => {
                            return Err(StepError::UnexpectedConstantPoolItem(
                                    item.to_friendly_name()));
                        }
                    };

                    self.operand_stack.push(stack_val);
                }
                // iload_0
                26 => self.operand_stack.push(JavaType::take(0, &mut self.variables)),
                // iload_1
                27 => self.operand_stack.push(JavaType::take(1, &mut self.variables)),
                // aload_0
                42 => self.operand_stack.push(JavaType::take(0, &mut self.variables)),
                // istore_1
                60 => {
                    self.variables[1] =
                        self.operand_stack.pop().expect("Operand stack was unexpectedly empty")
                }
                // iadd | isub | imul | idiv
                96 | 100 | 104 | 108 => {
                    let left = try!(JavaType::pop_int(&mut self.operand_stack));
                    let right = try!(JavaType::pop_int(&mut self.operand_stack));

                    let result = match *opcode {
                        96 => left + right,
                        100 => left - right,
                        104 => left * right,
                        108 => left / right,
                        _ => unreachable!(),
                    };

                    self.operand_stack.push(JavaType::Int { value: result });
                }
                // i2b
                145 => {
                    let int_val = try!(JavaType::pop_int(&mut self.operand_stack));
                    self.operand_stack.push(JavaType::Byte { value: int_val as U1 });
                }
                // return
                177 => return Ok(StepAction::EndOfMethod),
                // invokestatic
                184 => {
                    let index = try!(Self::next_opcode_entry_u2(code_position,
                                                                &self.code_attribute));

                    let method_info = try!(ConstantPoolItem::retrieve_method_info(index,
                                                                                  constant_pool));
                    let method = try!(Resolver::resolve_method_info(&*method_info, constant_pool));

                    let argument_count = Self::determine_number_of_arguments(&method.descriptor);

                    let mut args = vec![];
                    for _ in 0..argument_count {
                        args.push(self.operand_stack
                            .pop()
                            .expect("Expected value on operand stack"));
                    }

                    return Ok(StepAction::InvokeStaticMethod {
                        class_name: method.class_name,
                        name: method.name,
                        descriptor: method.descriptor,
                        args: args,
                    });
                }
                val @ _ => return Err(StepError::UnknownOpcode(val)),
            }

            return Ok(StepAction::Continue);
        }

        Ok(StepAction::EndOfMethod)
    }

    fn next_opcode_entry_u1(code_position: &mut Codepoint,
                            code_attribute: &CodeAttribute)
                            -> StepResult<U2> {
        let index = retrieve_and_advance!(code_position, code_attribute.code);
        Ok(index)
    }

    fn next_opcode_entry_u2(code_position: &mut Codepoint,
                            code_attribute: &Rc<CodeAttribute>)
                            -> StepResult<U2> {
        let index_one = retrieve_and_advance!(code_position, code_attribute.code);
        let index_two = retrieve_and_advance!(code_position, code_attribute.code);

        let index = (index_one << 8) | index_two;
        Ok(index)
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

    fn determine_number_of_arguments(descriptor: &Rc<Utf8Info>) -> usize {
        let argument = DESCRIPTOR_REGEX.captures(&descriptor)
            .expect("Couldn't find any arguments in descriptor!")
            .name("arguments")
            .unwrap();

        let mut characters = argument.chars();
        let mut argument_count = 0;

        while let Some(letter) = characters.next() {
            if letter.eq(&'L') {
                while let Some(additional_letter) = characters.next() {
                    if additional_letter.eq(&';') {
                        break;
                    }
                    // continue consuming the iterator
                }

                argument_count += 1;
                continue;
            }

            let should_increase_count = match letter {
                'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' => true,
                c @ _ => panic!("Unknown descriptor character: {}", c),
            };

            if should_increase_count {
                argument_count += 1;
            }
        }

        argument_count
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
                               -> StepResult<InitializedMethodInfo> {
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
