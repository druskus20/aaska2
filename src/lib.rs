use std::sync::OnceLock;

pub mod db;
pub mod html;
pub mod path;
pub mod path2;
pub(crate) mod internal_prelude {
    pub use eyre::{Context, Result, WrapErr, bail, eyre};
    pub use tracing::{debug, error, info, trace, warn};
}

pub mod prelude {}

use crate::path::SrcPath;

struct Aaska {}

struct Config {
    md_options: pulldown_cmark::Options,
}

// Chonk is now a regular struct returned by render_chonk
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Chonk {
    pub html: String,
    pub assets: Vec<SrcPath>,
    // other fields when we need to track metadata
    pub og_srcpath: SrcPath,
}

struct Asset {
    og_uri: String,
}

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
