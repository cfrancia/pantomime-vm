use pantomime_parser::ClassFile;

use super::{VirtualMachineError, VirtualMachineResult};

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

pub struct BaseClassLoader {
    loaded_classes: HashMap<String, ClassFile>,
    classfile_paths: Vec<PathBuf>,
}

impl BaseClassLoader {
    pub fn new() -> BaseClassLoader {
        BaseClassLoader {
            loaded_classes: HashMap::new(),
            classfile_paths: vec![],
        }
    }

    pub fn add_classfile_path(&mut self, path: PathBuf) {
        self.classfile_paths.push(path);
    }

    pub fn load_class(&mut self, name: &str) -> VirtualMachineResult<&ClassFile> {
        for path in &self.classfile_paths {
            let file = File::open(path).unwrap();

            let classfile = try!(ClassFile::from(file));
            let classname = try!(classfile.classname()).to_string();

            debug!("Loading class: {}", classname);
            self.loaded_classes.insert(classname, classfile);
        }

        debug!("Resolving class: {}", name);
        self.loaded_classes.get(name).ok_or(VirtualMachineError::ClassNotFound(name.to_string()))
    }
}