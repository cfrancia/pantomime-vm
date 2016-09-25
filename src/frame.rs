
use super::CommonDataStore;

use pantomime_parser::primitives::{U1, U2};

use pantomime_parser::{ClassFile, ParserError};
use pantomime_parser::components::{Attribute, CodeAttribute, ConstantPoolItem, Method, Utf8Info};

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

    pub fn reverse(&mut self, steps: usize) {
        self.position -= steps;
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
    InitializeClass(Rc<Utf8Info>),
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

#[derive(Copy, Clone, Debug)]
pub enum JavaType {
    String { index: U2 },
    Byte { value: i8 },
    Int { value: i32 },
    Long { value: i64 },
    Filler,
    Empty,
}

macro_rules! generate_javatype_pop_method {
    ($variant_name:ident, $return_type:ident, $method_name:ident) => {
        pub fn $method_name(item_vec: &mut Vec<JavaType>) -> StepResult<$return_type> {
            return match item_vec.pop() {
                Some(item) => {
                    match item {
                        JavaType::$variant_name { value } => Ok(value),
                        unexpected @ _ => {
                            Err(StepError::UnexpectedJavaType(unexpected.to_friendly_name()))
                        }
                    }
                }
                None => Err(StepError::UnexpectedEmptyVec),
            };
        }
    }
}


impl JavaType {
    pub fn to_friendly_name(&self) -> &'static str {
        return match self {
            &JavaType::String { .. } => "String",
            &JavaType::Byte { .. } => "Byte",
            &JavaType::Int { .. } => "Int",
            &JavaType::Long { .. } => "Long",
            &JavaType::Filler { .. } => "Filler",
            &JavaType::Empty => "Empty",
        };
    }

    pub fn load(index: usize, variables: &mut Vec<JavaType>) -> JavaType {
        variables.get(index)
            .expect(&format!("Expected vec to contain item at index: {}", index))
            .clone()
    }

    generate_javatype_pop_method!(Int, i32, pop_int);
    generate_javatype_pop_method!(Long, i64, pop_long);
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

    pub fn step(&mut self, data_store: &mut CommonDataStore) -> StepResult<StepAction> {
        let constant_pool = &self.classfile.constant_pool;
        let ref mut code_position = self.code_position;

        if let Some(opcode) = self.code_attribute.code.get(code_position.current()) {
            code_position.get_and_increment();

            match *opcode {
                // iconst_2
                5 => self.operand_stack.push(JavaType::Int { value: 2 }),
                // iconst_3
                6 => self.operand_stack.push(JavaType::Int { value: 3 }),
                // iconst_5
                8 => self.operand_stack.push(JavaType::Int { value: 5 }),
                // bipush
                16 => {
                    let entry = try!(Self::next_opcode_entry_u1(code_position,
                                                                &self.code_attribute));
                    self.operand_stack.push(JavaType::Int { value: entry as i32 });
                }
                // ldc
                18 => {
                    let index = try!(Self::next_opcode_entry_u1(code_position,
                                                                &self.code_attribute));
                    let stack_val = match try!(ConstantPoolItem::retrieve_item(index as usize,
                                                                               constant_pool)) {
                        &ConstantPoolItem::String(..) => JavaType::String { index: index },
                        &ConstantPoolItem::Integer(ref info) => {
                            JavaType::Int { value: info.bytes as i32 }
                        }
                        item @ _ => {
                            return Err(StepError::UnexpectedConstantPoolItem(
                                    item.to_friendly_name()));
                        }
                    };

                    self.operand_stack.push(stack_val);
                }
                // ldc2_w
                20 => {
                    let index = try!(Self::next_opcode_entry_u2(code_position,
                                                                &self.code_attribute));
                    let stack_val = match try!(ConstantPoolItem::retrieve_item(index as usize,
                                                                               constant_pool)) {
                        &ConstantPoolItem::Long(ref info) => {
                            let value: i64 = ((info.high_bytes as i64) << 32) +
                                             info.low_bytes as i64;
                            JavaType::Long { value: value }
                        }
                        item @ _ => {
                            return Err(StepError::UnexpectedConstantPoolItem(
                                    item.to_friendly_name()));
                        }
                    };

                    self.operand_stack.push(stack_val);
                    // We need to load up two spots in the operand stack
                    self.operand_stack.push(JavaType::Filler);
                }
                // iload_0
                26 => self.operand_stack.push(JavaType::load(0, &mut self.variables)),
                // iload_1
                27 => self.operand_stack.push(JavaType::load(1, &mut self.variables)),
                // lload_0 (the first value is filler)
                30 => self.operand_stack.push(JavaType::load(1, &mut self.variables)),
                // lload_2 (the first value is filler)
                32 => self.operand_stack.push(JavaType::load(3, &mut self.variables)),
                // aload_0
                42 => self.operand_stack.push(JavaType::load(0, &mut self.variables)),
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
                // ladd | lsub | lmul | ldiv
                97 | 101 | 105 | 109 => {
                    let left = try!(JavaType::pop_long(&mut self.operand_stack));
                    let right = try!(JavaType::pop_long(&mut self.operand_stack));

                    let result = match *opcode {
                        97 => left + right,
                        101 => left - right,
                        105 => left * right,
                        109 => left / right,
                        _ => unreachable!(),
                    };

                    self.operand_stack.push(JavaType::Long { value: result });
                    self.operand_stack.push(JavaType::Filler);
                }
                // i2b
                145 => {
                    let int_val = try!(JavaType::pop_int(&mut self.operand_stack));
                    self.operand_stack.push(JavaType::Byte { value: int_val as i8 });
                }
                // return
                177 => return Ok(StepAction::EndOfMethod),
                // getstatic
                178 => {
                    let index = try!(Self::next_opcode_entry_u2(code_position,
                                                                &self.code_attribute));
                    let field = try!(Resolver::resolve_field_info(index, constant_pool));

                    if !data_store.has_class_statics(&field.class_name) {
                        code_position.reverse(3);
                        return Ok(StepAction::InitializeClass(field.class_name));
                    }

                    let field_value = data_store.get_class_static(&field.class_name, &field.name);
                    self.operand_stack.push(field_value.clone());
                }
                // putstatic
                179 => {
                    let index = try!(Self::next_opcode_entry_u2(code_position,
                                                                &self.code_attribute));
                    let field = try!(Resolver::resolve_field_info(index, constant_pool));

                    if !data_store.has_class_statics(&field.class_name) {
                        code_position.reverse(3);
                        return Ok(StepAction::InitializeClass(field.class_name));
                    }

                    let value = self.operand_stack.pop().expect("Expected value on operand stack");
                    data_store.set_class_static(&field.class_name, field.name, value);
                }
                // invokestatic
                184 => {
                    let index = try!(Self::next_opcode_entry_u2(code_position,
                                                                &self.code_attribute));
                    let method = try!(Resolver::resolve_method_info(index, constant_pool));

                    let argument_count = Self::determine_number_of_arguments(&method.descriptor);
                    debug!("Passing <{}> arguments", argument_count);

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

            // To make to easier when preparing to pass arguments
            // we'll pretend that long/double arguments count as
            // two arguments
            argument_count += match letter {
                'B' | 'C' | 'F' | 'I' | 'S' | 'Z' => 1,
                'J' | 'D' => 2,
                c @ _ => panic!("Unknown descriptor character: {}", c),
            };
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

macro_rules! generate_resolver_method {
    ($method_name:ident, $retrieval_method:ident, $struct_name:ident) => {
        pub fn $method_name(index: U2,
                                   constant_pool: &Vec<ConstantPoolItem>)
            -> StepResult<$struct_name> {
                let info = try!(ConstantPoolItem::$retrieval_method(index,
                                                                    constant_pool));
                let class_index = info.class_index;
                let name_and_type_index = info.name_and_type_index;

                let class = try!(ConstantPoolItem::retrieve_class_info(class_index, constant_pool));
                let name_and_type =
                    try!(ConstantPoolItem::retrieve_name_and_type_info(name_and_type_index,
                                                                       constant_pool));

                let class_name = try!(ConstantPoolItem::retrieve_utf8_info(class.name_index,
                                                                           constant_pool));
                let name = try!(ConstantPoolItem::retrieve_utf8_info(name_and_type.name_index,
                                                                     constant_pool));
                let descriptor = try!(ConstantPoolItem::retrieve_utf8_info(
                        name_and_type.descriptor_index,
                        constant_pool));

                Ok($struct_name {
                    class_name: class_name,
                    name: name,
                    descriptor: descriptor,
                })
            }
    }
}

generate_field_method_interface_method_struct!(InitializedFieldInfo);
generate_field_method_interface_method_struct!(InitializedMethodInfo);
generate_field_method_interface_method_struct!(InitializedInterfaceMethodInfo);

struct Resolver;

impl Resolver {
    generate_resolver_method!(resolve_method_info,
                              retrieve_method_info,
                              InitializedMethodInfo);
    generate_resolver_method!(resolve_field_info,
                              retrieve_field_info,
                              InitializedFieldInfo);
    generate_resolver_method!(resolve_interface_method_info,
                              retrieve_interface_method_info,
                              InitializedInterfaceMethodInfo);
}
