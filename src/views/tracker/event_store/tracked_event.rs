use crate::event::{AnnualDay, Event, Interval, MonthlyDay, State};
use crate::prelude::*;
use chrono::{Datelike, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackedEvent(Event, State);

impl TrackedEvent {
    pub fn with_state(source: Event, state: State) -> TrackedEvent {
        TrackedEvent(source, state)
    }
    pub fn complete_now(&mut self) -> LocalTime {
        let now = Local::now();
        self.1 = State::Completed(now);
        now
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
        let next = match self.next_trigger_time(&now) {
            Some(t) => t,
            None => return,
        };

        if now >= next {
            match self.state_mut() {
                State::Dormant { .. } | State::Completed(_) => {
                    self.1 = State::Triggered(vec![now]);
                }
                State::Triggered(trigger_times) => {
                    trigger_times.push(now);
                }
            }
        }
    }
    /// Returns the next time this event is going to trigger counting from given time
    pub fn next_trigger_time(&self, counting_from: &LocalTime) -> Option<LocalTime> {
        let interval = self.event().interval();
        let state = self.state();

        // Does not stack -> does not re-trigger
        if let State::Triggered(_) = state {
            if !self.event().stacks() {
                return None;
            }
        }

        match interval {
            Interval::FromLastCompletion(delta) => match &state {
                State::Dormant(registered) => Some(delta.apply_to(*registered)),
                State::Triggered(trigger_times) => {
                    Some(delta.apply_to(*trigger_times.last().unwrap()))
                }
                State::Completed(time) => Some(delta.apply_to(*time)),
            },
            Interval::Annual(AnnualDay { month, day }, time) => {
                let an_instance = UtcTime::from_utc(
                    NaiveDate::from_ymd(counting_from.year(), *month, *day).and_time(*time),
                    Utc,
                )
                .with_timezone(&Local);

                // If the constructed instance is before our time, move it one year forward and return
                Some(if &an_instance < counting_from {
                    UtcTime::from_utc(
                        NaiveDate::from_ymd(counting_from.year() + 1, *month, *day).and_time(*time),
                        Utc,
                    )
                    .with_timezone(&Local)
                } else {
                    an_instance
                })
            }
            Interval::Monthly(MonthlyDay { day }, time) => {
                let an_instance = UtcTime::from_utc(
                    NaiveDate::from_ymd(counting_from.year(), counting_from.month(), *day)
                        .and_time(*time),
                    Utc,
                )
                .with_timezone(&Local);

                // If the constructed instance is before our time, move it one month forward and return
                Some(if &an_instance < counting_from {
                    UtcTime::from_utc(
                        NaiveDate::from_ymd(counting_from.year(), counting_from.month() + 1, *day)
                            .and_time(*time),
                        Utc,
                    )
                    .with_timezone(&Local)
                } else {
                    an_instance
                })
            }
            Interval::Weekly(weekday, time) => {
                let an_instance = UtcTime::from_utc(
                    NaiveDate::from_isoywd(
                        counting_from.year(),
                        counting_from.iso_week().week(),
                        *weekday,
                    )
                    .and_time(*time),
                    Utc,
                )
                .with_timezone(&Local);

                // If the constructed instance is before our time, move it one week forward and return
                Some(if &an_instance < counting_from {
                    UtcTime::from_utc(
                        NaiveDate::from_isoywd(
                            counting_from.year(),
                            counting_from.iso_week().week() + 1,
                            *weekday,
                        )
                        .and_time(*time),
                        Utc,
                    )
                    .with_timezone(&Local)
                } else {
                    an_instance
                })
            }
        }
    }
}
