use std::error;
use std::fmt;

#[derive(Debug)]
pub enum LoadError {
    FileDoesNotExist,
    FileEmpty,
    FileContentsMalformed(Box<serde_yaml::Error>),
}

impl error::Error for LoadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cannot load file")
    }
}

#[derive(Debug, Clone)]
pub enum CommandError {
}


impl error::Error for CommandError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cannot apply command")
    }
}
