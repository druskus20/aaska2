use crate::{Chonk, SrcPath, internal_prelude::*};
use std::path::PathBuf;

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use eyre::{Context, Report, Result};
use picante::PicanteResult;
use pulldown_cmark::{Event, Parser, Tag};

#[picante::input]
pub struct SourceFile {
    #[key]
    pub path: SrcPath,
    pub contents: Vec<u8>,
}

impl SourceFile {
    pub fn from_disk<DB: Db>(db: &DB, path: SrcPath) -> Result<Self> {
        SourceFile::new(
            db,
            path.clone(),
            std::fs::read(&path).wrap_err_with(|| {
                format!("Failed to read file from disk at path {}", path.display())
            })?,
        )
        .wrap_err_with(|| format!("Failed to create SourceFile for path {}", path.display()))
    }
}

/// Source of an asset, a path, not loaded, with a cannonical path

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct ParsedMdHash(Vec<u8>);

// Picante database with custom fields for caching
#[picante::db(
    inputs(SourceFile),
    tracked(render_chonk, process_asset, process_md),
    db_trait(Db)
)]
pub struct AaskaDb {
    pub in_mem_assets: DashMap<SrcPath, SourceFile>,
}

impl AaskaDb {
    pub fn new_simple() -> Self {
        Self::new(DashMap::new())
    }

    pub fn input(&self, path: SrcPath) -> Result<SourceFile> {
        Ok(match self.in_mem_assets.entry(path.clone()) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let contents = std::fs::read(&*path)
                    .wrap_err_with(|| format!("Failed to read {}", &path.display()))?;
                *entry.insert(SourceFile::new(self, path, contents)?)
            }
        })
    }
}

fn hash_md(md: &[u8]) -> ParsedMdHash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(md);
    let result = hasher.finalize();
    ParsedMdHash(result.to_ascii_lowercase())
}

// Diagnostic accumulator replaced with simple logging
fn log_error(file: SourceFile, db: &AaskaDb, err: Report) {
    let filename = file
        .path(db)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "<unknown>".to_string());
    error!("Error in file {}: {:?}", filename, err);
}

// ProcessedAsset is now a regular struct
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProcessedAsset {
    name: String,
    hashed_name: String,
}

// ParsedMd is now a regular struct
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ParsedMd {
    parsed_md_hash: ParsedMdHash,
    assets: Vec<SrcPath>,
}

#[picante::tracked]
pub async fn render_chonk<DB: Db>(db: &DB, md_file: SourceFile) -> PicanteResult<Chonk> {
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::collections::HashMap;

    let options = crate::config().md_options;
    let file_contents = md_file.contents(db)?;
    let file_contents_str = std::str::from_utf8(&file_contents).unwrap();

    let md_path = md_file.path(db)?;
    let anchor_path = md_path.as_anchor();

    // First pass: collect assets by consuming the parser
    let parser1 = Parser::new_ext(file_contents_str, options);
    let mut assets = Vec::new();
    let mut asset_url_map: Vec<(String, SrcPath)> = Vec::new();

    for event in parser1 {
        match event {
            Event::Start(tag) => match tag {
                Tag::Image { dest_url, .. } => {
                    let original_url = dest_url.to_string();
                    let asset_path =
                        SrcPath::from_relaxed_path(PathBuf::from(dest_url.as_ref()), anchor_path);
                    assets.push(asset_path.clone());
                    asset_url_map.push((original_url, asset_path));
                }
                Tag::Link { dest_url, .. } => {
                    if !is_internal_link(&dest_url) {
                        continue;
                    }
                    let original_url = dest_url.to_string();
                    let asset_path =
                        SrcPath::from_relaxed_path(PathBuf::from(dest_url.as_ref()), anchor_path);
                    assets.push(asset_path.clone());
                    asset_url_map.push((original_url, asset_path));
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
    }

    // Load all SourceFiles first (outside async closures) to avoid picante cycles
    let mut asset_files: Vec<(String, SrcPath, SourceFile)> = Vec::new();
    for (original_url, asset_path) in asset_url_map.iter() {
        match SourceFile::from_disk(db, asset_path.clone()) {
            Ok(file) => {
                asset_files.push((original_url.clone(), asset_path.clone(), file));
            }
            Err(e) => {
                error!("Failed to load asset {}: {:?}", asset_path.display(), e);
            }
        }
    }

    // Process all assets in parallel using FuturesUnordered for true concurrent execution
    let parallel_start = std::time::Instant::now();

    let mut asset_futures = asset_files
        .into_iter()
        .map(|(original_url, asset_path, file)| async move {
            use std::time::Instant;
            let query_start = Instant::now();

            let result = match process_asset(db, file).await {
                Ok(processed) => {
                    let query_duration = query_start.elapsed();
                    info!(
                        "Processed asset {} -> {} in {:?}",
                        asset_path.filename(),
                        processed.hashed_name,
                        query_duration
                    );
                    Some((original_url, processed.hashed_name, query_duration))
                }
                Err(e) => {
                    error!("Failed to process asset {}: {:?}", asset_path.display(), e);
                    None
                }
            };
            result
        })
        .collect::<FuturesUnordered<_>>();

    let mut results = Vec::new();
    while let Some(result) = asset_futures.next().await {
        results.push(result);
    }

    let parallel_total = parallel_start.elapsed();

    let mut asset_map = HashMap::new();
    let mut query_times = Vec::new();
    for result in results.into_iter().flatten() {
        let (original, hashed, duration) = result;
        asset_map.insert(original, hashed);
        query_times.push(duration);
    }

    // Log average query time and actual parallel execution time
    if !query_times.is_empty() {
        let avg = query_times.iter().sum::<std::time::Duration>() / query_times.len() as u32;
        info!(
            "Processed {} assets in parallel. Average query time: {:?}, Total wall-clock time: {:?}",
            query_times.len(),
            avg,
            parallel_total
        );
    }

    // Second pass: generate HTML with URL resolver
    let parser2 = Parser::new_ext(file_contents_str, options);
    let mut html = String::new();
    crate::html::push_html_with_resolver(&mut html, parser2, |url: &str| {
        asset_map
            .get(url)
            .cloned()
            .unwrap_or_else(|| url.to_string())
    });

    Ok(Chonk {
        html,
        assets,
        og_srcpath: (*md_file.path(db)?).clone(),
    })
}

/// Checks whether a link is an internal link (from our website) or an external link.
/// If it's an internal link, it is a depencency
fn is_internal_link(link: &str) -> bool {
    todo!()
}

#[picante::tracked]
pub async fn process_md<DB: Db>(db: &DB, input: SourceFile) -> PicanteResult<ParsedMd> {
    todo!()
}

#[picante::tracked]
pub async fn process_asset<DB: Db>(db: &DB, input: SourceFile) -> PicanteResult<ProcessedAsset> {
    use sha2::{Digest, Sha256};

    let path = input.path(db)?;
    let contents = input.contents(db)?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Artificial delay to make parallelism visible (remove this later)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Generate hash of asset contents
    let mut hasher = Sha256::new();
    hasher.update(contents);
    let hash = hasher.finalize();
    let hash_str = hash
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let short_hash = &hash_str[..8]; // Use first 8 chars

    // Create hashed filename: name.hash.ext
    let hashed_name = if let Some((stem, ext)) = name.rsplit_once('.') {
        format!("{}.{}.{}", stem, short_hash, ext)
    } else {
        format!("{}.{}", name, short_hash)
    };

    Ok(ProcessedAsset { name, hashed_name })
}
