extern crate glob;
extern crate yaml_rust;

use glob::glob;
use std::env;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use yaml_rust::yaml::{Yaml,Hash,YamlLoader};
use std::rc::Rc;
use std::collections::BTreeSet;
use std::iter::FromIterator;
use std::fmt;

// Metadata keys treated in a special way; could use strings in-place, but now they're in a single
// place here for explicitness.
const MAGIC_META_TEMPLATE: &'static str = "template";
const MAGIC_META_ORIGINAL: &'static str = "original";
const MAGIC_META_URL_AS_IS: &'static str = "url_as_is";

#[derive(Debug)]
struct Metadata {
    data: Hash
}

type MetadataValue = Yaml;

impl Metadata {
    // note: would be nice to have the as_bool() etc here too
    fn get(&self, key: &str) -> Option<&MetadataValue> {
        self.data.get(&Yaml::String(key.to_string()))
    }

    fn keys(&self) -> Vec<String> {
        self.data.keys().map(|x| x.as_str().unwrap().to_owned()).collect()//.cloned().collect()
    }

    fn contains_key(&self, name: &str) -> bool {
        self.data.contains_key(&MetadataValue::from_str(name))
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

//#[derive(Debug)]
struct Group<'a> {
    name: String,
    pages: Vec<&'a Page>,
}

impl<'a> Group<'a> {
    fn new(name: &str, pages: Vec<&'a Page>) -> Group<'a> {
        Group { name: name.to_owned(), pages: pages }
    }
}

impl<'a> fmt::Debug for Group<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Group {{ name: {}, pages: {:?} }}", self.name,
               self.pages.iter().map(|x| &x.path).collect::<Vec<_>>())
    }
}

#[derive(Debug)]
struct Page {
    metadata: Metadata,
    content: String,
    path: PathBuf,
    url: String,
    //groups: Vec<Rc<Group>>,
}

fn read_file(p: &Path) -> String {
    let mut s = String::new();
    let mut f = File::open(p).unwrap();
    f.read_to_string(&mut s).unwrap();

    s
}

// a/b/index.rst -> a/b/
// a/b/c.d.rst -> a/b/c.d/ or a/b/c.d
fn make_url(path: &Path, as_is: bool) -> String {
    if path.ends_with("index.rst") {
        path.parent().unwrap().to_str().unwrap().to_string() + "/"
    } else {
        // strip extension, add / unless as_is
        path.parent().unwrap().join(path.file_stem().unwrap()).
            to_str().unwrap().to_string() +
            if as_is { "" } else { "/" }
    }
    /*
    if path.ends_with("/index.rst") {
        path[..path.len() - "index.rst".len()]
    } else if as_is {
        path[..path.len() - ".rst".len()]
    } else {
        path[..path.len() - ".rst".len()] + "/"
    }
    */
}

impl Page {
    // all have a parent directory and a file, from discover_source constraints
    fn from_disk(path: &PathBuf) -> Page {
        let data = read_file(path);
        let split_pos = data.find("\n\n").expect(&format!(
                "Missing metadata separator in {}", path.to_string_lossy()));
        let metadata = Metadata::from_string(&data[..split_pos]);
        let content = &data[split_pos..];
        // ughh
        let as_is = &metadata.get(MAGIC_META_URL_AS_IS).unwrap_or(&Yaml::Boolean(false)).as_bool().unwrap();
        let url = make_url(path, *as_is);

        Page {
            metadata: metadata,
            content: content.to_string(),
            path: path.clone(),
            url: url,
            //groups: vec![],
        }
    }
    fn output(&self, path: &PathBuf) {
        let filename = if self.url.chars().nth(0).unwrap() == '/' {
            self.url.clone() + "index.html" } else { self.url.clone() };
    }
}

fn discover_source(pathname: &str) -> Vec<PathBuf> {
    let paths = glob(&*(pathname.to_string() + "/**/*.rst")).unwrap();
    // silently ignore Err items, unreadable files are skipped on purpose
    let pathbufs: Vec<_> = paths.filter_map(|x| x.ok()).collect();

    pathbufs
}

fn pages_by_key<'a>(pages: &'a Vec<Page>, name: &str) -> Vec<&'a Page> {
    pages.iter().filter(|p| p.metadata.contains_key(name)).collect()
}


fn main() {
    let dir = &env::args().nth(1).unwrap();
    let srcpaths = discover_source(dir);
    let pages: Vec<_> = srcpaths.iter().map(Page::from_disk).collect();
    for p in &pages {
        let b = &p.metadata.get("blog").unwrap_or(&Yaml::Boolean(false)).as_bool().unwrap();
        println!("{:?}", p);
        println!("blog {:?} {:?}", p.metadata.get("blog"), b);
        println!("bloggity {:?}", p.metadata.get("bloggity"));
    }
    let group_names = BTreeSet::from_iter(pages.iter().map(|p| p.metadata.keys())
        .fold(vec![], |mut tot, i| { tot.extend(i); tot }));
    let groups: Vec<_> = group_names.iter().map(|key| Group::new(key, pages_by_key(&pages, key))).collect();
    println!("{:?}", groups);
}
