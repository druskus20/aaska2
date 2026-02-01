use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock},
};

pub mod db;
pub mod html;
pub mod prelude {
    pub use tracing::{debug, error, info, trace, warn};
}
pub use prelude::*;

struct Aaska {}

struct Config {
    md_options: pulldown_cmark::Options,
}

#[derive(Debug)]
pub struct Chonk {
    html: String,
    assets: Vec<String>,
    // other fields when we need to track metadata
    og_srcpath: SrcPath,
}

struct Asset {
    og_uri: String,
}

/// To avoid problems with assets being included in a relative way, every SrcPath should be a
/// cannonical, absolute path. Then, in the generated site, we map these to paths relative to the
/// site root.
///
/// The main problem that this avoids is having the same asset included in multiple different ways,
/// e.g. `images/pic.png` vs `../images/pic.png` vs `/images/pic.png`. It then makes it easier to
/// derive an Id from the SrcPath.
///
/// Problem: cannonicalization fails if the path does not exist.
/// Problem: we need to modify the generated HTML to point to the generated asset paths. These
/// require processing the assets first, to generate a hash. But then, we cannot simply process
/// them as we traverse the markdown. Maybe derive the hash from something different than the
/// content, i.e. the metadata...? Seems icky. OR! generate the hash from the SOURCE,not the
/// generated asset. And make sure no two generated assets will ever have the same name (i.e.
/// favicon extensions..)
///
///
///
/// SO ACTUALLY  I might not even need a graph.
/// All assets must exist, at parsing time. We simply make one salsa query to read from relative
/// path, which then in turn calls another salsa query to read from the cannonical path. This ensures
/// that even though an asset is included multiple times, we only actually read it once, but we
/// keep alsa's internal depencency graph there with the relative paths.
///
///
///
#[derive(Debug)]
pub struct SrcPath {
    path: PathBuf,
    filename_i: usize, // includes the e
    ext_index: usize,
}
impl SrcPath {
    pub fn new(path: PathBuf) -> Self {
        let filename_i = path
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .map(|s| {
                let full_path_str = path.to_str().unwrap();
                full_path_str.len() - s.len()
            })
            .expect("Failed to get filename index");
        let ext_index = path
            .extension()
            .and_then(|os_str| os_str.to_str())
            .map(|s| {
                let full_path_str = path.to_str().unwrap();
                full_path_str.len() - s.len() - 1 // -1 for the dot
            })
            .unwrap_or(path.to_str().unwrap().len()); // No extension case

        SrcPath {
            path,
            filename_i,
            ext_index,
        }
    }
    pub fn filename(&self) -> &str {
        &self.path.to_str().unwrap()[self.filename_i..]
    }
    pub fn filename_no_ext(&self) -> &str {
        &self.path.to_str().unwrap()[self.filename_i..self.ext_index]
    }
    pub fn ext(&self) -> &str {
        &self.path.to_str().unwrap()[self.ext_index..]
    }
}

pub struct Md(String);

pub struct MdFile {
    content: Md,
    src_path: SrcPath,
    // Do i need to keep track of the path?
}

impl MdFile {
    pub fn new_from_str(content: &str, filename: PathBuf) -> Self {
        MdFile {
            content: Md(content.to_string()),
            src_path: SrcPath::new(filename),
        }
    }
    pub fn read_from(path: PathBuf) -> Self {
        let content = std::fs::read_to_string(&path).expect("Failed to read markdown file");
        MdFile {
            content: Md(content),
            src_path: SrcPath::new(path),
        }
    }
}

// Global config
static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init() {
    let config = Config {
        md_options: pulldown_cmark::Options::all(),
    };
    CONFIG.set(config).ok().expect("Config already initialized");
}

fn config<'a>() -> &'a Config {
    CONFIG.get().expect("Config not initialized")
}

use pulldown_cmark::{CowStr, Event, Parser, Tag};

/// We actually are wrong in merging together parsing and generation. Parsing should be one query:
/// File -> Ast(includes asset names without reading the content)
/// Generation should be another:
/// Ast -> Html + Assets
pub fn md_to_chonk(md_file: MdFile) -> Chonk {
    let (html, mut assets) = {
        let options = config().md_options;
        let mut assets = Vec::new();
        let parser = Parser::new_ext(&md_file.content.0, options);

        // Slow - alternatively, modify the html writer?
        // Transform events: track assets and optionally modify URLs
        // I think it's best to modify the html generation to fetch the new urls from the Asset
        // collection
        let events = parser.inspect(|event| {
        match &event { // borrow event to avoid moving
            Event::Start(tag) => {
                match tag {
                    Tag::Image { dest_url, ..  } => {
                        assets.push(dest_url.to_string());
                    }
                    Tag::Link { dest_url, ..  } => {
                        if !is_internal_link(dest_url) {
                            return;
                        }
                        assets.push(dest_url.to_string());
                    }
                    _ => ()
                }
            }
            Event::Html(_html) | Event::InlineHtml(_html) => {
                warn!(
                    "HTML content found but skipped, any links or assets in HTML are not tracked."
                );
            }
            _ => (),
        }
    });

        let mut html = String::new();
        crate::html::push_html(&mut html, events);
        (html, assets)
    };

    assets.sort();
    assets.dedup();

    Chonk {
        html,
        assets,
        og_srcpath: md_file.src_path,
    }
}

/// Checks whether a link is an internal link (from our website) or an external link.
fn is_internal_link(link: &str) -> bool {
    todo!()
}
