use crate::prelude::*;
use chrono::{Duration, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Interval {
    /// Interval that depends on time of completion
    FromLastCompletion(TimeDelta),
    /// A fixed interval between a certain time on multiple subsequent specified
    /// days
    Periodic(TimePeriod),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TimeDelta {
    Days(i64),
    Hm(i64, i64),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TimePeriod {
    Annual(AnnualDay, NaiveTime),
    Monthly(MonthlyDay, NaiveTime),
    Weekly(Weekday, NaiveTime),
    Daily(NaiveTime),
    MultiAnnual(Vec<AnnualDay>), // Not implemented
}

impl Interval {
    pub fn to_duration_heuristic(&self) -> Option<Duration> {
        use Interval::*;
        match self {
            FromLastCompletion(delta) => Some(delta.to_duration()),
            Periodic(period) => period.to_duration_heuristic(),
        }
    }
}

impl TimePeriod {
    pub fn to_duration_heuristic(&self) -> Option<Duration> {
        use TimePeriod::*;
        match self {
            Annual(_, _) => Some(Duration::days(365)),
            Monthly(_, _) => Some(Duration::days(30)),
            Weekly(_, _) => Some(Duration::days(7)),
            Daily(_) => Some(Duration::days(1)),
            // Returns the average amount of time between the days, which is 365 / number of days
            MultiAnnual(d) => Some(Duration::days(365 / d.len() as i64)),
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Interval::*;
        match self {
            FromLastCompletion(delta) => write!(f, "triggers {} after previous completion", delta),
            Periodic(p) => write!(f, "{}", p),
        }
    }
}

impl std::fmt::Display for TimePeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TimePeriod::*;
        match self {
            MultiAnnual(days) => write!(
                f,
                "triggers multi-annually on following dates and times {:?}",
                &days
            ),
            Annual(day, time) => write!(
                f,
                "triggers annually on {} at {}",
                day,
                time.format("%H:%M")
            ),
            Monthly(day, time) => {
                write!(f, "triggers monthly on {} at {}", day, time.format("%H:%M"))
            }
            Weekly(weekday, time) => write!(
                f,
                "triggers weekly on {} at {}",
                weekday,
                time.format("%H:%M")
            ),
            Daily(time) => write!(f, "triggers daily at {}", time.format("%H:%M")),
        }
    }
}

impl std::fmt::Display for TimeDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TimeDelta::*;
        match self {
            Days(n) => write!(f, "{} days", n),
            Hm(h, m) => {
                match h {
                    0 => write!(f, ""),
                    h => write!(f, "{}h", h),
                }?;
                match m {
                    0 => write!(f, ""),
                    m => write!(f, "{}m", m),
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn hours_minutes_formats_right() {
        let dt = TimeDelta::Hm(1, 20);
        assert_eq!(&format!("{}", dt), "1h20m");

        let dt = TimeDelta::Hm(0, 15);
        assert_eq!(&format!("{}", dt), "15m");

        let dt = TimeDelta::Hm(5, 0);
        assert_eq!(&format!("{}", dt), "5h");
    }
}

impl TimeDelta {
    pub fn apply_to(&self, time: LocalTime) -> LocalTime {
        time + self.to_duration()
    }
    pub fn to_duration(&self) -> Duration {
        match self {
            TimeDelta::Days(d) => Duration::days(*d),
            TimeDelta::Hm(h, m) => Duration::hours(*h as i64) + Duration::minutes(*m as i64),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnnualDay {
    pub month: u32,
    pub day: u32,
}

impl std::fmt::Display for AnnualDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.", self.month, self.day)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonthlyDay {
    pub day: u32,
}

impl std::fmt::Display for MonthlyDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.", self.day)
    }
}
