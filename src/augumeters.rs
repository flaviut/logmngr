use core::option::Option;
use core::option::Option::Some;
use core::result::Result;
use core::result::Result::Ok;

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
        let instant = if let Some(tz) = self.default_timezone {
            let dt = chrono::NaiveDateTime::parse_from_str(&date_str, self.fmt).map_err(LineError::DateParseFailed)?;
            chrono::DateTime::<chrono::FixedOffset>::from_local(dt, tz)
        } else {
            chrono::DateTime::parse_from_str(&date_str, self.fmt).map_err(|_| LineError::MatchFailed())?
        };

        line.insert(self.key, instant.to_rfc3339()).unwrap();
        Ok(())
    }
}

pub struct PathAugmenter {
    pub value: String,
}

impl<'a> Augmenter for PathAugmenter{
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError> {
        line.insert("filename", self.value.as_str()).unwrap();
        Ok(())
    }
}
