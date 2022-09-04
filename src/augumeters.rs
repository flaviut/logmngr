use core::option::Option;
use core::option::Option::Some;
use core::result::Result;
use core::result::Result::Ok;

use chrono::NaiveDateTime;

// line processing errors
#[derive(Debug)]
pub enum LineError {
    MatchFailed(),
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

impl DateAugmenter {
    fn time(&self, line: &mut json::JsonValue) -> Result<NaiveDateTime, LineError> {
        line[self.key].take_string()
            .map_or_else(|| Err(LineError::MatchFailed()), |s| Ok(s))
            .and_then(|s| if let Some(tz) = self.default_timezone {
                let dt = NaiveDateTime::parse_from_str(&s, self.fmt).map_err(LineError::DateParseFailed)?;
                Ok(chrono::DateTime::<chrono::FixedOffset>::from_local(dt, tz)
                    .naive_utc())
            } else {
                Ok(chrono::DateTime::parse_from_str(&s, self.fmt).map_err(LineError::DateParseFailed)?
                    .naive_utc())
            })
            .or_else(|_| Ok(chrono::Utc::now().naive_utc()))
    }
}

impl Augmenter for DateAugmenter {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError> {
        let dt = self.time(line)?;

        line.insert(self.key, dt.timestamp_millis()).unwrap();
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
