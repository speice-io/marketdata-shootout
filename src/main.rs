use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use clap::{App, Arg};

use crate::iex::{IexMessage, IexParser};

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

    let start = SystemTime::now();

    // Try with Capnproto for now
    let parser = IexParser::new(&buf[..]);
    let capnp_buf = capnp_runner::serialize_capnp(parser, buf.len(), true);
    let stats = capnp_runner::read_capnp(&capnp_buf, true);

    dbg!(stats);

    println!(
        "Parse time seconds={}",
        SystemTime::now().duration_since(start).unwrap().as_secs()
    )
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

impl SummaryStats {
    fn new(sym: &str) -> SummaryStats {
        SummaryStats {
            symbol: sym.to_string(),
            trade_volume: 0,
            bid_high: 0,
            bid_low: u64::max_value(),
            ask_high: 0,
            ask_low: u64::max_value(),
        }
    }
}
