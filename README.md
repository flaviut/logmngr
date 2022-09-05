# logmngr Log Processing Toolkit

This is a collection of tools for indexing and searching log files, with a
focus on keepin' it simple.

## Usage

```
$ # Index a log file or a set of log files
$ logmngr --index <DIR> process <<file>...|/proc/self/fd/0>
$ # Search the index
$ logmngr --index <DIR> search <PCRE regex>
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

### Future work

Networking, horizontal scaling and data durability have not been implemented, but
will flow quite simply out of this design:

- there can be multiple servers accepting log entries
- there can be multiple servers accepting search queries
- these can be either the same set of servers or different servers connected
  a network file system
    - network file systems work great here because we generally get 10:1
      compression
- clients can be configured to choose a server at random to send batched log
  entries to, and locally store them until the server acknowledges receipt
- queries should be sent to all servers and the results merged

### Performance

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

#### Analysis

When indexing, at the moment, multiple threads are used to process log lines,
but only one thread will compress and write to disk at a time. This explains
the poor performance: in a single-threaded test, I was seeing ~50MiB/s of
throughput.

When searching, the bottlenecks are:

- decompression
- regular expression matching (which is done by PCRE in JIT mode)
- writing to standard output (more significant when many lines are matched)
