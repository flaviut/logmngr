use core::option::Option;
use core::option::Option::Some;
use core::result::Result;
use core::result::Result::Ok;

use chrono::{Datelike, Timelike};

// line processing errors
#[derive(Debug)]
pub enum LineError {
    MatchFailed(),
    AugmentFailed(&'static str),
    DateParseFailed(chrono::ParseError),
}

pub trait Augmenter {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError>;
}

pub struct DateAugmenter {
    pub default_timezone: Option<chrono::FixedOffset>,
    pub fmt: &'static str,
    pub key: &'static str,
}

impl Augmenter for DateAugmenter {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError> {
        let date_str = line[self.key].take_string().ok_or(LineError::AugmentFailed("Failed to read key"))?;
        let dt = if let Some(tz) = self.default_timezone {
            let dt = chrono::NaiveDateTime::parse_from_str(&date_str, self.fmt).map_err(LineError::DateParseFailed)?;
            chrono::DateTime::<chrono::FixedOffset>::from_local(dt, tz)
                .naive_utc()
        } else {
            chrono::DateTime::parse_from_str(&date_str, self.fmt).map_err(|_| LineError::MatchFailed())?
                .naive_utc()
        };

        line.insert(self.key,
                    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                            dt.year(), dt.month(), dt.day(),
                            dt.hour(), dt.minute(), dt.second(), dt.timestamp_subsec_millis())).unwrap();
        Ok(())
    }
}

pub struct PathAugmenter {
    pub value: String,
}

impl<'a> Augmenter for PathAugmenter {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError> {
        line.insert("filename", self.value.as_str()).unwrap();
        Ok(())
    }
}
