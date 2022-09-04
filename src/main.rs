extern crate pcre2;

use std::fs::File;
use std::io::{self, BufRead, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use json::codegen::{Generator, WriterGenerator};
use json::object;
use pcre2::bytes::{CaptureLocations, Regex};

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
    subject: &'a [u8],
    captures: &'a CaptureLocations,
    index: usize,
}

impl<'a, 'b> MyCapturesIter<'a, 'b> {
    fn new(re: &'b Regex, subject: &'a [u8], captures: &'a CaptureLocations) -> Self {
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

// line processing errors
#[derive(Debug)]
enum LineError {
    MatchFailed(),
    Json(json::Error),
    AugmentFailed(&'static str),
    DateParseFailed(chrono::ParseError),
}

trait Augmenter {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError>;
}

struct DateAugmenter {
    default_timezone: Option<chrono::FixedOffset>,
    fmt: &'static str,
    key: &'static str,
}

impl Augmenter for DateAugmenter {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError> {
        let date = line[self.key].take_string().ok_or(LineError::AugmentFailed("Failed to read key"))?;
        if let Some(tz) = self.default_timezone {
            let dt = chrono::NaiveDateTime::parse_from_str(&date, self.fmt).map_err(LineError::DateParseFailed)?;
            line[self.key] = chrono::DateTime::<chrono::FixedOffset>::from_local(dt, tz).to_rfc3339().into();
        } else {
            let dt = chrono::DateTime::parse_from_rfc3339(&date).map_err(|_| LineError::MatchFailed())?;
            line[self.key] = dt.to_rfc3339().into();
        }
        Ok(())
    }
}

struct RegexAugmenter {
    re: Regex,
    captures: CaptureLocations,
}

impl RegexAugmenter {
    fn new(re: &str) -> Self {
        let re = unsafe {
            pcre2::bytes::RegexBuilder::new()
                // disabling utf check is unsafe, but we've validated that the
                // lines are valid utf8 in the BufReader.
                .disable_utf_check()
                .jit_if_available(true)
                .crlf(true)
                .build(re)
                .expect("pcre regex should compile")
        };
        Self {
            captures: re.capture_locations(),
            re,
        }
    }
    fn augment(&mut self, line: &String) -> json::JsonValue {
        self.re.captures_read(&mut self.captures, line.as_bytes()).unwrap()
            .map_or_else(|| Err(LineError::MatchFailed()), |caps| Ok(caps))
            .and_then(|_| MyCapturesIter::new(&self.re, line.as_bytes(), &self.captures).try_fold(
                json::JsonValue::new_object(),
                |mut obj, cap| {
                    let name = cap.name().expect("every capture group should have a name");
                    // fine because we turn the UTF-8 string into bytes since pcre2 doesn't
                    // support working with strings
                    let value = unsafe { std::str::from_utf8_unchecked(cap.value()) };
                    obj[name] = value.into();
                    Ok(obj)
                })
            )
            .unwrap_or_else(|_| object! {message: line.to_string()})
    }
}

struct PathAugmenter<'a> {
    value: &'a str,
}

impl<'a> Augmenter for PathAugmenter<'a> {
    fn augment(&self, line: &mut json::JsonValue) -> Result<(), LineError> {
        line["filename"] = self.value.into();
        Ok(())
    }
}

fn main() {
    let path_str = std::env::args().nth(1).expect("argument 1 should be the input file path");
    let file_path = Path::new(path_str.as_str());
    let file = File::open(file_path).expect("Failed to open file");

    let reader = io::BufReader::new(file);
    let mut writer = io::BufWriter::new(io::stdout());

    // language=regexp
    let mut re_augmenter = RegexAugmenter::new(
        r"(?<date>[\d/]{8} [\d:]{8}) (?<level>[A-Z]+) (?<component>[^:]+): (?<message>.*)$");
    let date_augmenter = DateAugmenter {
        default_timezone: Some(chrono::FixedOffset::east(0)),
        fmt: "%y/%m/%d %H:%M:%S",
        key: "date",
    };
    let path_augmenter = PathAugmenter { value: std::str::from_utf8(file_path.file_name().unwrap().as_bytes()).unwrap() };

    for line in reader.lines() {
        let line = line.unwrap();
        let mut result = re_augmenter.augment(&line);
        date_augmenter.augment(&mut result);
        path_augmenter.augment(&mut result);

        result.write(&mut writer).unwrap();
        writer.write_all(b"\n").unwrap();
    }
}