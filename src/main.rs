// Cap'n'Proto and Flatbuffers typically ask that you generate code on the fly to match
// the schemas. For purposes of auto-complete and easy browsing in the repository,
// we generate the code and just copy it into the src/ tree.
pub mod marketdata_capnp;
pub mod marketdata_generated; // Flatbuffers

pub mod marketdata_custom;

fn main() {
    println!("Hello, world!");
}
