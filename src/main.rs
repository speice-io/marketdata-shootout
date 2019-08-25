use std::fs::File;
use std::io::Read;
use std::path::Path;

use clap::{App, Arg};
use nom::sequence::tuple;

use parsers::Block;

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

    let mut rem = &buf[..];
    while let Ok((unparsed, block)) = parsers::read_block(rem) {
        let offset = (unparsed.as_ptr() as usize) - (buf.as_ptr() as usize);
        rem = unparsed;
        match block {
            Block::SectionHeader(sh) => println!("{:?}, next offset={}", sh, offset),
            Block::InterfaceDescription(id) => println!("{:?}, next offset={}", id, offset),
            Block::EnhancedPacket(epb) => println!("EnhancedPacketBlock {{ block_len: {}, packet_len: {} }}, next offset={}", epb.block_len, epb.packet_data.len(), offset)
        }
    }

    println!("Remaining unparsed len={}", rem.len());
}
