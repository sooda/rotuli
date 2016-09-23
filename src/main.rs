extern crate glob;
extern crate yaml_rust;

use glob::glob;
use std::env;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use yaml_rust::yaml::{Yaml,Hash,YamlLoader};

#[derive(Debug)]
struct Metadata {
    data: Hash
}

type MetadataValue = Yaml;

impl Metadata {
    fn get(&self, key: &str) -> Option<&MetadataValue> {
        self.data.get(&Yaml::String(key.to_string()))
    }

    fn from_string(s: &str) -> Metadata {
        let mut loadvec = YamlLoader::load_from_str(s).expect("failed to load metadata");
        let data: Yaml = loadvec.pop().expect("Empty metadata, deal with this separately?");

        let hash = match data {
            Yaml::Hash(h) => h,
            _ => panic!("Unexpected top-level metadata type")
        };
        Metadata { data: hash }
    }
}

#[derive(Debug)]
struct Page {
    metadata: Metadata,
    content: String,
    path: PathBuf,
}

fn read_file(p: &Path) -> String {
    let mut s = String::new();
    let mut f = File::open(p).unwrap();
    f.read_to_string(&mut s).unwrap();

    s
}

impl Page {
    fn from_disk(path: &PathBuf) -> Page {
        let data = read_file(path);
        let split_pos = data.find("\n\n").expect(&format!(
                "Missing metadata separator in {}", path.to_string_lossy()));
        let metadata = Metadata::from_string(&data[..split_pos]);
        let content = &data[split_pos..];

        Page {
            metadata: metadata,
            content: content.to_string(),
            path: path.clone(),
        }
    }
}

fn discover_source(pathname: &str) -> Vec<PathBuf> {
    let paths = glob(&*(pathname.to_string() + "/**/*.rst")).unwrap();
    // silently ignore Err items, unreadable items are skipped on purpose
    let pathbufs: Vec<_> = paths.filter_map(|x| x.ok()).collect();

    pathbufs
}

fn main() {
    let dir = &env::args().nth(1).unwrap();
    let srcpaths = discover_source(dir);
    let pages: Vec<_> = srcpaths.iter().map(Page::from_disk).collect();
    for p in pages {
        println!("{:?}", p);
        println!("blog {:?}", p.metadata.get("blog"));
        println!("bloggity {:?}", p.metadata.get("bloggity"));
    }
}
