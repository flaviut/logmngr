extern crate pcre2;

use std::fs::File;
use std::io::{self, BufRead};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Mutex;

use clap::{Parser, Subcommand};
use rayon::prelude::*;

use crate::augmenters::{Augmenter, DateAugmenter, PathAugmenter};
use crate::parsers::{LineParser, RegexParser};
use crate::readers::IndexSearcher;
use crate::util::RegexValue;
use crate::writers::{LogWriter, PartitionWriter};

mod parsers;
mod augmenters;
mod writers;
mod util;
mod readers;

fn process_stream(
    reader: impl BufRead,
    writer: &Mutex<PartitionWriter>,
    parser: &mut dyn LineParser,
    augmenters: Vec<Box<dyn Augmenter>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut output_line: Vec<u8> = Vec::new();
    for line in reader.lines() {
        let mut result = parser.parse(&line?);
        for augmenter in &augmenters {
            augmenter.augment(&mut result);
        }

        result.write(&mut output_line)?;
        let timestamp = result["timestamp"].as_i64().unwrap();

        writer.lock().unwrap().write_log(&output_line, timestamp)?;
        output_line.clear();
    }
    Ok(())
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// selects the index location
    #[clap(long, value_name = "DIR", value_parser)]
    index: PathBuf,

    /// Sets a custom config file
    #[clap(short, long, value_parser, value_name = "FILE")]
    config: Option<PathBuf>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// loads a log file into the index
    Process {
        /// the input log file to process
        #[clap()]
        inputs: Vec<PathBuf>,
    },
    /// searches the index for a given query
    Search {
        /// the query to search for
        #[clap(value_parser)]
        query: RegexValue,

        /// the start date to search from
        #[clap(long, value_parser)]
        from: Option<chrono::NaiveDateTime>,

        /// the end date to search to
        #[clap(long, value_parser)]
        to: Option<chrono::NaiveDateTime>,
    },
}

fn main() {
    // parse args
    let cli: Cli = Cli::parse();

    match cli.command {
        Commands::Process { inputs } => {
            let writer = PartitionWriter::new(
                &cli.index,
                1024 * 1024 * 64,
            );
            let writer = Mutex::new(writer);

            inputs
                .par_iter()
                .for_each(|file_path| {
                    let file = File::open(&file_path).expect("should have been able to open the input file");
                    let reader = io::BufReader::new(file);

                    process_stream(
                        reader, &writer,
                        &mut RegexParser::new(
                            r"(?<timestamp>[\d/]{8} [\d:]{8}) (?<level>[A-Z]+) (?<component>[^:]+): (?<message>.*)$"),
                        vec![
                            Box::new(DateAugmenter {
                                default_timezone: Some(chrono::FixedOffset::east(0)),
                                fmt: "%y/%m/%d %H:%M:%S",
                                key: "timestamp",
                            }),
                            Box::new(PathAugmenter {
                                value: file_path.file_name().unwrap().to_str().unwrap().to_string(),
                            }),
                        ]).unwrap();
                });
        }
        Commands::Search { query, from, to } => {
            let searcher = IndexSearcher::load(cli.index).unwrap();

            let to = to.unwrap_or_else(|| chrono::NaiveDateTime::MAX);
            let from = from.unwrap_or_else(|| chrono::NaiveDateTime::MIN);

            if let Err(e) = searcher.search(&query.0, from.timestamp_millis(), to.timestamp_millis()) {
                if e.kind() == io::ErrorKind::BrokenPipe {
                    // being piped to another process, but the other process has closed
                    // so we can just exit
                } else {
                    panic!("Error searching: {}", e);
                }
            }
        }
    }
}