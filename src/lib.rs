use std::{
    borrow::Cow,
    path::Path,
    sync::{LazyLock, OnceLock},
};

pub mod prelude {}

struct Aaska {}

struct Config {
    md_options: pulldown_cmark::Options,
}

#[derive(Debug)]
pub struct Chonk {
    html: String,
    // other fields when we need to track metadata
    og_filename: Filename,
}

#[derive(Debug)]
pub struct Filename {
    filename: String,
    dot_index: usize,
}
impl Filename {
    pub fn new(filename: String) -> Self {
        let dot_index = filename.rfind('.').unwrap_or(filename.len());
        Self {
            filename,
            dot_index,
        }
    }
    pub fn name_no_ext(&self) -> &str {
        &self.filename[..self.dot_index]
    }
    pub fn ext(&self) -> &str {
        &self.filename[self.dot_index..]
    }
}

pub struct Md(String);

pub struct MdFile {
    content: Md,
    filename: Filename,
    // Do i need to keep track of the path?
}

impl MdFile {
    pub fn new_from_str(content: &str, filename: &str) -> Self {
        MdFile {
            content: Md(content.to_string()),
            filename: Filename::new(filename.to_string()),
        }
    }
    pub fn read_from(path: impl AsRef<Path>) -> Self {
        let content = std::fs::read_to_string(&path).expect("Failed to read markdown file");
        let filename = path
            .as_ref()
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .map(|s| s.to_string())
            .expect("Failed to get filename");
        MdFile {
            content: Md(content),
            filename: Filename::new(filename),
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

pub fn md_to_chonk(md_file: MdFile) -> Chonk {
    let html = {
        let parser = pulldown_cmark::Parser::new_ext(&md_file.content.0, config().md_options);
        let mut html_output = String::new();
        pulldown_cmark::html::push_html(&mut html_output, parser);
        html_output
    };

    Chonk {
        html,
        og_filename: md_file.filename,
    }
}
