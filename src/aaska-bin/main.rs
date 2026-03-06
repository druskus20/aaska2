#[allow(unused)]
mod prelude {
    pub use eyre::{Context, ContextCompat, WrapErr, eyre};
    pub use tracing::{debug, error, info, trace, warn};
}
use std::path::Path;

use prelude::*;

use eyre::{Context, Result, bail};

mod cli;

#[macro_export]
macro_rules! check {
    ($cond:expr $(,)?) => {
        if !$cond {
            tracing::error!("assertion failed: {}", stringify!($cond));
            std::process::exit(1);
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            tracing::error!(
                "assertion failed: {}",
                format_args!($($arg)+)
            );
            std::process::exit(1);
        }
    };
}

#[tokio::main]
async fn main() {
    let cli = cli::parse_args();
    init_tracing(&cli);
    aaska2::init();

    match cli.command() {
        cli::Command::Run { root } => {
            let path = root.clone().unwrap_or_else(|| {
                std::env::current_dir()
                    .expect_tracing("Failed to get current directory")
                    .to_str()
                    .expect_tracing("Failed to convert current directory to string")
                    .to_string()
            });
            run(&path).expect_tracing("Failed to run Aaska")
        }
    }
}

fn run(path: &str) -> Result<()> {
    info!("Run");
    let base_paths = compute_aaska_paths(path);
    if !base_paths.are_valid() {
        bail!("Invalid base paths");
    }

    let _db = aaska2::db::AaskaDb::new_simple();
    let md_files = glob(base_paths.content, "**/*.md")?;

    dbg!(md_files);

    Ok(())
}

use tracing::error;

pub trait ExpectWithTracing<T> {
    fn expect_tracing(self, msg: &str) -> T;
}

impl<T> ExpectWithTracing<T> for Option<T> {
    fn expect_tracing(self, msg: &str) -> T {
        match self {
            Some(v) => v,
            None => {
                error!("{}", msg);
                std::process::exit(1);
            }
        }
    }
}

impl<T, E> ExpectWithTracing<T> for std::result::Result<T, E>
where
    E: std::fmt::Debug,
{
    fn expect_tracing(self, msg: &str) -> T {
        match self {
            Ok(v) => v,
            Err(err) => {
                error!("{}: {:#?}", msg, err);
                std::process::exit(1);
            }
        }
    }
}

fn init_tracing(cli: &cli::Args) {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .without_time()
        .with_max_level(cli.log_level())
        .init();
}

struct AaskaBasePaths {
    root: std::path::PathBuf,
    content: std::path::PathBuf,
}

fn compute_aaska_paths(root: &str) -> AaskaBasePaths {
    let root = aaska2::path::soft_cannonicalize_cwd(root);
    let content = aaska2::path::soft_cannonicalize_rel("content", &root);

    AaskaBasePaths { root, content }
}

impl AaskaBasePaths {
    fn are_valid(&self) -> bool {
        if !self.root.is_dir() {
            error!("Root path {} is not a directory", self.root.display());
            return false;
        }
        if !self.content.is_dir() {
            error!("Content path {} is not a directory", self.content.display());
            return false;
        }
        info!("Root path: {}", self.root.display());
        info!("Content path: {}", self.content.display());
        true
    }

    fn content(&self) -> &std::path::Path {
        &self.content
    }
}

fn glob(base: impl AsRef<Path>, glob: &str) -> Result<Vec<std::path::PathBuf>> {
    // check base is valid path string
    let pattern = base
        .as_ref()
        .join(glob)
        .to_str()
        .wrap_err_with(|| {
            format!(
                "Failed to construct glob pattern from base {} and glob {}",
                base.as_ref().display(),
                glob
            )
        })?
        .to_string();

    let paths = glob::glob(&pattern)
        .wrap_err_with(|| format!("Failed to read glob pattern {}", pattern))?
        .map(|res| {
            res.wrap_err_with(|| format!("Failed to read path from glob pattern {}", pattern))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(paths)
}
