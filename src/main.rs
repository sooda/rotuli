extern crate glob;
extern crate tera;
extern crate serde;
extern crate serde_yaml;
extern crate rst_parser;
extern crate rst_renderer;
extern crate document_tree;

use glob::glob;

use tera::{Tera, Context};

use std::env;
use std::path::{Path, PathBuf};
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::collections::{BTreeSet, BTreeMap, HashMap};
use std::iter::FromIterator;
use std::fmt;
use serde::Serialize;
use document_tree::element_categories::HasChildren;

// Metadata keys treated in a special way; could use strings in-place, but now they're in a single
// place here for explicitness.
const MAGIC_META_TEMPLATE: &'static str = "template";
const MAGIC_META_ORIGINAL: &'static str = "original";
const MAGIC_META_URL_AS_IS: &'static str = "url_as_is";

#[derive(Debug)]
struct Metadata {
    data: serde_yaml::Mapping
}

type MetadataValue = serde_yaml::Value;

impl Metadata {
    // note: would be nice to have the as_bool() etc here too
    fn get(&self, key: &str) -> Option<&MetadataValue> {
        self.data.get(&serde_yaml::to_value(key).unwrap())
    }

    fn keys(&self) -> Vec<String> {
        self.data.iter().map(|(k, _)| k.as_str().unwrap().to_owned()).collect()
    }

    fn contains_key(&self, name: &str) -> bool {
        self.data.contains_key(&serde_yaml::to_value(name).unwrap())
    }

    fn from_string(s: &str) -> Metadata {
        let value = serde_yaml::from_str(s).unwrap();
        Metadata { data: value }
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
    title: String,
    metadata: Metadata,
    content: String,
    content_rendered: String,
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
        let content = &data[split_pos + 2..];
        let url = make_url(path, root);
        let (rendered, title) = rstrender(content);

        Page {
            path: path.to_path_buf(),
            url: url,
            title: title,
            metadata: metadata,
            content: content.to_string(),
            content_rendered: rendered,
            groups: vec![],
            translations: vec![]
        }
    }

    // XXX: will be replaced with general groups
    fn original_url(&self) -> Option<PathBuf> {
        self.metadata.get(MAGIC_META_ORIGINAL).map(|meta| {
            let orig = Path::new(meta.as_str().unwrap());
            if orig.starts_with("/") {
                orig.to_path_buf()
            } else {
                // :( FIXME: can't get this functionality just via raw metadata, but perhaps via
                // custom types (yaml tags)
                self.url.parent().unwrap().join(orig)
            }
        })
    }

    fn _display_url(&self) -> String {
        let as_is = &self.metadata.get(MAGIC_META_URL_AS_IS)
            .map(|x| x.as_bool().unwrap()).unwrap_or(false);

        if self.path.ends_with("index.rst") || !as_is {
            self.url.to_str().unwrap().to_owned() + "/"
        } else {
            self.url.to_str().unwrap().to_owned()
        }
    }

    fn url_file(&self) -> PathBuf {
        let as_is = &self.metadata.get(MAGIC_META_URL_AS_IS)
            .map(|x| x.as_bool().unwrap()).unwrap_or(false);

        if *as_is {
            self.url.strip_prefix("/").unwrap().to_path_buf()
        } else {
            self.url.strip_prefix("/").unwrap().join("index.html")
        }
    }

    fn template_name(&self) -> &str {
        self.metadata.get(MAGIC_META_TEMPLATE).unwrap().as_str().unwrap()
    }

    fn title(&self) -> &str {
        // FIXME: read the rst title and default to it
        self.metadata.get("title").map(|x| x.as_str().unwrap()).unwrap_or(&self.title)
    }
}

fn discover_source(pathname: &str) -> Vec<PathBuf> {
    let paths = glob(&*(pathname.to_string() + "/**/*.rst")).unwrap();
    // silently ignore Err items, unreadable files are skipped on purpose
    // (FIXME: verbose mode to print them)
    let pathbufs: Vec<_> = paths.filter_map(|x| x.ok()).collect();

    pathbufs
}

fn pages_by_key(pages: &Vec<Page>, name: &str) -> Vec<PageReference> {
    pages.iter().enumerate()
        .filter(|&(_, p)| p.metadata.contains_key(name))
        .map(|(i, _)| PageReference(i))
        .collect()
}

struct Site {
    _directory: PathBuf,
    pages: Vec<Page>,
    groups: Vec<Group>,
}

impl Site {
    fn new(dir: &str) -> Self {
        // Load source data as pages with just metadata properly initialized
        let srcpaths = discover_source(dir);
        let dir = Path::new(dir);
        let page_ok = |p: &Page| p.metadata.get("ok")
            .map(|x| x.as_bool().unwrap())
            .unwrap_or(false);
        let pages: Vec<_> = srcpaths.iter()
            .map(|path| Page::from_disk(path, dir)).filter(page_ok).collect();

        // debug notes
        /*
        for p in &pages {
            println!("{:?}", p);
            println!("path {:?}", p.path);
            println!("url {:?} {:?}", p.url, p.display_url());
            println!("language {:?}", p.language());
        }
        */

        // Move pages to site, construct groups
        let mut site = Site { _directory: dir.to_path_buf(), pages: pages, groups: vec![] };
        let group_names = BTreeSet::from_iter(
            site.pages.iter()
            .map(|p| p.metadata.keys())
            .fold(vec![], |mut tot, key| { tot.extend(key); tot })
        );
        site.groups = group_names.iter().map(
            |key| Group::new(key, pages_by_key(&site.pages, key))).collect();

        // Transpose groups from (list of pages by group ref) into (list of groups by page ref)
        let pagegroups = (0..site.pages.len()).map(
            |pi| site.groups_for(PageReference(pi))).collect::<Vec<_>>();

        for (p, gs) in site.pages.iter_mut().zip(pagegroups.into_iter()) {
            p.groups = gs;
        }

        // build translation cross-references for pages that link to originals
        // XXX: this has to become translation-agnostic and more general, so that pages are
        // detected in any metadata group and crossrefs built on all of them (like categories)
        // (perhaps use a custom datatype for this, or hack it up with special hashes for now)
        // https://github.com/chyh1990/yaml-rust/issues/35
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
                    None => () // pop up a verbose message?
                };
            }
            // then, flip edges to the other direction by copying the lists; also record cycle edges from
            // origs to origs so the translation lists are the same and complete for every translated page
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
        #[derive(Debug, Serialize)]
        struct PageContext<'a> {
            path: &'a str,
            url: &'a str,
            title: &'a str,
            meta: &'a serde_yaml::Mapping,
        }

        #[derive(Debug, Serialize)]
        struct GroupContext<'a> {
            name: &'a str,
            pages: Vec<&'a PageContext<'a>>,
        }

        #[derive(Debug, Serialize)]
        struct SiteContext<'a> {
            pages: &'a Vec<PageContext<'a>>,
            pages_by_url: BTreeMap<&'a str, &'a PageContext<'a>>,
            groups: BTreeMap<&'a str, GroupContext<'a>>,
        }

        #[derive(Debug, Serialize, Clone)]
        struct RefContext<'a> {
            // this should be a map instead, but just one metadata key is enough for prototyping
            original: Vec<&'a PageContext<'a>>,
        }

        // XXX: this is here for now to remind about a possible additional post-load draft flag
        let page_ok = |p: &&Page| p.metadata.get("ok")
            .map(|x| x.as_bool().unwrap())
            .unwrap_or(false);
        // closure because cloning one filter isn't ergonomic
        let ok_pages = || self.pages.iter().filter(page_ok);

        let pages_cx = ok_pages()
            .map(|p| PageContext {
                path: p.path.to_str().unwrap(),
                url: p.url.to_str().unwrap(),
                title: p.title(),
                meta: &p.metadata.data,
            }).collect::<Vec<_>>();

        let pages_by_url_cx = pages_cx.iter().map(|p| (&p.url as &str, p)).collect();

        let groups_cx = self.groups.iter().map(|g| (&g.name as &str, GroupContext {
            name: &g.name,
            pages: g.pages.iter().map(|pageref| &pages_cx[pageref.0]).collect(),
        })).collect();

        let site_cx = SiteContext {
            pages: &pages_cx,
            pages_by_url: pages_by_url_cx,
            groups: groups_cx,
        };

        let mut refs_cxs = vec![RefContext { original: vec![] }; pages_cx.len()];
        for (j, p) in ok_pages().enumerate() {
            if let Some(ref_target) = p.metadata.get("original").map(|x| x.as_str().unwrap()) {
                refs_cxs[self.page_by_url(ref_target).0].original.push(&pages_cx[j]);
            }
            // loop p's metadata keys/values, find somehow typed page refs in values...
        }
        let refs_cxs = refs_cxs;

        let output_dir = Path::new(output_dir);
        for (i, p) in ok_pages().enumerate() {
            println!("render {:?} to {:?} using {}", p.path, p.url_file(), p.template_name());

            let mut cx = Context::new();

            let page_cx = &site_cx.pages[i];
            let refs_cx = &refs_cxs[i];

            cx.insert("site", &site_cx);
            cx.insert("page", &page_cx);
            cx.insert("content", &p.content_rendered);
            cx.insert("meta", &page_cx.meta);
            cx.insert("refs", &refs_cx);

            let s = tera.render(p.template_name(), &cx).unwrap();

            let outfile = output_dir.join(p.url_file());
            write_file(&outfile, &s);
        }
    }
}

fn document_title(document: &document_tree::Document) -> String {
    use document_tree::{
        element_categories as ec
    };
    /*
     * Section {
     *   common: CommonAttributes { ids: [ID("sample-website")], names: [], source: None, classes: [] },
     *   children: [
     *     Title {
     *       common: CommonAttributes {
     *         ids: [], names: [NameToken("sample-website")], source: None, classes: []
     *       },
     *       children: ["Sample website"]
     *     },
     *     Paragraph {
     *       common: CommonAttributes { ids: [], names: [], source: None, classes: [] },
     *       children: ["Hello world from rotuli."] },
     */
    // each document should be a section only for consistency; without a top level title we'd get
    // just a bunch of paragraphs
    assert!(document.children().len() <= 1, "don't know what to do with this complex document (did you misformat or omit the title?)");

    let section_element: &ec::StructuralSubElement = &document.children().first()
        .expect("an empty document, you made a mistake");
    // the inner elements are boxed, hence an extra deref
    let section_substructure: &ec::SubStructure = &**match section_element {
        ec::StructuralSubElement::SubStructure(x) => x,
        // title, subtitle, decoration, ...
        _ => panic!("strange section subelement")
    };
    let section_obj: &document_tree::Section = &**match section_substructure {
        ec::SubStructure::Section(x) => x,
        // topic, sidebar, transition, ...
        _ => panic!("strange section substructure, do you have just one paragraph there?")
    };

    let first_element = &section_obj.children().first()
        .expect("how did you make a section with no content?");

    let titobj: &document_tree::Title = &**match first_element {
        ec::StructuralSubElement::Title(x) => x,
        // substructure, subtitle, decoration, ...
        _ => panic!("only titles in the document front please")
    };
    assert!(titobj.children().len() <= 1, "the title is too complicated, please use just plain text");
    let inner_textobj: &ec::TextOrInlineElement = &titobj.children().first()
        .expect("how did you make a title with no content?");

    let title_text: &String = &**match inner_textobj {
        ec::TextOrInlineElement::String(x) => x,
        _ => panic!("do not format (emphasize etc.) the titles"),
    };

    title_text.to_string()
}

fn rstrender(s: &str) -> (String, String) {
    if let Ok(document) = rst_parser::parse(s) {
        let mut rendered_bytes = Vec::new();
        let _rend_res = rst_renderer::render_html(&document, &mut rendered_bytes, false);
        let rendered = String::from_utf8(rendered_bytes).unwrap();

        let title = document_title(&document);

        (rendered, title)
    } else {
        // TODO: error handling and verbosity
        ("".to_string(), "".to_string())
    }
}

// Array([Array([String("x")]), Array([String("y")]), Array([String("z"), String("w")])
// into Array([String("x"), String("y"), String("z"), String("w")])
fn flatten_array(value: &tera::Value, _args: &HashMap<String, tera::Value>)
-> tera::Result<tera::Value> {
    if let tera::Value::Array(items) = value {
        let flattened: Vec<tera::Value> = items.into_iter()
            .flat_map(|inner_array_val: &tera::Value| {
                inner_array_val.as_array().expect(
                    "trying to flatten something that contains other than just arrays"
                    ).clone().into_iter()
            }).collect();
        Ok(tera::Value::Array(flattened))
    } else {
        Err(tera::Error::msg("trying to flatten something that's not an array"))
    }
}

fn get_json_pointer(key: &str) -> String {
    ["/", &key.replace(".", "/")].join("")
}

fn before_attr(value: &tera::Value, args: &HashMap<String, tera::Value>)
-> tera::Result<tera::Value> {
    let arr = tera::try_get_value!("before_attr", "value", Vec<tera::Value>, value);
    if arr.is_empty() {
        return Ok(tera::Value::Null);
    }

    let key = match args.get("attribute") {
        Some(val) => tera::try_get_value!("before_attr", "attribute", String, val),
        None => return Err(tera::Error::msg("The `before_attr` filter has to have an `attribute` argument")),
    };

    let val_lookup = match args.get("value") {
        Some(val) => val,
        None => return Err(tera::Error::msg("The `before_attr` filter has to have an `value` argument")),
    };

    let json_pointer = get_json_pointer(&key);

    let value = arr.iter().zip(arr.iter().skip(1))
        .find(|&(_, current)| current.pointer(&json_pointer).map_or(false, |value| value == val_lookup))
        .map(|(preceding, _)| preceding.clone()).unwrap_or(tera::Value::Null);

    // An unmatched attr or a match at the first one (with no previous) is never an error; null is
    // returned in those cases.
    Ok(value)
}

fn after_attr(value: &tera::Value,
               args: &std::collections::HashMap<String, tera::Value>)
-> tera::Result<tera::Value> {
    let arr = tera::try_get_value!("after_attr", "value", Vec<tera::Value>, value);
    if arr.is_empty() {
        return Ok(tera::Value::Null);
    }

    let key = match args.get("attribute") {
        Some(val) => tera::try_get_value!("after_attr", "attribute", String, val),
        None => return Err(tera::Error::msg("The `after_attr` filter has to have an `attribute` argument")),
    };

    let val_lookup = match args.get("value") {
        Some(val) => val,
        None => return Err(tera::Error::msg("The `after_attr` filter has to have an `value` argument")),
    };

    let json_pointer = get_json_pointer(&key);

    let value = arr.iter().zip(arr.iter().skip(1))
        .find(|&(current, _)| current.pointer(&json_pointer).map_or(false, |value| value == val_lookup))
        .map(|(_, following)| following.clone()).unwrap_or(tera::Value::Null);

    // An unmatched attr or a match at the last one (with no next) is never an error; null is
    // returned in those cases.
    Ok(value)
}

fn main() {
    let source = &env::args().nth(1).unwrap();
    let output = &env::args().nth(2).unwrap();
    let site = Site::new(source);

    let tera = Tera::new("sample-templates/**/*.html");
    let mut tera = match tera {
        Ok(t) => t,
        Err(e) => {
            if let tera::ErrorKind::Msg(s) = e.kind {
                println!("rotuli: templates failed: {}", s);
            } else {
                println!("rotuli: templates failed: {:?}", e);
            }
            return;
        }
    };
    tera.register_filter("flatten_array", flatten_array);
    tera.register_filter("before_attr", before_attr);
    tera.register_filter("after_attr", after_attr);

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
