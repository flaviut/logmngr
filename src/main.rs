extern crate pcre2;

use std::fs::File;
use std::io::{self, BufRead, BufWriter, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use json::JsonValue;
use rand::distributions::DistString;

use crate::augumeters::{Augmenter, DateAugmenter, PathAugmenter};
use crate::parsers::{LineParser, RegexParser};

mod parsers;
mod augumeters;

struct MeasuringWriter {
    pub bytes: usize,
    target: Box<dyn Write>,
}

impl MeasuringWriter {
    pub fn new(target: Box<dyn Write>) -> MeasuringWriter {
        MeasuringWriter {
            bytes: 0,
            target,
        }
    }
}

impl Write for MeasuringWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes += buf.len();
        self.target.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.target.flush()
    }
}

struct OutputFile {
    temp_file_path: Box<Path>,
    writer: MeasuringWriter,
}

struct ChunkedWriter {
    directory: Box<Path>,
    max_size: u64,

    /** the temporary file we're currently writing to */
    temp_file: Option<OutputFile>,

    min_time: u64,
    max_time: u64,
}

// Splits our processed log lines into files based on the timestamp
// This is very similar to postgres' BRIN index--these blocks may overlap
impl ChunkedWriter {
    fn new(directory: Box<Path>, max_size: u64) -> ChunkedWriter {
        ChunkedWriter {
            directory,
            max_size,
            temp_file: None,
            min_time: u64::MAX,
            max_time: u64::MIN,
        }
    }

    fn begin(&mut self) -> io::Result<()> {
        assert!(self.temp_file.is_none());

        let rand_suffix = rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 8);
        let path = self.directory.join(format!(".part-{}.json.zst", rand_suffix));
        let file = File::create(&path)?;

        let writer = {
            let mut w = zstd::Encoder::new(file, 1).unwrap();
            w.include_checksum(true).unwrap();
            w.auto_finish()
        };
        let writer = MeasuringWriter::new(Box::new(writer));

        self.temp_file = Some(OutputFile {
            temp_file_path: Box::from(path),
            writer,
        });
        Ok(())
    }

    fn write(&mut self, line: JsonValue) -> io::Result<()> {
        if self.temp_file.is_none() {
            self.begin()?;
        }
        let temp_file = self.temp_file.as_mut().unwrap();

        let time = line["timestamp"].as_u64().expect("timestamp should be present in each log line");
        self.min_time = self.min_time.min(time);
        self.max_time = self.max_time.max(time);

        let writer = &mut temp_file.writer;
        line.write(writer)?;
        writer.write(b"\n")?;

        if writer.bytes > self.max_size as usize {
            self.finalize()?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> io::Result<()> {
        let temp_file = self.temp_file.as_mut().unwrap();
        temp_file.writer.flush()?;

        let rand_suffix = rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 8);
        let path = self.directory.join(format!("{}-{}-{}.json.zst", self.min_time, self.max_time, rand_suffix));
        std::fs::rename(&temp_file.temp_file_path, &path)?;
        self.temp_file = None;
        self.max_time = u64::MIN;
        self.min_time = u64::MAX;

        Ok(())
    }
}

fn process_stream(
    reader: impl BufRead,
    writer: &mut ChunkedWriter,
    parser: &mut dyn LineParser,
    augmenters: Vec<Box<dyn Augmenter>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for line in reader.lines() {
        let mut result = parser.parse(&line?);
        for augmenter in &augmenters {
            augmenter.augment(&mut result).unwrap_or_else(|err| {
                // eprintln!("Failed to augment line: {:?}", err)
            });
        }

        writer.write(result)?;
    }
    Ok(())
}

fn main() {
    let path_str = std::env::args().nth(1).expect("argument 1 should be the input file path");
    let file_path = Path::new(path_str.as_str());
    let file = File::open(file_path).expect("Failed to open file");

    let reader = io::BufReader::new(file);
    let mut writer = ChunkedWriter::new(Box::from(Path::new("/home/user/tmp/logs/chunks/")), 1024 * 1024 * 64);

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