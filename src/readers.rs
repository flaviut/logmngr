use std::io::{IoSlice, stdout, Write};
use std::{io, os};
use std::path::PathBuf;

use linereader::LineReader;
use memchr::memmem;
use pcre2::bytes::Regex;
use rayon::prelude::*;

pub struct IndexSearcher {
    // list of partitions by their start and end date
    partitions: Vec<(i64, i64, PathBuf)>,
}

fn parse_filename(filename: &str) -> Option<(i64, i64)> {
    let mut parts = filename.split('-');
    let start = parts.next()?.parse().ok()?;
    let end = parts.next()?.parse().ok()?;
    Some((start, end))
}

impl IndexSearcher {
    pub fn load(directory_path: PathBuf) -> Result<IndexSearcher, Box<dyn std::error::Error>> {
        let mut partitions = Vec::new();

        for entry in std::fs::read_dir(&directory_path)? {
            let entry = entry?;
            let file_name = entry.file_name().into_string()
                .map_err(|_| "Invalid filename");
            if file_name.is_err() { continue; }
            let file_name = file_name.unwrap();

            let parsed = parse_filename(&file_name);
            if parsed.is_none() { continue; }
            let (start, end) = parsed.unwrap();

            partitions.push((start, end, entry.path()));
        }

        partitions.sort_by_key(|(start, end, _)| (*start, *end));

        Ok(IndexSearcher {
            partitions,
        })
    }

    fn search_partition(&self, query: &Regex, partition: &PathBuf) -> Result<(), io::Error> {
        let file = std::fs::File::open(partition)?;
        let reader = zstd::Decoder::new(file)?;
        let mut reader = LineReader::new(reader);

        while let Some(lines) = reader.next_batch() {
            let lines = lines.expect("read error");
            // line is a &[u8] owned by reader.

            let mut line_index = vec![0usize];
            line_index.extend(memmem::find_iter(&lines, "\n"));

            let mut bitset: Vec<bool> = Vec::new();
            bitset.resize(line_index.len(), false);

            for m in query.find_iter(lines) {
                let m = m.expect("regex error");
                // find the line number
                let lineno = line_index.binary_search(&m.start()).unwrap_or_else(|x| x - 1);
                bitset[lineno] = true;
            }

            let lines = bitset.iter()
                .enumerate().filter(|(_, b)| **b).map(|(i, _)| i)
                .map(|i| IoSlice::new(&lines[line_index[i]..line_index[i + 1]]))
                .collect::<Vec<_>>();

            stdout().write_vectored(&lines)?;
        }
        Ok(())
    }

    pub fn search(&self, query: &Regex, from: i64, to: i64) -> Result<(), io::Error> {
        // filter all partitions that don't overlap with the search range
        self.partitions.par_iter()
            .filter(|(start, end, _)| {
                // partition start is before the search end
                *start <= to &&
                    // partition end is after the search start
                    *end >= from
            })
            .try_for_each(|(_, _, path)| {
                self.search_partition(query, path)
            })
    }
}