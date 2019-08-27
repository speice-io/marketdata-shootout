use std::cmp::{max, min};
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::fs::File;
use std::hash::Hasher;
use std::io::{BufRead, Read};
use std::io::Error;
use std::path::Path;
use std::time::SystemTime;

use clap::{App, Arg};

use crate::iex::IexParser;

// Cap'n'Proto and Flatbuffers typically ask that you generate code on the fly to match
// the schemas. For purposes of auto-complete and easy browsing in the repository,
// we generate the code and just copy it into the src/ tree.
pub mod marketdata_capnp;
#[allow(unused_imports)]
pub mod marketdata_generated; // Flatbuffers

mod capnp_runner;
mod iex;
mod parsers;

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

    let _start = SystemTime::now();
    let mut summarizer = Summarizer::default();
    let mut parser = IexParser::new(&buf[..]);

    let mut capnp_writer = capnp_runner::CapnpWriter::new();
    let capnp_reader = capnp_runner::CapnpReader::new();
    let mut output_buf = Vec::new();

    for iex_payload in parser {
        //let iex_payload = parser.next().unwrap();
        capnp_writer.serialize(&iex_payload, &mut output_buf, true);
    }

    let mut read_buf = StreamVec::new(output_buf);
    let mut parsed_msgs: u64 = 0;
    while let Ok(_) = capnp_reader.deserialize_packed(&mut read_buf, &mut summarizer) {
        parsed_msgs += 1;
    }

    assert_eq!(read_buf.pos, read_buf.inner.len());
    dbg!(parsed_msgs);
    dbg!(summarizer);
}

#[derive(Debug)]
pub struct SummaryStats {
    symbol: String,
    trade_volume: u64,
    bid_high: u64,
    bid_low: u64,
    ask_high: u64,
    ask_low: u64,
}

#[derive(Default, Debug)]
pub struct Summarizer {
    data: HashMap<u64, SummaryStats>
}

impl Summarizer {
    fn entry(&mut self, sym: &str) -> &mut SummaryStats {
        let mut hasher = DefaultHasher::new();
        hasher.write(sym.as_bytes());
        self.data.entry(hasher.finish())
            .or_insert(SummaryStats {
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
        StreamVec {
            pos: 0,
            inner: buf,
        }
    }
}

impl Read for StreamVec {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // TODO: There's *got* to be a better way to handle this
        let end = self.pos + buf.len();
        let end = if end > self.inner.len() { self.inner.len() } else { end };
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
