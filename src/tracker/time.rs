use serde::{Deserialize, Serialize, Serializer};
use crate::prelude::*;
use chrono::Timelike;
use std::ops::Deref;

// Implements a custom serializer with lower precision timestamps, note that the full timestamp must
// exist in runtime, but the serialized format can do with second precision.
#[derive(Clone, Debug, Deserialize)]
pub struct Time(pub LocalTime);

impl Serialize for Time {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Time with truncated nanoseconds
        s.serialize_newtype_struct("Time", &self.0.with_nanosecond(0).unwrap_or(self.0.clone()))

    }
}

impl Deref for Time {
    type Target = LocalTime;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LocalTime> for Time {
    fn from(src: LocalTime) -> Self {
        Time( src )
    }
}
