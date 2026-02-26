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
use picante::PicanteResult;

#[picante::input]
pub struct File {
    #[key]
    path: SrcPath,
    contents: String,
}

/// Source of an asset, a path, not loaded, with a cannonical path

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct ParsedMdHash(Vec<u8>);

// Picante database with custom fields for file watching and caching
#[picante::db(inputs(File), tracked(render_chonk, process_asset, process_md))]
pub struct Database {
    pub in_mem_assets: DashMap<SrcPath, File>,
    pub file_watcher: Arc<Mutex<Debouncer<RecommendedWatcher>>>,
}

impl Database {
    pub fn new_with_watcher(tx: Sender<DebounceEventResult>) -> Self {
        Self::new(
            DashMap::new(),
            Arc::new(Mutex::new(
                new_debouncer(Duration::from_secs(1), tx).unwrap(),
            )),
        )
    }

    pub fn input(&self, path: SrcPath) -> Result<File> {
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
                *entry.insert(File::new(self, path, contents)?)
            }
        })
    }
}

fn hash_md(md: &String) -> ParsedMdHash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(md.as_bytes());
    let result = hasher.finalize();
    ParsedMdHash(result.to_ascii_lowercase())
}

// Diagnostic accumulator replaced with simple logging
fn log_error(file: File, db: &Database, err: Report) {
    let filename = file.path(db)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "<unknown>".to_string());
    error!("Error in file {}: {:?}", filename, err);
}

// ProcessedAsset is now a regular struct
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct ProcessedAsset {
    name: String,
}

// ParsedMd is now a regular struct
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct ParsedMd {
    parsed_md_hash: ParsedMdHash,
    assets: Vec<SrcPath>,
}

#[picante::tracked]
async fn process_asset<DB: DatabaseTrait>(db: &DB, input: File) -> PicanteResult<ProcessedAsset> {
    todo!()
}

#[picante::tracked]
pub async fn render_chonk<DB: DatabaseTrait>(db: &DB, md_file: File) -> PicanteResult<Chonk> {
    let options = crate::config().md_options;
    let mut assets = Vec::new();
    let file_contents = md_file.contents(db)?;
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

    Ok(Chonk {
        html,
        assets,
        og_srcpath: (*md_file.path(db)?).clone(),
    })
}

/// Checks whether a link is an internal link (from our website) or an external link.
fn is_internal_link(link: &str) -> bool {
    todo!()
}

#[picante::tracked]
async fn process_md<DB: DatabaseTrait>(db: &DB, md: ParsedMd) -> PicanteResult<ParsedMd> {
    todo!()
}
