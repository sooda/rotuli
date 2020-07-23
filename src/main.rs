use std::path::{Path, PathBuf};
use std::collections::{BTreeSet, BTreeMap, HashMap};
use std::iter::FromIterator;
use std::fmt;

use glob::glob;
use tera::{Tera, Context};
use serde::Serialize;
use document_tree::element_categories::HasChildren;
use structopt::StructOpt;
use std::str::FromStr;

// Metadata keys treated in a special way; could use strings in-place, but now they're in a single
// place here for explicitness.
const MAGIC_META_TEMPLATE: &'static str = "template";
const MAGIC_META_URL_AS_IS: &'static str = "url_as_is";
const MAGIC_META_TITLE: &'static str = "title";

#[derive(Debug)]
struct Metadata {
    data: serde_yaml::Mapping
}

type MetadataValue = serde_yaml::Value;

impl Metadata {
    // note: would be nice to have the as_bool() etc here too
    fn get(&self, key: &str) -> Option<&MetadataValue> {
        self.data.get(&serde_yaml::to_value(key).expect("string serialization failed??"))
    }

    fn get_bool_or_false(&self, key: &str) -> bool {
        self.get(key)
            .map(|x| x.as_bool().expect(&format!("metadata `{}' does not parse as a bool", key)))
            .unwrap_or(false)
    }

    fn keys(&self) -> Vec<String> {
        self.data.iter().map(|(k, _)| k.as_str().expect("only string keys for now please").to_owned()).collect()
    }

    fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(&serde_yaml::to_value(key).expect("string serialization failed??"))
    }

    fn from_string(s: &str) -> Metadata {
        let value = serde_yaml::from_str(s).expect("metadata deserialization failed");
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    summary_rendered: String,
    // filled after initial page construction
    groups: Vec<GroupReference>,
}

// ("src/foo/some-page.markup", "src") -> "/foo/some-page"
// ("src/foo/some-page.xml.markup", "src") -> "/foo/some-page.xml"
fn make_url(path: &Path, root: &Path, index_filename: &Path) -> PathBuf {
    let parent = path.parent().expect("a file must has a parent");
    let formatted = if path.ends_with(index_filename) {
        // just strip the index part off
        parent.to_path_buf()
    } else {
        // strip markup extension, append back to the directory
        parent.join(path.file_stem().expect("this was supposed to be a file"))
    };
    let from_root = formatted.strip_prefix(root).expect("this was built from the root!!");

    // perhaps should return a string because this isn't a real file in the fs anymore, but a path
    // is more easily modified later if necessary
    Path::new("/").join(from_root)
}

impl Page {
    // all have a parent directory and a file, from discover_source constraints
    fn from_disk(path: &Path, root: &Path, index_filename: &Path) -> Self {
        let data = std::fs::read_to_string(path).expect("file vanished after finding it?");
        let split_pos = data.find("\n\n").expect(&format!(
                "Missing metadata separator in {}", path.to_string_lossy()));
        let metadata = Metadata::from_string(&data[..split_pos]);
        let content = &data[split_pos + 2..];
        let url = make_url(path, root, index_filename);
        let render_result = rstrender(content);

        Page {
            path: Path::new("/").join(path.strip_prefix(root).expect("glob betrayed us")),
            url: url,
            title: render_result.title,
            metadata: metadata,
            content: content.to_string(),
            content_rendered: render_result.body,
            summary_rendered: render_result.summary,
            groups: vec![],
        }
    }

    // append a slash for things that look like a directory, because those are rendered into
    // directories for nice things (e.g., avoid linking to the html extension, emphasizing just the
    // document structure)
    fn display_url(&self) -> String {
        let as_is = self.metadata.get_bool_or_false(MAGIC_META_URL_AS_IS);

        let as_string = self.url.to_str().expect("only UTF-8 files please").to_owned();
        // why both though?
        if as_string == "/" || as_is {
            as_string
        } else {
            as_string + "/"
        }
    }

    fn output_path(&self) -> PathBuf {
        let as_is = self.metadata.get_bool_or_false(MAGIC_META_URL_AS_IS);

        let relative = self.url.strip_prefix("/").expect("`/' was added before but now it's gone?");
        if as_is {
            relative.to_path_buf()
        } else {
            relative.join("index.html")
        }
    }

    fn template_name(&self) -> &str {
        self.metadata.get(MAGIC_META_TEMPLATE).expect("must specify template")
            .as_str().expect("template name must be a string")
    }

    fn title(&self) -> &str {
        self.metadata.get(MAGIC_META_TITLE).map(|x| x.as_str().expect("title must be a string"))
            .unwrap_or(&self.title)
    }
}

fn discover_source(source: &Path, ml: MarkupLanguage) -> Vec<PathBuf> {
    let pathname = source.to_str().expect("only UTF-8 directories please").to_owned();
    let paths = glob(&*(pathname + "/**/*." + &ml.to_string())).expect("invalid search pattern");
    // silently ignore Err items, unreadable files are skipped on purpose
    // (FIXME: verbose mode to print them)
    let pathbufs: Vec<_> = paths.filter_map(|x| x.ok()).collect();

    pathbufs
}

fn pages_by_metadata_key(pages: &Vec<Page>, name: &str) -> Vec<PageReference> {
    pages.iter().enumerate()
        .filter(|&(_, p)| p.metadata.contains_key(name))
        .map(|(i, _)| PageReference(i))
        .collect()
}

struct Site {
    directory: PathBuf,
    pages: Vec<Page>,
    groups: Vec<Group>,
}

#[derive(Debug, Copy, Clone)]
enum MarkupLanguage {
    RestructuredText,
}

#[derive(Debug)]
struct MarkupLanguageParseError;

impl fmt::Display for MarkupLanguageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown markup language, please use one of: [rst]")
    }
}

impl FromStr for MarkupLanguage {
    type Err = MarkupLanguageParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rst" => Ok(MarkupLanguage::RestructuredText),
            _ => Err(MarkupLanguageParseError),
        }
    }
}
impl fmt::Display for MarkupLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            MarkupLanguage::RestructuredText => "rst",
        };
        write!(f, "{}", s)
    }
}

impl Site {
    fn new(dir: PathBuf, markup_language: MarkupLanguage, directory_index: &str) -> Self {
        // Load source data as pages with just metadata properly initialized
        let srcpaths = discover_source(&dir, markup_language);
        let page_ok = |p: &Page| p.metadata.get_bool_or_false("ok");

        let index_filename = Path::new(directory_index).with_extension(markup_language.to_string());
        let pages: Vec<_> = srcpaths.iter()
            .map(|path| Page::from_disk(path, &dir, &index_filename))
            .filter(page_ok)
            .collect();

        if pages.iter().map(|p| p.display_url()).collect::<BTreeSet<_>>().len() != pages.len() {
            panic!("duplicates found, do you have foo.rst and foo/index.rst?");
        }

        // Move pages to site, construct groups
        let mut site = Site { directory: dir, pages: pages, groups: vec![] };
        let group_names = BTreeSet::from_iter(
            site.pages.iter()
            .map(|p| p.metadata.keys())
            .fold(vec![], |mut tot, key| { tot.extend(key); tot })
        );
        site.groups = group_names.iter().map(
            |key| Group::new(key, pages_by_metadata_key(&site.pages, key))).collect();

        // Transpose groups from (list of pages by group ref) into (list of groups by page ref)
        let pagegroups = (0..site.pages.len()).map(
            |pi| site.groups_for(PageReference(pi))).collect::<Vec<_>>();

        for (p, gs) in site.pages.iter_mut().zip(pagegroups.into_iter()) {
            p.groups = gs;
        }

        // TODO: custom datatypes for relative urls and stuff if needed to refer to specific pages
        // (wait for tags or hack it up with special hashes for now as a workaround datatype)
        // https://github.com/chyh1990/yaml-rust/issues/35
        // e.g.: other_thing: { url_magic: ../stuff/ }

        site
    }

    fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    // TODO cleanup & safety:
    // fn enumerate_pages() -> ... { self.pages.iter.enumerate() returning PageReference instead of usize
    // fn enumerate_groups() -> ..

    fn groups_for(&self, page: PageReference) -> Vec<GroupReference> {
        self.pages[page.0].metadata.keys().iter()
            .map(|k| self.get_group(k).expect("groups do not match pages"))
            .collect()
        // or the other way:
        // self.groups.iter().enumerate()
        //     .filter(|&(_, g)| g.pages.contains(&page))
        //     .map(|(i, _)| GroupReference(i))
        //     .collect()
        // but the above looks more natural and returns them in the specified order
    }

    fn get_group(&self, name: &str) -> Option<GroupReference> {
        self.groups.iter().enumerate().find(|&(_, g)| g.name == name)
            .map(|(i, _)| GroupReference(i))
    }

    fn render(&self, tera: &Tera, output_dir: &Path) {
        #[derive(Debug, Serialize)]
        struct PageContext<'a> {
            path: &'a str,
            url: String,
            title: &'a str,
            meta: &'a serde_yaml::Mapping,
            summary: &'a str,
        }

        #[derive(Debug, Serialize)]
        struct GroupContext<'a> {
            name: &'a str,
            pages: Vec<&'a PageContext<'a>>,
        }

        #[derive(Debug, Serialize)]
        struct SiteContext<'a> {
            directory: String,
            pages: &'a Vec<PageContext<'a>>,
            pages_by_url: BTreeMap<&'a str, &'a PageContext<'a>>,
            groups: BTreeMap<&'a str, GroupContext<'a>>,
        }

        // XXX: this is here for now to remind about a possible additional post-load draft flag
        let page_ok = |p: &&Page| p.metadata.get_bool_or_false("ok");
        // closure because cloning one filter isn't ergonomic
        let ok_pages = || self.pages.iter().filter(page_ok);

        let pages_cx = ok_pages()
            .map(|p| PageContext {
                path: p.path.to_str().expect("only UTF-8 directories please"),
                url: p.display_url(),
                title: p.title(),
                meta: &p.metadata.data,
                summary: &p.summary_rendered,
            }).collect::<Vec<_>>();

        let pages_by_url_cx = pages_cx.iter().map(|p| (p.url.as_str(), p)).collect();

        let groups_cx = self.groups.iter().map(|g| (&g.name as &str, GroupContext {
            name: &g.name,
            pages: g.pages.iter().map(|pageref| &pages_cx[pageref.0]).collect(),
        })).collect();

        let site_cx = SiteContext {
            directory: self.directory.canonicalize().expect("can't get this far with a bad dir")
                .to_str().expect("only UTF-8 directories please").to_owned(),
            pages: &pages_cx,
            pages_by_url: pages_by_url_cx,
            groups: groups_cx,
        };

        for (i, p) in ok_pages().enumerate() {
            println!("render {:?} to {:?} using {}", p.path, p.output_path(), p.template_name());

            let mut cx = Context::new();

            let page_cx = &site_cx.pages[i];

            // TODO: reserializing site every time might be heavy; consider creating a common
            // context before the loop and extending the per-page context from it. Valgrind claims
            // that it requires much more allocations though, so the question is which is faster,
            // serialization or allocation.
            cx.insert("site", &site_cx);
            cx.insert("page", &page_cx);
            cx.insert("content", &p.content_rendered);

            let tpl_rendered = match tera.render(p.template_name(), &cx) {
                Ok(text) => text,
                Err(e) => {
                    println!("rotuli: render failed: {}", p.path.to_string_lossy());

                    println!("{}", e);
                    let mut e: &dyn std::error::Error = &e;
                    while let Some(x) = e.source() {
                        println!("{}", x);
                        e = x;
                    }

                    // TODO: propagate error
                    panic!()
                }
            };

            let outfile = output_dir.join(p.output_path());
            std::fs::create_dir_all(outfile.parent().expect("tried to write to the root, huh?"))
                .expect("output dir is unwritable");
            std::fs::write(&outfile, &tpl_rendered).expect("output file is unwritable");
        }
    }
}

fn top_level_rst_section(document: &document_tree::Document) -> &document_tree::Section {
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

    section_obj
}

fn document_title(document: &document_tree::Document) -> String {
    use document_tree::{
        element_categories as ec
    };
    let section_obj = top_level_rst_section(document);

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

fn first_document_paragraph(document: &document_tree::Document) -> String {
    use document_tree::{
        element_categories as ec
    };
    let section_obj = top_level_rst_section(document);

    // the title is expected to be at index 0; just see what immediately follows it
    let element: Option<&ec::StructuralSubElement> = section_obj.children().get(1);
    if let Some(element) = element {
        if let ec::StructuralSubElement::SubStructure(x) = element {
            // could also SubStructure(box x) = element but that's an unstable feature, will
            // revisit. https://github.com/rust-lang/rust/issues/29641
            if let ec::SubStructure::BodyElement(y) = &**x {
                if let ec::BodyElement::Paragraph(_) = &**y {
                    let mut rendered_bytes = Vec::new();
                    let doc = document_tree::Document::with_children(vec![element.clone()]);
                    rst_renderer::render_html(&doc, &mut rendered_bytes, false)
                        .expect("failed to render the first rst paragraph");
                    String::from_utf8(rendered_bytes).expect("only UTF-8 documents please")
                } else {
                    // Perhaps a list or something, behaviour would be ambiguous for now
                    "".to_string()
                }
            } else {
                // Perhaps an immediate subheading; ambiguous what to do. Recursing to its section
                // might be a good idea, but not necessarily intended by the author.
                "".to_string()
            }
        } else {
            panic!("what is this?")
        }
    } else {
        // the section has no children after the title
        "".to_string()
    }
}

struct RenderedRst {
    title: String,
    summary: String,
    body: String,
}

fn rstrender(s: &str) -> RenderedRst {
    if s == "" {
        // some pages might not need any body content if it comes from templates only
        return RenderedRst { title: "".to_string(), summary: "".to_string(), body: "".to_string() }
    }
    let document = rst_parser::parse(s).expect("failed to parse rst document");
    let mut rendered_bytes = Vec::new();
    rst_renderer::render_html(&document, &mut rendered_bytes, false)
        .expect("failed to render the rst document even though it parsed fine");
    let body = String::from_utf8(rendered_bytes).expect("only UTF-8 documents please");

    let title = document_title(&document);
    let summary = first_document_paragraph(&document);

    RenderedRst { title, summary, body }
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

fn take_until_attr(value: &tera::Value, args: &HashMap<String, tera::Value>)
-> tera::Result<tera::Value> {
    let arr = tera::try_get_value!("take_until_attr", "value", Vec<tera::Value>, value);
    if arr.is_empty() {
        return Ok(tera::Value::Null);
    }

    let key = match args.get("attribute") {
        Some(val) => tera::try_get_value!("take_until_attr", "attribute", String, val),
        None => return Err(tera::Error::msg("The `take_until_attr` filter has to have an `attribute` argument")),
    };

    let val_lookup = match args.get("value") {
        Some(val) => val,
        None => return Err(tera::Error::msg("The `take_until_attr` filter has to have an `value` argument")),
    };

    let json_pointer = get_json_pointer(&key);

    let value = arr.iter()
        .take_while(|&x| x.pointer(&json_pointer).map_or(false, |value| value != val_lookup))
        .collect::<Vec<_>>();

    Ok(tera::to_value(value).expect("couldn't re-value an array of Tera values??"))
}

#[derive(Debug, StructOpt)]
#[structopt(name = "rotuli", about = "The universal document processor")]
struct Opt {
    #[structopt(short, long, help = "read document sources from here")]
    source_path: PathBuf,
    #[structopt(short, long, help = "write the results here")]
    output_path: PathBuf,
    #[structopt(short, long, default_value="rst")]
    markup_language: MarkupLanguage,
    #[structopt(short, long, default_value="index")]
    directory_index: String,
}

fn main() {
    let opt = Opt::from_args();

    if opt.output_path.exists() {
        println!("error: output path already exists");
        return;
    }

    let site = Site::new(opt.source_path, opt.markup_language, &opt.directory_index);

    if site.is_empty() {
        panic!("no files found");
    }

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
    tera.register_filter("take_until_attr", take_until_attr);

    site.render(&tera, &opt.output_path);
}
