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

#[salsa::tracked(debug)]
pub struct Chonk<'db> {
    html: String,
    assets: Vec<SrcPath>,
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
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct SrcPath {
    path: PathBuf,
    filename_i: usize, // includes the e
    ext_index: usize,
}
impl SrcPath {
    pub fn from_relaxed_path(path: PathBuf) -> Self {
        let path = std::fs::canonicalize(&path).expect("Failed to canonicalize relaxed path");

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
            src_path: SrcPath::from_relaxed_path(filename),
        }
    }
    pub fn read_from(path: PathBuf) -> Self {
        let content = std::fs::read_to_string(&path).expect("Failed to read markdown file");
        MdFile {
            content: Md(content),
            src_path: SrcPath::from_relaxed_path(path),
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
