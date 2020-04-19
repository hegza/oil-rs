pub use log::{debug, error, info, trace, warn};
pub use std::convert::TryFrom;

pub type LocalTime = DateTime<Local>;
pub type UtcTime = DateTime<Utc>;

use chrono::{DateTime, Local, Utc};

// TODO: make util.rs or something
pub fn is_today(time: &LocalTime) -> bool {
    let today = Local::now().date();
    let date = time.date();
    today == date
}
