use crate::datamodel::*;
use crate::prelude::*;
use chrono::{Datelike, Duration, FixedOffset, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackedEvent(pub EventData, pub Status);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Ord, PartialOrd, PartialEq, Eq)]
#[serde(transparent)]
pub struct Uid(pub usize);

impl TrackedEvent {
    pub fn with_state(source: EventData, state: Status) -> TrackedEvent {
        TrackedEvent(source, state)
    }

    pub fn is_triggered(&self) -> bool {
        self.1.is_triggered()
    }
    pub fn is_done(&self) -> bool {
        self.1.is_done()
    }
    pub fn is_completed(&self) -> bool {
        self.is_done()
    }
    pub fn text(&self) -> &str {
        self.0.text()
    }
    pub fn update(&mut self) {
        let now = Local::now();

        // Early out if the event's not going to trigger
        let next = match self.next_trigger_time() {
            Some(t) => t,
            None => return,
        };

        if now >= next {
            self.trigger_now();
        }
    }
    /// Returns true if the event moved from an untriggered start to a triggered
    /// state
    pub fn trigger_at(&mut self, _t: LocalTime) -> bool {
        unimplemented!()
    }
    /// Returns true if the event moved from an untriggered start to a triggered
    /// state
    pub fn trigger_now(&mut self) -> bool {
        self.1.trigger_now()
    }
    pub fn complete_now(&mut self) -> bool {
        self.1.complete_now()
    }
    /// Returns None if the fraction cannot be evaluated
    pub fn fraction_of_interval_remaining(&self, at_time: &LocalTime) -> Option<f64> {
        let state = &self.1;
        let event = &self.0;

        // Does not stack -> does not re-trigger
        if let StatusKind::Triggered = state.status {
            if !event.stacks() {
                return None;
            }
        }

        match self.next_trigger_time() {
            // Wait doesn't apply if the event is not going to trigger
            None => None,
            Some(next) => {
                let seconds_until_next = next.signed_duration_since(*at_time).num_seconds();
                let interval_seconds = match event.interval().to_duration_heuristic() {
                    Some(d) => d.num_seconds(),
                    None => return None,
                };

                match interval_seconds {
                    0 => Some(0.),
                    int => Some(seconds_until_next as f64 / int as f64),
                }
            }
        }
    }
    /// Returns the next time this event is going to trigger. Returns None if
    /// currently triggered.
    pub fn next_trigger_time(&self) -> Option<LocalTime> {
        let interval = self.0.interval();
        let state = &self.1;

        // Does not stack -> does not re-trigger
        if let StatusKind::Triggered = state.status {
            if !self.0.stacks() {
                return None;
            }
        }

        // Start counting the next time of trigger from the previous trigger time if
        // possible, otherwise, pick 1. time of registration, 2. time of skipping, time
        // of completion
        let prev_trigger = match state.prev_trigger_time() {
            Some(t) => t,
            None => {
                match &state.status {
                    StatusKind::Dormant(t) => t,
                    StatusKind::Skip(t) => t,
                    StatusKind::Completed(t) => t,
                    _ => unreachable!(),
                }
                .0
            }
        };
        match interval {
            Interval::FromLastCompletion(delta) => Some(delta.apply_to(prev_trigger)),
            Interval::Periodic(TimePeriod::Annual(AnnualDay { month, day }, time)) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_ymd(prev_trigger.year(), *month, *day).and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one year forward and
                // return
                Some(if an_instance < prev_trigger {
                    LocalTime::from_utc(
                        NaiveDate::from_ymd(prev_trigger.year() + 1, *month, *day).and_time(*time),
                        FixedOffset::east(0),
                    )
                } else {
                    an_instance
                })
            }
            Interval::Periodic(TimePeriod::MultiAnnual(_days)) => unimplemented!(),
            Interval::Periodic(TimePeriod::Monthly(MonthlyDay { day }, time)) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_ymd(prev_trigger.year(), prev_trigger.month(), *day)
                        .and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one month forward and
                // return
                Some(if an_instance < prev_trigger {
                    LocalTime::from_utc(
                        NaiveDate::from_ymd(
                            prev_trigger.year(),
                            prev_trigger.month() % 12 + 1,
                            *day,
                        )
                        .and_time(*time),
                        FixedOffset::east(0),
                    )
                } else {
                    an_instance
                })
            }
            Interval::Periodic(TimePeriod::Weekly(weekday, time)) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_isoywd(
                        prev_trigger.year(),
                        prev_trigger.iso_week().week(),
                        *weekday,
                    )
                    .and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one week forward and
                // return
                Some(if an_instance < prev_trigger {
                    an_instance + Duration::weeks(1)
                } else {
                    an_instance
                })
            }
            Interval::Periodic(TimePeriod::Daily(time)) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_ymd(
                        prev_trigger.year(),
                        prev_trigger.month(),
                        prev_trigger.day(),
                    )
                    .and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one day forward and
                // return
                Some(if an_instance < prev_trigger {
                    an_instance + Duration::days(1)
                } else {
                    an_instance
                })
            }
        }
    }
}
