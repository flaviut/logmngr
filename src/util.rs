use std::ffi::OsStr;

use clap::{Arg, Command, Error, ErrorKind};
use pcre2::bytes::Regex;

pub fn build_regex(re: &str) -> Result<Regex, pcre2::Error> {
    unsafe {
        pcre2::bytes::RegexBuilder::new()
            // disabling utf check is unsafe, but we've validated that the
            // lines are valid utf8 in the BufReader.
            .disable_utf_check()
            .jit_if_available(true)
            .crlf(true)
            .build(re)
    }
}

#[derive(Clone, Debug)]
pub struct RegexValue(pub Regex);

#[derive(Clone, Debug)]
pub struct RegexValueParser;

impl clap::builder::TypedValueParser for RegexValueParser {
    type Value = RegexValue;

    fn parse_ref(&self, _cmd: &Command, _arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, Error> {
        let re = value.to_str()
            .ok_or_else(|| Error::raw(ErrorKind::InvalidValue, "invalid utf-8 for regex"))?;
        build_regex(re)
            .map_err(|e| Error::raw(ErrorKind::InvalidValue, e.to_string()))
            .map(RegexValue)
    }
}

impl clap::builder::ValueParserFactory for RegexValue {
    type Parser = RegexValueParser;

    fn value_parser() -> Self::Parser {
        RegexValueParser
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HumanReadableDate(pub chrono::NaiveDateTime);

#[derive(Clone, Debug)]
pub struct HumanReadableDateParser;

impl clap::builder::TypedValueParser for HumanReadableDateParser {
    type Value = HumanReadableDate;

    fn parse_ref(&self, _cmd: &Command, _arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, Error> {
        let date = value.to_str()
            .ok_or_else(|| Error::raw(ErrorKind::InvalidValue, "invalid utf-8 for date"))?;
        chrono_english::parse_date_string(date, chrono::Local::now(), chrono_english::Dialect::Uk)
            .map_err(|e| Error::raw(ErrorKind::InvalidValue, e.to_string()))
            .map(|dt| dt.naive_utc())
            .map(HumanReadableDate)
    }
}

impl clap::builder::ValueParserFactory for HumanReadableDate {
    type Parser = HumanReadableDateParser;

    fn value_parser() -> Self::Parser {
        HumanReadableDateParser
    }
}