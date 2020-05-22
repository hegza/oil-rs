use super::event_store::Uid as EventUid;
use std::error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub enum CommandError {
    EventNotFound(EventUid),
    InvalidReceiver(String),
}

#[derive(Debug)]
pub enum LoadError {
    FileDoesNotExist,
    // Parameters: error, path, contents
    FileContentsMalformed(Box<serde_yaml::Error>, String, String),
}

#[derive(Debug)]
pub enum StoreError {
    FileCreateFailed,
    WriteFailed,
    SerializeFailed(Box<serde_yaml::Error>),
}

/// Represents a situation where an item was added to a map that already had an
/// item with the key. Parameters: key, old value, new value.
#[derive(Debug)]
pub struct ItemAlreadyExistsError<K, V>(pub K, pub V, pub V)
where
    K: Debug,
    V: Debug;

/// Represents a situation where an item was not found from a map
#[derive(Debug)]
pub struct NotFoundError<K>(pub K)
where
    K: Debug;

impl error::Error for LoadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl Display for LoadError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "cannot load file")
    }
}

impl error::Error for StoreError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl Display for StoreError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "cannot store file")
    }
}

impl<K, V> error::Error for ItemAlreadyExistsError<K, V>
where
    K: Debug,
    V: Debug,
{
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl<K, V> Display for ItemAlreadyExistsError<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "cannot insert pair with key: {:?}, because {:?} would replace {:?}",
            &self.0, &self.2, &self.1
        )
    }
}

impl<K> error::Error for NotFoundError<K>
where
    K: Debug,
{
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl<K> Display for NotFoundError<K>
where
    K: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "item not found for key: {:?}", self.0)
    }
}

impl error::Error for CommandError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "cannot apply command")
    }
}
