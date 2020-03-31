use crate::event::{AnnualDay, Event, Interval, MonthlyDay, State};
use crate::prelude::*;
use chrono::{Datelike, FixedOffset, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackedEvent(Event, State);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Ord, PartialOrd, PartialEq, Eq)]
#[serde(transparent)]
pub struct Uid(pub usize);

impl TrackedEvent {
    pub fn with_state(source: Event, state: State) -> TrackedEvent {
        TrackedEvent(source, state)
    }
    pub fn text(&self) -> &str {
        self.0.text()
    }
    pub fn event(&self) -> &Event {
        &self.0
    }
    pub fn state(&self) -> &State {
        &self.1
    }
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.1
    }
    pub fn update(&mut self) {
        let now = Local::now();

        // Early out if the event's not going to trigger
        let next = match self.next_trigger_time() {
            Some(t) => t,
            None => return,
        };

        if now >= next {
            self.trigger_now(now);
        }
    }
    /// Returns if event was actually triggered
    pub fn trigger_now(&mut self, now: LocalTime) -> bool {
        self.state_mut().trigger_now(now);
        true
    }
    pub fn fraction_of_interval_remaining(&self, at_time: &LocalTime) -> Option<f64> {
        let state = self.state();
        let event = self.event();

        // Does not stack -> does not re-trigger
        if let State::Triggered(_) = state {
            if !event.stacks() {
                return None;
            }
        }

        match self.next_trigger_time() {
            // Wait doesn't apply if the event is not going to trigger
            None => return None,
            Some(next) => {
                let seconds_until_next = next.signed_duration_since(*at_time).num_seconds();
                let interval_seconds = event.interval().to_duration_heuristic().num_seconds();

                match interval_seconds {
                    0 => Some(0.),
                    int => Some(seconds_until_next as f64 / int as f64),
                }
            }
        }
    }
    /// Returns the next time this event is going to trigger counting from given time
    pub fn next_trigger_time(&self) -> Option<LocalTime> {
        let interval = self.event().interval();
        let state = self.state();

        // Does not stack -> does not re-trigger
        if let State::Triggered(_) = state {
            if !self.event().stacks() {
                return None;
            }
        }

        let last_trigger = match &&state {
            State::Dormant(registered) => *registered,
            State::Triggered(trigger_times) => *trigger_times.last().unwrap(),
            State::Completed(time) => *time,
        };
        match interval {
            Interval::FromLastCompletion(delta) => Some(delta.apply_to(last_trigger)),
            Interval::Annual(AnnualDay { month, day }, time) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_ymd(last_trigger.year(), *month, *day).and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one year forward and return
                Some(if an_instance < last_trigger {
                    LocalTime::from_utc(
                        NaiveDate::from_ymd(last_trigger.year() + 1, *month, *day).and_time(*time),
                        FixedOffset::east(0),
                    )
                } else {
                    an_instance
                })
            }
            Interval::Monthly(MonthlyDay { day }, time) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_ymd(last_trigger.year(), last_trigger.month(), *day)
                        .and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one month forward and return
                Some(if an_instance < last_trigger {
                    LocalTime::from_utc(
                        NaiveDate::from_ymd(last_trigger.year(), last_trigger.month() + 1, *day)
                            .and_time(*time),
                        FixedOffset::east(0),
                    )
                } else {
                    an_instance
                })
            }
            Interval::Weekly(weekday, time) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_isoywd(
                        last_trigger.year(),
                        last_trigger.iso_week().week(),
                        *weekday,
                    )
                    .and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one week forward and return
                Some(if an_instance < last_trigger {
                    LocalTime::from_utc(
                        NaiveDate::from_isoywd(
                            last_trigger.year(),
                            last_trigger.iso_week().week() + 1,
                            *weekday,
                        )
                        .and_time(*time),
                        FixedOffset::east(0),
                    )
                } else {
                    an_instance
                })
            }
            Interval::Daily(time) => {
                let an_instance = LocalTime::from_utc(
                    NaiveDate::from_ymd(
                        last_trigger.year(),
                        last_trigger.month(),
                        last_trigger.day(),
                    )
                    .and_time(*time),
                    FixedOffset::east(0),
                );

                // If the constructed instance is before our time, move it one month forward and return
                Some(if an_instance < last_trigger {
                    LocalTime::from_utc(
                        NaiveDate::from_ymd(
                            last_trigger.year(),
                            last_trigger.month(),
                            last_trigger.day() + 1,
                        )
                        .and_time(*time),
                        FixedOffset::east(0),
                    )
                } else {
                    an_instance
                })
            }
        }
    }
}
