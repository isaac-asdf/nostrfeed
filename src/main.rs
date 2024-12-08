use std::{io::Read, path::PathBuf};

fn main() {
    println!("Hello, world!");
    let f = std::fs::File::open(PathBuf::from("./npubs.txt"));
}
