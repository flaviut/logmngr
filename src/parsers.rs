use core::result::Result::{Err, Ok};

use json::object;
use pcre2::bytes::{CaptureLocations, Regex};

use crate::util::build_regex;

pub trait LineParser {
    fn parse(&mut self, line: &String) -> json::JsonValue;
}


struct MyCapture<'a, 'b> {
    re: &'b Regex,
    index: usize,
    val: &'a [u8],
}

impl<'a, 'b> MyCapture<'a, 'b> {
    fn name(&self) -> Option<&'b str> {
        if let Some(s) = &self.re.capture_names()[self.index] {
            Some(s.as_str())
        } else {
            None
        }
    }
    fn value(&self) -> &'a [u8] {
        self.val
    }
}

struct MyCapturesIter<'a, 'b> {
    re: &'b Regex,
    captures: &'b CaptureLocations,
    subject: &'a [u8],
    index: usize,
}

impl<'a, 'b> MyCapturesIter<'a, 'b> {
    fn new(re: &'b Regex, subject: &'a [u8], captures: &'b CaptureLocations) -> Self {
        Self { re, subject, captures, index: 0 }
    }
}

impl<'a, 'b> Iterator for MyCapturesIter<'a, 'b> {
    type Item = MyCapture<'a, 'b>;
    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.captures.len() {
            self.index += 1;
            let index = self.index;
            if let Some(capture) = self.captures.get(index) {
                return Some(MyCapture { re: self.re, index, val: &self.subject[capture.0..capture.1] });
            }
        }
        None
    }
}

pub struct RegexParser {
    re: Regex,
    captures: CaptureLocations,
}

impl RegexParser {
    pub fn new(re_pattern: &str) -> Self {
        let re = build_regex(re_pattern).expect("regex to be valid");
        Self {
            captures: re.capture_locations(),
            re,
        }
    }
}

enum LineParseError {
    MatchFailed(),
}

impl LineParser for RegexParser {
    fn parse(&mut self, line: &String) -> json::JsonValue {
        self.re.captures_read(&mut self.captures, line.as_bytes()).unwrap()
            .map_or_else(|| Err(LineParseError::MatchFailed()), |caps| Ok(caps))
            .and_then(|_| MyCapturesIter::new(&self.re, line.as_bytes(), &self.captures).try_fold(
                json::JsonValue::new_object(),
                |mut obj, cap| {
                    let name = cap.name().expect("every capture group should have a name");
                    // fine because we turn the UTF-8 string into bytes since pcre2 doesn't
                    // support working with strings
                    let value = unsafe { std::str::from_utf8_unchecked(cap.value()) };
                    obj.insert(name, value).unwrap();
                    Ok(obj)
                })
            )
            .unwrap_or_else(|_| object! {message: line.to_string()})
    }
}