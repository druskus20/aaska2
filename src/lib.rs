use std::sync::OnceLock;

pub mod db;
pub mod html;
pub mod path;
pub mod prelude {
    pub use tracing::{debug, error, info, trace, warn};
}
pub use prelude::*;

use crate::path::SrcPath;

struct Aaska {}

struct Config {
    md_options: pulldown_cmark::Options,
}

#[salsa::tracked(debug)]
pub struct Chonk<'db> {
    pub html: String,
    pub assets: Vec<SrcPath>,
    // other fields when we need to track metadata
    pub og_srcpath: SrcPath,
}

struct Asset {
    og_uri: String,
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
