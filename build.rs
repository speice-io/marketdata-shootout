extern crate capnpc;

use std::path::Path;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("")
        .file("marketdata.capnp")
        .output_path("src/")
        .run().expect("Unable to compile capnpc");

    flatc_rust::run(flatc_rust::Args {
        inputs: &[Path::new("marketdata.fbs")],
        out_dir: Path::new("src/"),
        ..Default::default()
    }).expect("Unable to compile flatc");
}