use pantomime_parser::ClassFile;

use super::{VirtualMachineError, VirtualMachineResult};

use std::collections::HashMap;
use std::fs::File;
use std::fs::read_dir;
use std::path::PathBuf;
use std::rc::Rc;

pub struct BaseClassLoader {
    loaded_classes: HashMap<String, Rc<ClassFile>>,
    classfile_paths: Vec<PathBuf>,
    classfile_directories: Vec<PathBuf>,
}

impl BaseClassLoader {
    pub fn new() -> BaseClassLoader {
        BaseClassLoader {
            loaded_classes: HashMap::new(),
            classfile_paths: vec![],
            classfile_directories: vec![],
        }
    }

    pub fn add_classfile_path(&mut self, path: PathBuf) {
        if path.is_file() {
            self.classfile_paths.push(path);
        } else {
            self.classfile_directories.push(path);
        }
    }

    pub fn preload_classes(&mut self) {
        for path in &self.classfile_paths {
            let file = File::open(path).unwrap();

            let classfile = ClassFile::from(file)
                .expect(&format!("Unable to load class from: {:?}", path));
            let classname = classfile.classname()
                .expect(&format!("Unable to retrieve classname from: {:?}", path))
                .to_string();

            if self.loaded_classes.contains_key(&classname) {
                continue;
            }

            debug!("Loading class: {}", classname);
            self.loaded_classes.insert(classname, Rc::new(classfile));
        }
    }

    pub fn load_class(&mut self, name: &str) -> VirtualMachineResult<Rc<ClassFile>> {
        if self.loaded_classes.contains_key(name) {
            return self.resolve_class(name);
        }

        for directory in &self.classfile_directories {
            if let Some(classfile) = Self::inspect_directories(0, &name, &directory) {
                let classname = classfile.classname()
                    .unwrap()
                    .to_string();

                debug!("Loading class: {}", classname);
                self.loaded_classes.insert(classname, Rc::new(classfile));

                return self.resolve_class(&name);
            }
        }

        Err(VirtualMachineError::ClassNotFound(name.to_string()))
    }

    fn inspect_directories(position: usize, name: &str, path: &PathBuf) -> Option<ClassFile> {
        if let Some(package) = name.split("/").nth(position) {
            let listing = read_dir(path).unwrap();
            for item in listing {
                let item_path = item.unwrap().path();
                if item_path.file_stem().unwrap().eq(package) {
                    if item_path.is_dir() {
                        return Self::inspect_directories(position + 1, &name, &item_path);
                    } else {
                        let file = File::open(&item_path).unwrap();

                        let classfile = ClassFile::from(file)
                            .expect(&format!("Unable to load class from: {:?}", item_path));
                        return Some(classfile);
                    }
                }
            }

        }
        None
    }

    pub fn resolve_class(&self, name: &str) -> VirtualMachineResult<Rc<ClassFile>> {
        debug!("Resolving class: {}", name);
        self.loaded_classes
            .get(name)
            .map(|val| val.clone())
            .ok_or(VirtualMachineError::ClassNotFound(name.to_string()))
    }
}
