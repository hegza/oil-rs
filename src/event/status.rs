use super::*;
use crate::tracker::Time;
use std::default::Default;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    trigger_times: Vec<Time>,
    pub status: StatusKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum StatusKind {
    // Never before triggered or completed with time of registration
    Dormant(Time),
    // Triggered
    Triggered,
    // Completed and ready to trigger again with time of completion
    Completed(Time),
    // Skipped until next trigger, with time of skip
    Skip(Time),
}

/// Implements PartialEq such that Dormant(t), Completed(t) and Skip(t) instances are equal regardless of inner time value.
impl PartialEq for StatusKind {
    fn eq(&self, other: &Self) -> bool {
        use StatusKind::*;
        match (self, other) {
            (Dormant(_), Dormant(_)) => true,
            (Completed(_), Completed(_)) => true,
            (Skip(_), Skip(_)) => true,
            (Triggered, Triggered) => true,
            // Cover the false cases to future-proof and cause a compile error when a new variant is added.
            (Dormant(_), _) => false,
            (Triggered, _) => false,
            (Completed(_), _) => false,
            (Skip(_), _) => false,
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Status {
            trigger_times: vec![],
            status: StatusKind::Dormant(Time::now()),
        }
    }
}

impl Default for StatusKind {
    fn default() -> Self {
        StatusKind::Dormant(Time::now())
    }
}

impl Status {
    /// Returns true if status is exactly `Triggered`
    pub fn is_triggered(&self) -> bool {
        match self.status {
            StatusKind::Dormant(_) | StatusKind::Completed(_) | StatusKind::Skip(_) => false,
            StatusKind::Triggered => true,
        }
    }
    /// Returns true if status is exactly `Completed`
    pub fn is_done(&self) -> bool {
        match self.status {
            StatusKind::Dormant { .. } | StatusKind::Triggered { .. } | StatusKind::Skip { .. } => {
                false
            }
            StatusKind::Completed { .. } => true,
        }
    }

    /// Returns true if the event moved from an untriggered start to a triggered state
    pub fn trigger_now(&mut self) -> bool {
        let now = Time::now();
        match &self.status {
            StatusKind::Dormant { .. } | StatusKind::Completed(_) => {
                self.status = StatusKind::Triggered;
                self.trigger_times = vec![now];
                true
            }
            StatusKind::Skip(_time_of_skip) => {
                self.status = StatusKind::Completed(now);
                self.trigger_times = vec![];
                false
            }
            // Just add another triggering for already triggered events
            StatusKind::Triggered => {
                self.trigger_times.push(now);
                false
            }
        }
    }
    /// Returns true if the event was completed at this time as effect of this function. Resets the list of trigger times to vec![]. Sets the item completed even if the item is set to be skipped.
    pub fn complete_now(&mut self) -> bool {
        let now = Time::now();
        let ret;
        if let StatusKind::Completed(_) = self.status {
            ret = true;
        } else {
            ret = false;
        }

        self.trigger_times = vec![];
        self.status = StatusKind::Completed(now);

        ret
    }
    /// Sets the event as skipped until the next trigger time
    pub fn skip_now(&mut self) {
        let now = Time::now();
        *self = Status {
            trigger_times: vec![],
            status: StatusKind::Skip(now),
        };
    }

    pub fn prev_trigger_time(&self) -> Option<LocalTime> {
        self.trigger_times.last().and_then(|t| Some(t.0))
    }
}
