extern crate glob;

use std::env;
use glob::glob;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;


fn discover_source(pathname: &str) -> Vec<PathBuf> {
    let paths = glob(&*(pathname.to_string() + "/**/*.rst")).unwrap();
    // silently ignore Err items, unreadable items are skipped on purpose
    let pathbufs: Vec<_> = paths.filter_map(|x| x.ok()).collect();

    pathbufs
}

fn read_file(p: &Path) -> String {
    let mut s = String::new();
    let mut f = File::open(p).unwrap();
    f.read_to_string(&mut s).unwrap();

    s
}

fn main() {
    let srcpaths = discover_source(&env::args().nth(1).unwrap());
    println!("{:?}", srcpaths);
    println!("{:?}", read_file(&srcpaths[0]));
}
