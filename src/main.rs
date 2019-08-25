use std::fs::File;
use std::io::Read;
use std::path::Path;

use clap::{App, Arg};

use crate::iex::IexParser;

// Cap'n'Proto and Flatbuffers typically ask that you generate code on the fly to match
// the schemas. For purposes of auto-complete and easy browsing in the repository,
// we generate the code and just copy it into the src/ tree.
pub mod marketdata_capnp;
#[allow(unused_imports)]
pub mod marketdata_generated; // Flatbuffers

mod iex;
mod parsers;

fn main() {
    let matches = App::new("Marketdata Shootout")
        .arg(Arg::with_name("file")
            .short("f")
            .long("file")
            .value_name("FILE")
            .help("IEX DEEP file to process")
            .required(true)
            .takes_value(true))
        .get_matches();

    let deep = matches.value_of("file").unwrap();
    let path = Path::new(deep);
    let mut file = File::open(path).expect(&format!("Unable to open file={}", path.display()));

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).expect(&format!("Unable to read file={}", path.display()));

    for _payload in IexParser::new(&buf[..]) {
        //dbg!(payload);
    }
}
