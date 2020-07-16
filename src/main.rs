extern crate glob;
extern crate yaml_rust;
extern crate tera;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use glob::glob;

use tera::{Tera, Context};

use std::env;
use std::path::{Path, PathBuf};
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use yaml_rust::yaml::{Yaml, Hash, YamlLoader};
use std::collections::BTreeSet;
use std::iter::FromIterator;
use std::fmt;
use std::process::{Command, Stdio};

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

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
struct GroupReference(usize);

#[derive(Debug)]
struct Page {
    path: PathBuf,
    url: PathBuf,
    metadata: Metadata,
    content: String,
    // filled after initial page construction
    groups: Vec<GroupReference>,
    translations: Vec<PageReference>
}

fn read_file(p: &Path) -> String {
    let mut s = String::new();
    let mut f = File::open(p).unwrap();
    f.read_to_string(&mut s).unwrap();

    s
}

fn write_file(p: &Path, s: &str) {
    create_dir_all(p.parent().unwrap()).unwrap();
    let mut f = File::create(p).unwrap();
    f.write_all(s.as_bytes()).unwrap();
}

fn make_url(path: &Path, root: &Path) -> PathBuf {
    let formatted = if path.ends_with("index.rst") {
        // just strip the index part off
        path.parent().unwrap().to_path_buf()
    } else {
        // strip markup extension, append back to the directory
        path.parent().unwrap().join(path.file_stem().unwrap())
    };
    let child = formatted.strip_prefix(root).unwrap();

    Path::new("/").join(child)
}

impl Page {
    // all have a parent directory and a file, from discover_source constraints
    fn from_disk(path: &Path, root: &Path) -> Self {
        let data = read_file(path);
        let split_pos = data.find("\n\n").expect(&format!(
                "Missing metadata separator in {}", path.to_string_lossy()));
        let metadata = Metadata::from_string(&data[..split_pos]);
        let content = &data[split_pos..];
        let url = make_url(path, root);

        Page {
            metadata: metadata,
            content: content.to_string(),
            path: path.to_path_buf(),
            url: url,
            groups: vec![],
            translations: vec![]
        }
    }

    fn original_url(&self) -> Option<PathBuf> {
        self.metadata.get("original").map(|meta| {
            let orig = Path::new(meta.as_str().unwrap());
            if orig.starts_with("/") {
                orig.to_path_buf()
            } else {
                self.url.parent().unwrap().join(orig)
            }
        })
    }
    fn language(&self) -> Option<&str> {
        self.metadata.get("language").map(|meta| meta.as_str().unwrap())
    }

    fn display_url(&self) -> String {
        let as_is = &self.metadata.get(MAGIC_META_URL_AS_IS)
            .unwrap_or(&Yaml::Boolean(false)).as_bool().unwrap();

        if self.path.ends_with("index.rst") || !as_is {
            self.url.to_str().unwrap().to_owned() + "/"
        } else {
            self.url.to_str().unwrap().to_owned()
        }
    }

    fn url_file(&self) -> PathBuf {
        let as_is = &self.metadata.get(MAGIC_META_URL_AS_IS)
            .unwrap_or(&Yaml::Boolean(false)).as_bool().unwrap();

        if *as_is {
            self.url.strip_prefix("/").unwrap().to_path_buf()
        } else {
            self.url.strip_prefix("/").unwrap().join("index.html")
        }
    }

    fn template_name(&self) -> &str {
        self.metadata.get("template").unwrap().as_str().unwrap()
    }

    fn output(&self, path: &PathBuf) {
        /*
        let filename = if self.url.chars().nth(0).unwrap() == '/' {
            self.url.clone() + "index.html" } else { self.url.clone() };
        */
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

struct TemplateContext {
}

struct Site {
    directory: PathBuf,
    pages: Vec<Page>,
    groups: Vec<Group>,
}

impl Site {
    fn new(dir: &str) -> Self {
        // Load source data as pages with just metadata properly initialized
        let srcpaths = discover_source(dir);
        let dir = Path::new(dir);
        let pages: Vec<_> = srcpaths.iter().map(|x| Page::from_disk(x, dir)).collect();

        // debug notes
        /*
        for p in &pages {
            let b = &p.metadata.get("blog").unwrap_or(&Yaml::Boolean(false)).as_bool().unwrap();
            println!("{:?}", p);
            println!("blog {:?} {:?}", p.metadata.get("blog"), b);
            println!("bloggity {:?}", p.metadata.get("bloggity"));
            println!("path {:?}", p.path);
            println!("url {:?} {:?}", p.url, p.display_url());
            println!("language {:?}", p.language());
        }
        */

        // Move pages to site, construct groups
        let mut site = Site { directory: dir.to_path_buf(), pages: pages, groups: vec![] };
        let group_names = BTreeSet::from_iter(site.pages.iter().map(|p| p.metadata.keys())
            .fold(vec![], |mut tot, key| { tot.extend(key); tot }));
        site.groups = group_names.iter().map(
            |key| Group::new(key, pages_by_key(&site.pages, key))).collect();

        // Transpose groups (lists of pages by key) into their links (lists of groups by pages
        // (i.e., lists of keys))
        let pagegroups = (0..site.pages.len()).map(
            |pi| site.groups_for(PageReference(pi))).collect::<Vec<_>>();

        for (p, gs) in site.pages.iter_mut().zip(pagegroups.into_iter()) {
            p.groups = gs;
        }

        // build translation cross-references for pages that link to originals
        let mut translations: Vec<Vec<PageReference>> = vec![vec![]; site.pages.len()];
        {
            let orig_id_by_page = |p: &Page| {
                p.original_url().map(|url| site.page_by_url(url.to_str().unwrap()))
            };

            // first, let the sources (original pages) know there's pages at the other end of the
            // edges of this undirected graph
            let mut from_origs: Vec<Vec<PageReference>> = vec![vec![]; site.pages.len()];
            for (i, p) in site.pages.iter().enumerate() {
                let orig_id = orig_id_by_page(p);
                match orig_id {
                    Some(id) => from_origs[id.0].push(PageReference(i)),
                    None => ()
                };
            }
            // then, flip edges to the other direction by copying the lists; also record edges from
            // origs to origs so the translation lists are the same for every translated page
            for (orig_i, dests) in from_origs.iter_mut().enumerate() {
                if !dests.is_empty() { dests.push(PageReference(orig_i)); }
                for pageref in dests.iter() {
                    // each page has just one original, so the original's link list is complete
                    translations[pageref.0] = dests.clone();
                }
            }
        }
        for (p, ts) in site.pages.iter_mut().zip(translations.into_iter()) {
            p.translations = ts;
        }

        site
    }

    // TODO cleanup & safety:
    // fn enumerate_pages() -> ... { self.pages.iter.enumerate() returning PageReference instead of usize
    // fn enumerate_groups() -> ..

    fn groups_for(&self, page: PageReference) -> Vec<GroupReference> {
        self.pages[page.0].metadata.keys().iter().map(|k| self.get_group(k)).collect()
        //self.groups.iter().enumerate().filter(|&(i, g)| g.pages.contains(&page)).map(|(i, g)| GroupReference(i)).collect()
    }

    fn get_group(&self, name: &str) -> GroupReference {
        GroupReference(self.groups.iter().enumerate().find(|&(_, x)| x.name == name).unwrap().0)
    }
    fn page_by_url(&self, url: &str) -> PageReference {
        PageReference(self.pages.iter().enumerate().find(
                |&(_, x)| x.url.to_str().unwrap() == url).unwrap().0)
    }

    fn render(&self, tera: &Tera, output_dir: &str) {
        #[derive(Serialize)]
        struct PageContext {
            prev: String,
            next: String,
            url: String,
            title: String,
        }

        let output_dir = Path::new(output_dir);
        for (i, p) in self.pages.iter().filter(|p| p.metadata.contains_key("ok")).enumerate() {
            println!("AAAA {:?}", p.metadata.contains_key("ok"));
            println!("AAAA {:?}", p.metadata.get("ok"));
            if false { // p.path.to_string_lossy() != "sample-source/2016/9/14/test-commit.rst" {
            //if p.path.to_string_lossy() != "sample-source/index.rst" {
            //if p.path.to_string_lossy() != "sample-source/eng.rst" {
                println!("not render {:?} to {:?} using {}", p.path.to_string_lossy(), p.url_file(), p.template_name());
                continue;
            }
            println!("render {:?} to {:?} using {}", p.path, p.url_file(), p.template_name());

            let process = Command::new("./rstrender.py")
                .stdin(Stdio::piped()).stdout(Stdio::piped())
                .spawn().unwrap();

            process.stdin.unwrap().write_all(p.content.as_bytes()).unwrap();
            let mut content_rendered = String::new();
            process.stdout.unwrap().read_to_string(&mut content_rendered).unwrap();

            let mut c = Context::new();
            c.add("path", &p.path.to_str().unwrap());
            c.add("content", &content_rendered);
            c.add("prev", &PageContext { prev: "p".to_owned(), next: "n".to_owned(), url: "u".to_owned(), title: "t".to_owned() });
            c.add("next", &PageContext { prev: "p".to_owned(), next: "n".to_owned(), url: "u".to_owned(), title: "t".to_owned() });

            //let outfile = output_dir.join(p.url_file());
            //write_file(&outfile, s);
            //let s = tera.render(&("templates/".to_owned() + p.template_name()), c).unwrap();
            let s = tera.render(p.template_name(), c).unwrap();

            let outfile = output_dir.join(p.url_file());
            write_file(&outfile, &s);
        }
    }
}

fn main() {
    let source = &env::args().nth(1).unwrap();
    let output = &env::args().nth(2).unwrap();
    let site = Site::new(source);
    let tera = Tera::new("sample-templates/**/*.html");

    println!("--- yiss! groups ---");

    for g in site.groups.iter().enumerate() {
        println!("{:?}", g);
    }

    println!("--- yiss! pages ---");

    for p in site.pages.iter().enumerate() {
        println!("{:?}", p);
    }

    println!("--- yiss! render ---");

    site.render(&tera, output);
}
