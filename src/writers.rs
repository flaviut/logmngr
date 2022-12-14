use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result::Ok;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

use rand::distributions::DistString;

struct MeasuringWriter {
    pub bytes: usize,
    target: Box<dyn Write + Send>,
}

impl MeasuringWriter {
    pub fn new(target: Box<dyn Write + Send>) -> MeasuringWriter {
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

pub trait LogWriter {
    fn write_log(&mut self, log_line: &[u8], timestamp: i64) -> io::Result<()>;
}

struct OutputFile {
    temp_file_path: Box<Path>,
    writer: MeasuringWriter,
}

pub struct PartitionWriter {
    directory: Box<Path>,
    max_size: usize,

    /** the temporary file we're currently writing to */
    temp_file: Option<OutputFile>,

    min_time: i64,
    max_time: i64,
}

// Splits our processed log lines into files based on the timestamp
// This is very similar to postgres' BRIN index--these blocks may overlap
impl PartitionWriter {
    pub fn new(directory: &Path, max_size: usize) -> PartitionWriter {
        PartitionWriter {
            directory: Box::from(directory),
            max_size,
            temp_file: None,
            min_time: i64::MAX,
            max_time: i64::MIN,
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


    fn finalize(&mut self) -> io::Result<()> {
        let temp_file = self.temp_file.as_mut().unwrap();
        temp_file.writer.flush()?;

        let rand_suffix = rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), 8);
        let path = self.directory.join(format!("{}-{}-{}.json.zst", self.min_time, self.max_time, rand_suffix));
        std::fs::rename(&temp_file.temp_file_path, &path)?;
        self.temp_file = None;
        self.max_time = i64::MIN;
        self.min_time = i64::MAX;

        Ok(())
    }
}

impl LogWriter for PartitionWriter {
    fn write_log(&mut self, log_line: &[u8], timestamp: i64) -> io::Result<()> {
        if self.temp_file.is_none() {
            self.begin()?;
        }
        let temp_file = self.temp_file.as_mut().unwrap();

        self.min_time = self.min_time.min(timestamp);
        self.max_time = self.max_time.max(timestamp);

        temp_file.writer.write(log_line)?;
        temp_file.writer.write(b"\n")?;

        if temp_file.writer.bytes > self.max_size as usize {
            self.finalize()?;
        }

        Ok(())
    }
}
