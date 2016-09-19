extern crate pantomime_vm;

#[macro_use]
extern crate log;

use pantomime_vm::VirtualMachine;

use log::{Log, LogLevel, LogLevelFilter, LogMetadata, LogRecord, SetLoggerError};

use std::env::args;
use std::path::PathBuf;

fn main() {
    ConsoleLogger::init().unwrap();

    info!("Starting VM...");
    let mut virtual_machine = VirtualMachine::new();

    if args().len() < 2 {
        panic!("You must provide at least a single path to a classfile and the main class!");
    }

    for arg in args().skip(1).take(args().len() - 2) {
        info!("Adding path: {}", arg);
        virtual_machine.add_classfile_path(PathBuf::from(arg));
    }

    let main_class = args().last().unwrap();
    info!("Main class: {}", main_class);

    virtual_machine.start(&main_class);
}

struct ConsoleLogger;

impl ConsoleLogger {
    pub fn init() -> Result<(), SetLoggerError> {
        log::set_logger(|max_log_level| {
            max_log_level.set(LogLevelFilter::Debug);
            Box::new(ConsoleLogger)
        })
    }
}

impl Log for ConsoleLogger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= LogLevel::Debug
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            println!("{}: {}", record.level(), record.args());
        }
    }
}
