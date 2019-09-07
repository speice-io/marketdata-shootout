use std::cmp::{max, min};
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::fs::File;
use std::hash::Hasher;
use std::io::Error;
use std::io::{BufRead, Read};
use std::path::Path;
use std::str::from_utf8_unchecked;
use std::time::Instant;

use clap::{App, Arg};
use hdrhistogram::Histogram;
use nom::{bytes::complete::take_until, IResult};

use crate::iex::{IexParser, IexPayload};

// Cap'n'Proto and Flatbuffers typically ask that you generate code on the fly to match
// the schemas. For purposes of auto-complete and easy browsing in the repository,
// we generate the code and just copy it into the src/ tree.
pub mod marketdata_capnp;
#[allow(unused_imports)]
pub mod marketdata_generated; // Flatbuffers
#[allow(dead_code)]
pub mod marketdata_sbe;

mod capnp_runner;
mod flatbuffers_runner;
mod iex;
mod parsers;
mod sbe_runner;

fn main() {
    let matches = App::new("Marketdata Shootout")
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .value_name("FILE")
                .help("IEX DEEP file to process")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let deep = matches.value_of("file").unwrap();
    let path = Path::new(deep);
    let mut file = File::open(path).expect(&format!("Unable to open file={}", path.display()));

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .expect(&format!("Unable to read file={}", path.display()));

    let _capnp_unpacked = run_analysis(
        &buf,
        &mut capnp_runner::CapnpWriter::new(false),
        &mut capnp_runner::CapnpReader::new(false),
    );

    let _capnp_packed = run_analysis(
        &buf,
        &mut capnp_runner::CapnpWriter::new(true),
        &mut capnp_runner::CapnpReader::new(true),
    );

    let _flatbuffers = run_analysis(
        &buf,
        &mut flatbuffers_runner::FlatbuffersWriter::new(),
        &mut flatbuffers_runner::FlatbuffersReader::new(),
    );

    let _sbe = run_analysis(
        &buf,
        &mut sbe_runner::SBEWriter::new(),
        &mut sbe_runner::SBEReader::new(),
    );
}

#[derive(Debug, PartialEq)]
pub struct SummaryStats {
    symbol: String,
    trade_volume: u64,
    bid_high: u64,
    bid_low: u64,
    ask_high: u64,
    ask_low: u64,
}

#[derive(Default, Debug, PartialEq)]
pub struct Summarizer {
    data: HashMap<u64, SummaryStats>,
}

impl Summarizer {
    fn entry(&mut self, sym: &str) -> &mut SummaryStats {
        let mut hasher = DefaultHasher::new();
        hasher.write(sym.as_bytes());
        self.data.entry(hasher.finish()).or_insert(SummaryStats {
            symbol: sym.to_string(),
            trade_volume: 0,
            bid_high: 0,
            bid_low: u64::max_value(),
            ask_high: 0,
            ask_low: u64::max_value(),
        })
    }

    pub fn append_trade_volume(&mut self, sym: &str, volume: u64) {
        self.entry(sym).trade_volume += volume;
    }

    pub fn update_quote_prices(&mut self, sym: &str, price: u64, is_buy: bool) {
        let entry = self.entry(sym);
        if is_buy {
            entry.bid_low = min(entry.bid_low, price);
            entry.bid_high = max(entry.bid_high, price);
        } else {
            entry.ask_low = min(entry.ask_low, price);
            entry.ask_high = max(entry.ask_high, price);
        }
    }
}

pub struct StreamVec {
    pos: usize,
    inner: Vec<u8>,
}

impl StreamVec {
    pub fn new(buf: Vec<u8>) -> StreamVec {
        StreamVec { pos: 0, inner: buf }
    }
}

impl Read for StreamVec {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // TODO: There's *got* to be a better way to handle this
        let end = self.pos + buf.len();
        let end = if end > self.inner.len() {
            self.inner.len()
        } else {
            end
        };
        let read_size = end - self.pos;
        buf[..read_size].copy_from_slice(&self.inner[self.pos..end]);
        self.pos = end;

        Ok(read_size)
    }
}

impl BufRead for StreamVec {
    fn fill_buf(&mut self) -> Result<&[u8], Error> {
        Ok(&self.inner[self.pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos += amt;
    }
}

trait RunnerSerialize {
    fn serialize(&mut self, payload: &IexPayload, output: &mut Vec<u8>);
}

trait RunnerDeserialize {
    fn deserialize<'a>(&self, buf: &'a mut StreamVec, stats: &mut Summarizer) -> Result<(), ()>;
}

fn __take_until<'a>(tag: &'static str, input: &'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    take_until(tag)(input)
}

fn parse_symbol(sym: &[u8; 8]) -> &str {
    // TODO: Use the `jetscii` library for all that SIMD goodness
    // IEX guarantees ASCII, so we're fine using an unsafe conversion
    let (_, sym_bytes) = __take_until(" ", &sym[..]).unwrap();
    unsafe { from_utf8_unchecked(sym_bytes) }
}

struct RunAnalysis {
    serialize_hist: Histogram<u64>,
    deserialize_hist: Histogram<u64>,
    summary_stats: Summarizer,
    serialize_total_nanos: u128,
    deserialize_total_nanos: u128,
    buf_len: usize,
}

fn run_analysis<S, D>(iex_data: &Vec<u8>, serializer: &mut S, deserializer: &mut D) -> RunAnalysis
where
    S: RunnerSerialize,
    D: RunnerDeserialize,
{
    let upper = if cfg!(debug_assertions) {
        1_000_000
    } else {
        100_000
    };
    let iex_parser = IexParser::new(iex_data);

    let mut output_buf = Vec::with_capacity(iex_data.len());
    let mut serialize_hist = Histogram::<u64>::new_with_bounds(1, upper, 2).unwrap();
    let mut serialize_nanos_total = 0u128;
    let mut serialize_msgs = 0;

    for iex_payload in iex_parser {
        let serialize_start = Instant::now();

        serializer.serialize(&iex_payload, &mut output_buf);

        let serialize_end = Instant::now().duration_since(serialize_start).as_nanos();
        serialize_hist.record(serialize_end as u64).unwrap();
        serialize_nanos_total += serialize_end;
        serialize_msgs += 1;
    }
    let output_len = output_buf.len();

    let mut read_buf = StreamVec::new(output_buf);
    let mut summarizer = Summarizer::default();
    let mut deserialize_hist = Histogram::<u64>::new_with_bounds(1, upper, 2).unwrap();
    let mut parsed_msgs: u64 = 0;
    let mut deserialize_nanos_total = 0u128;

    loop {
        let deserialize_start = Instant::now();

        let res = deserializer.deserialize(&mut read_buf, &mut summarizer);

        let deserialize_end = Instant::now().duration_since(deserialize_start).as_nanos();

        if res.is_ok() {
            deserialize_hist.record(deserialize_end as u64).unwrap();
            deserialize_nanos_total += deserialize_end;
            parsed_msgs += 1;
        } else {
            break;
        }
    }

    dbg!(serialize_msgs, parsed_msgs);

    RunAnalysis {
        serialize_hist,
        deserialize_hist,
        summary_stats: summarizer,
        serialize_total_nanos: serialize_nanos_total,
        deserialize_total_nanos: deserialize_nanos_total,
        buf_len: output_len,
    }
}
