extern crate zmodem;

extern crate log;
extern crate env_logger;
extern crate clap;

mod read_write;

use std::fs::File;
use std::path::Path;
use clap::{Arg, App};

#[tokio::main]
async fn main() {
    env_logger::init().unwrap();

    let matches = App::new("Pure Rust implementation of sz utility")
        .arg(Arg::with_name("file")
             .required(true)
             .index(1))
        .get_matches();

    let file_opt = matches.value_of("file").unwrap();
    let mut file = File::open(file_opt).unwrap();

    let filename = Path::new(file_opt).file_name().unwrap().clone();
    let size = file.metadata().map(|x| x.len() as u32).ok();

    let inout = read_write::AsyncReadWrite::new(tokio::io::stdin, tokio::io::stdout);

    zmodem::send::send(inout, &mut file, filename.to_str().unwrap(), size).await.unwrap();
}
