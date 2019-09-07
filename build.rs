extern crate capnpc;

use std::path::Path;
use std::process::Command;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("")
        .file("marketdata.capnp")
        .output_path("src/")
        .run()
        .expect("Unable to compile capnpc");

    flatc_rust::run(flatc_rust::Args {
        inputs: &[Path::new("marketdata.fbs")],
        out_dir: Path::new("src/"),
        ..Default::default()
    })
    .expect("Unable to compile flatc");

    // There's no Rust-style builder crate for SBE,
    // so we need to run the command by hand.
    // TODO: Automatically download the SBE JAR?
    let _output = Command::new("java")
        .arg("-Dsbe.output.dir=src")
        .arg("-Dsbe.xinclude.aware=true")
        .arg("-Dsbe.target.language=uk.co.real_logic.sbe.generation.rust.Rust")
        .arg("-Dsbe.target.namespace=marketdata_sbe")
        .arg("-jar")
        .arg("sbe-all-1.13.2-all.jar")
        .arg("marketdata.xml")
        .output()
        .expect("Unable to execute SBE compiler");
}
