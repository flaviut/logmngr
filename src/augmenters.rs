use core::option::Option;
use core::option::Option::Some;

use chrono::NaiveDateTime;

pub trait Augmenter {
    fn augment(&self, line: &mut json::JsonValue);
}

pub struct DateAugmenter {
    pub default_timezone: Option<chrono::FixedOffset>,
    pub fmt: &'static str,
    pub key: &'static str,
}

impl DateAugmenter {
    fn time(&self, line: &mut json::JsonValue) -> NaiveDateTime {
        line[self.key].take_string()
            .and_then(|s| if let Some(tz) = self.default_timezone {
                let dt = NaiveDateTime::parse_from_str(&s, self.fmt).ok()?;
                Some(chrono::DateTime::<chrono::FixedOffset>::from_local(dt, tz)
                    .naive_utc())
            } else {
                Some(chrono::DateTime::parse_from_str(&s, self.fmt).ok()?
                    .naive_utc())
            })
            .unwrap_or_else(|| chrono::Utc::now().naive_utc())
    }
}

impl Augmenter for DateAugmenter {
    fn augment(&self, line: &mut json::JsonValue) {
        let dt = self.time(line);
        line.insert(self.key, dt.timestamp_millis()).unwrap();
    }
}

pub struct PathAugmenter {
    pub value: String,
}

impl<'a> Augmenter for PathAugmenter {
    fn augment(&self, line: &mut json::JsonValue) {
        line.insert("filename", self.value.as_str()).unwrap();
    }
}
