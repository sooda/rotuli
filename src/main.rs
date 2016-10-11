extern crate glob;
extern crate yaml_rust;

use glob::glob;
use std::env;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use yaml_rust::yaml::{Yaml, Hash, YamlLoader};
use std::cell::RefCell;
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

#[derive(Debug, PartialEq)]
struct PageReference(usize);

//#[derive(Debug)]
struct Group {
    name: String,
    pages: Vec<PageReference>,
}

impl Group {
    fn new(name: &str, pages: Vec<PageReference>) -> Self {
        Group { name: name.to_owned(), pages: pages }
    }
}

impl<'a> fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Group {{ name: {}, pages: {:?} }}", self.name,
               self.pages)
    }
}

#[derive(Debug, PartialEq)]
struct GroupReference(usize);

#[derive(Debug)]
struct Page {
    metadata: Metadata,
    content: String,
    path: PathBuf,
    url: String,
    // filled after initial page construction
    groups: Vec<GroupReference>,
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
    fn from_disk(path: &PathBuf) -> Self {
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
            groups: vec![],
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

fn pages_by_key(pages: &Vec<Page>, name: &str) -> Vec<PageReference> {
    pages.iter().enumerate().filter(|&(_, p)| p.metadata.contains_key(name)).map(|(i, _)| PageReference(i)).collect()
}

struct Site {
    pages: Vec<Page>,
    groups: Vec<Group>,
}

impl Site {
    fn new(dir: &str) -> Self {
        // Load source data as pages with just metadata properly initialized
        let srcpaths = discover_source(dir);
        let pages: Vec<_> = srcpaths.iter().map(Page::from_disk).collect();

        // debug notes
        for p in &pages {
            let b = &p.metadata.get("blog").unwrap_or(&Yaml::Boolean(false)).as_bool().unwrap();
            println!("{:?}", p);
            println!("blog {:?} {:?}", p.metadata.get("blog"), b);
            println!("bloggity {:?}", p.metadata.get("bloggity"));
        }

        // Move pages to site, construct groups
        let mut site = Site { pages: pages, groups: vec![] };
        let group_names = BTreeSet::from_iter(site.pages.iter().map(|p| p.metadata.keys())
            .fold(vec![], |mut tot, key| { tot.extend(key); tot }));
        site.groups = group_names.iter().map(
            |key| Group::new(key, pages_by_key(&site.pages, key))).collect();

        // Transpose groups (lists of pages by key) into their links (lists of groups by pages
        // (i.e., lists of keys))
        let mut pagegroups: Vec<Vec<GroupReference>> = vec![];
        for p in 0..site.pages.len() {
            pagegroups.push(site.groups_for(PageReference(p)));
        }
        for (p, gs) in site.pages.iter_mut().zip(pagegroups.into_iter()) {
            p.groups = gs;
        }

        site
    }

    fn groups_for(&self, page: PageReference) -> Vec<GroupReference> {
        self.pages[page.0].metadata.keys().iter().map(|k| self.get_groupi(k)).collect()
        //self.groups.iter().enumerate().filter(|&(i, g)| g.pages.contains(&page)).map(|(i, g)| GroupReference(i)).collect()
    }

    fn get_group(&self, name: &str) -> &Group {
        self.groups.iter().find(|x| x.name == name).unwrap()
    }
    fn get_groupi(&self, name: &str) -> GroupReference {
        GroupReference(self.groups.iter().enumerate().find(|&(_, x)| x.name == name).unwrap().0)
    }
}

fn main() {
    let dir = &env::args().nth(1).unwrap();
    let site = Site::new(dir);

    for g in site.groups.iter().enumerate() {
        println!("{:?}", g);
    }

    for p in site.pages.iter().enumerate() {
        println!("{:?}", p);
    }
}
