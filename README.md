# logmngr Log Processing Toolkit

This is a collection of tools for indexing and searching log files, with a
focus on keepin' it simple.

## Usage

```
$ # Index a log file or a set of log files
$ logmngr --index <DIR> process <<file>...|/proc/self/fd/0>
$ # Search the index
$ logmngr --index <DIR> search <PCRE regex>
{"timestamp":1469535138000,"level":"ERROR","component":"executor.Executor","message":"Exception in task 8.0 in stage 101.0 (TID 4380)","filename":"container_1460011102909_0176_01_000020.log"}
{"timestamp":1469535138000,"level":"ERROR","component":"executor.Executor","message":"Exception in task 37.0 in stage 101.0 (TID 4409)","filename":"container_1460011102909_0176_01_000020.log"}
{"timestamp":1469535147000,"level":"ERROR","component":"executor.Executor","message":"Exception in task 9.0 in stage 102.0 (TID 4424)","filename":"container_1460011102909_0176_01_000020.log"}
{"timestamp":1469535147000,"level":"ERROR","component":"executor.Executor","message":"Exception in task 38.0 in stage 102.0 (TID 4453)","filename":"container_1460011102909_0176_01_000020.log"}
```

## Architecture

I've kept things as simple as possible. There's no complicated indexing and
each log entry is stored as a single JSON line.

The index is a directory of files,

- each compressed with zstd -1
- each containing a list of log entries, one per line, in JSON format
- each file named `<epochms min>-<epochms max>-<random>.json.zst`
- each file with a nominal uncompressed size of x MiB

### Indexing

The log entries are stored in the order received, and the minimum and maximum
timestamps of the entries in that file are stored in the file name. An additional
random component is added to the file name to avoid collisions when there are
multiple threads or machines writing to the same index.

When creating the index, existing files are never modified. Instead, new files
are created. This makes backups, log retention, and caching straightforward.

### Searching

Search is performed by filtering the log files based on the timestamp range that
the user is interested in. Then, in parallel, each file is decompressed and searched
with the given regular expression in chunks of 64KiB. Each matched line is written to
standard output.

## Performance

The dataset used for testing were the [2.8GiB Spark job logs from Loghub][spark-logs].

All files were moved to a single directory from subdirectories, and lines without
a timestamp were removed using `sed -i -E '/^[0-9][0-9]\/[0-9][0-9]\/[0-9][0-9] [0-9][0-9]:[0-9][0-9]:[0-9][0-9]/!d' *`.
2.61GiB of data remained across the various files.

Testing was done on an AMD Ryzen 7 2700X CPU, with an NVME SSD and the entire
dataset loaded into the page cache.

[spark-logs]: https://github.com/logpai/loghub/tree/master/Spark

Indexing this data took 30s wall-clock, some trivial amount of RAM, and 254s of
CPU time. 2.61GiB of data was compressed to 321MiB, giving a compression ratio
of 8.3:1, at a rate of 89MiB/s.

Searching for "ERROR" takes 0.4s wall-clock and 5.6s CPU time, a rate of 6.5GiB/s.
11133 lines were matched.

Searching for "INFO" takes 0.7s wall-clock and 10.0s CPU time, a rate of 3.8GiB/s.
27074351 lines were matched.

### Analysis

When indexing, at the moment, multiple threads are used to process log lines,
but only one thread will compress and write to disk at a time. This explains
the poor performance: in a single-threaded test, I was seeing ~50MiB/s of
throughput.

When searching, the bottlenecks are:

- decompression
- regular expression matching (which is done by PCRE in JIT mode)
- writing to standard output (more significant when many lines are matched)

## Potential future work

- Any tests whatsoever
- Improve reliability--replace unwrap(), expect(), etc. with proper error handling
- Implement some level of data durability
- Spread search load across multiple machines
- Remove bottleneck in indexing by using multiple threads to compress and write
  different files rather than compressing and writing a single file at a time
- Allow indexing from a TCP socket
- Allow indexing from a TLS TCP socket
- Allow indexing from HTTP
- File-based configuration of
  - the log parser (the text -> json step)
  - log augmenters (add or normalize fields)
  - output options (compression, chunk size)
- Dynamic configuration over HTTP

## License

Licensed under the AGPLv3. See the LICENSE file for details. There are [many
misconceptions about the AGPLv3.][agpl-misc]

In short: you must distribute this source (or link here) if users can access
this software over a network, but your software does not become AGPLv3 just
because it makes network calls to this software.

[agpl-misc]: https://drewdevault.com/2020/07/27/Anti-AGPL-propaganda.html