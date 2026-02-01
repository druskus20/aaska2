use crate::{Chonk, SrcPath, prelude::*};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use eyre::{Context, Report, Result, eyre};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use pulldown_cmark::{Event, Parser, Tag};
use salsa::{Accumulator, Setter, Storage};

#[salsa::input]
struct File {
    path: SrcPath,
    #[returns(ref)]
    contents: String,
}

#[salsa::db]
#[derive(Clone)]
pub struct LazyInputDatabase {
    storage: Storage<Self>,
    #[cfg(test)]
    logs: Arc<Mutex<Vec<String>>>,
    in_mem_assets: DashMap<SrcPath, File>,
    file_watcher: Arc<Mutex<Debouncer<RecommendedWatcher>>>,
}

/// Source of an asset, a path, not loaded, with a cannonical path

#[derive(Clone, PartialEq, Eq, Hash)]
struct ParsedMdHash(Vec<u8>);

impl Deref for SrcPath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl LazyInputDatabase {
    pub fn new(tx: Sender<DebounceEventResult>) -> Self {
        let logs: Arc<Mutex<Vec<String>>> = Default::default();
        Self {
            storage: Storage::new(Some(Box::new({
                let logs = logs.clone();
                move |event| {
                    // don't log boring events
                    if let salsa::EventKind::WillExecute { .. } = event.kind {
                        logs.lock().unwrap().push(format!("{event:?}"));
                    }
                }
            }))),
            #[cfg(test)]
            logs,
            in_mem_assets: DashMap::new(),
            file_watcher: Arc::new(Mutex::new(
                new_debouncer(Duration::from_secs(1), tx).unwrap(),
            )),
        }
    }
}

#[salsa::db]
impl salsa::Database for LazyInputDatabase {}

#[salsa::db]
trait Db: salsa::Database {
    fn input(&self, path: SrcPath) -> Result<File>;
}

#[salsa::db]
impl Db for LazyInputDatabase {
    fn input(&self, path: SrcPath) -> Result<File> {
        Ok(match self.in_mem_assets.entry(path.clone()) {
            Entry::Occupied(entry) => *entry.get(),
            // If we haven't read this file yet set up the watch, read the
            // contents, store it in the cache, and return it.
            Entry::Vacant(entry) => {
                // Set up the watch before reading the contents to try to avoid
                // race conditions.
                let watcher = &mut *self.file_watcher.lock().unwrap();
                watcher
                    .watcher()
                    .watch(&path, RecursiveMode::NonRecursive)
                    .unwrap();
                let contents = std::fs::read_to_string(&*path)
                    .wrap_err_with(|| format!("Failed to read {}", &path.display()))?;
                *entry.insert(File::new(self, path, contents))
            }
        })
    }
}

fn hash_md(db: &dyn Db, md: &String) -> ParsedMdHash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(md.as_bytes());
    let result = hasher.finalize();
    ParsedMdHash(result.to_ascii_lowercase())
}

#[salsa::accumulator]
struct Diagnostic(String);

impl Diagnostic {
    fn push_error(db: &dyn Db, file: File, error: Report) {
        Diagnostic(format!(
            "Error in file {}: {:?}\n",
            file.path(db)
                .file_name()
                .unwrap_or_else(|| "<unknown>".as_ref())
                .to_string_lossy(),
            error,
        ))
        .accumulate(db);
    }
}

#[salsa::tracked]
struct ProcessedAsset<'db> {
    name: String,
}

#[salsa::tracked]
struct ParsedMd<'db> {
    parsed_md_hash: ParsedMdHash,
    assets: Vec<SrcPath>,
}

#[salsa::tracked]
fn process_asset(db: &dyn Db, input: File) -> ProcessedAsset<'_> {
    todo!()
}

#[salsa::tracked]
pub fn md_to_html(db: &dyn Db, md_file: File) -> Chonk<'_> {
    let options = crate::config().md_options;
    let mut assets = Vec::new();
    let file_contents = md_file.contents(db);
    let mut parser = Parser::new_ext(&file_contents, options);

    // Slow - alternatively, modify the html writer?
    // Transform events: track assets and optionally modify URLs
    // I think it's best to modify the html generation to fetch the new urls from the Asset
    // collection
    let events = parser.by_ref().inspect(|event| {
        match event {
            // borrow event to avoid moving
            Event::Start(tag) => match tag {
                Tag::Image { dest_url, .. } => {
                    assets.push(SrcPath::from_relaxed_path(PathBuf::from(dest_url.as_ref())));
                }
                Tag::Link { dest_url, .. } => {
                    if !is_internal_link(dest_url) {
                        return;
                    }
                    assets.push(SrcPath::from_relaxed_path(PathBuf::from(dest_url.as_ref())));
                }
                _ => (),
            },
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

    Chonk::new(db, html, assets, md_file.path(db))
}

/// Checks whether a link is an internal link (from our website) or an external link.
fn is_internal_link(link: &str) -> bool {
    todo!()
}

#[salsa::tracked]
fn process_md<'db>(db: &'db dyn Db, md: ParsedMd<'db>) -> ParsedMd<'db> {
    todo!()
}
