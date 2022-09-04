extern crate pcre2;

use std::fs::File;
use std::io::{self, BufRead};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::augumeters::{Augmenter, DateAugmenter, PathAugmenter};
use crate::parsers::{LineParser, RegexParser};
use crate::writers::{LogWriter, PartitionWriter};

mod parsers;
mod augumeters;
mod writers;

fn process_stream(
    reader: impl BufRead,
    writer: &mut impl LogWriter,
    parser: &mut dyn LineParser,
    augmenters: Vec<Box<dyn Augmenter>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for line in reader.lines() {
        let mut result = parser.parse(&line?);
        for augmenter in &augmenters {
            augmenter.augment(&mut result);
            // .unwrap_or_else(|err| { eprintln!("Failed to augment line: {:?}", err)            });
        }

        writer.write_log(&result)?;
    }
    Ok(())
}

fn main() {
    let path_str = std::env::args().nth(1).expect("argument 1 should be the input file path");
    let file_path = Path::new(path_str.as_str());
    let file = File::open(file_path).expect("Failed to open file");

    let reader = io::BufReader::new(file);
    let mut writer = PartitionWriter::new(
        Path::new("/home/user/tmp/logs/chunks/"),
        1024 * 1024 * 64,
    );

    process_stream(
        reader, &mut writer,
        &mut RegexParser::new(
            r"(?<timestamp>[\d/]{8} [\d:]{8}) (?<level>[A-Z]+) (?<component>[^:]+): (?<message>.*)$"),
        vec![
            Box::new(DateAugmenter {
                default_timezone: Some(chrono::FixedOffset::east(0)),
                fmt: "%y/%m/%d %H:%M:%S",
                key: "timestamp",
            }),
            Box::new(PathAugmenter {
                value: std::str::from_utf8(file_path.file_name().unwrap().as_bytes()).unwrap().to_string(),
            }),
        ]).unwrap();
}