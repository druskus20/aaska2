use crate::prelude::*;
use std::{
    ops::Deref,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Eq, PartialEq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SrcPath {
    path: PathBuf,
    filename_i: usize, // includes the e
    ext_index: usize,
}
impl SrcPath {
    pub fn from_relaxed_path(path: impl AsRef<Path>) -> Self {
        eprintln!("Pwd: {:?}", std::env::current_dir());
        eprintln!("Relaxed path: {:?}", path.as_ref());
        let pwd = std::env::current_dir().expect("Failed to get current working directory");
        let path = crate::path::soft_cannonicalize(&path, &pwd);
        eprintln!("Canonicalized path: {:?}", path);

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

impl Deref for SrcPath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

// https://github.com/rust-lang/cargo/blob/e490fd421fd85efd5cd31a4aacbd0bb42a567d3e/crates/cargo-util/src/paths.rs#L84
// - Does not fail on invalid paths like std::fs::canonicalize
// - Does not resolve symlinks
// - Collapses `.` and `..` components
// - Does not access the filesystem (faster)
fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut components = path.as_ref().components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(Component::RootDir);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if ret.ends_with(Component::ParentDir) {
                    ret.push(Component::ParentDir);
                } else {
                    let popped = ret.pop();
                    if !popped && !ret.has_root() {
                        ret.push(Component::ParentDir);
                    }
                }
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

/// Canonicalizes a path relative to a given working directory without accessing the filesystem.
/// - this function resolves `.` and `..` components
/// - if the input path is relative, it is considered relative to `pwd`
/// - does not resolve symlinks
/// - does not fail on invalid or non-existent paths
/// - does not access the filesystem
pub fn soft_cannonicalize(path: impl AsRef<Path>, pwd: &Path) -> PathBuf {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        pwd.join(path.as_ref())
    };
    normalize_path(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_normalize_path() {
        use super::normalize_path;
        let cases = vec![
            ("a/b/c", "a/b/c"),
            ("a/./b/../c", "a/c"),
            ("/a/b/../c", "/a/c"),
            ("./a/b/c", "a/b/c"),
            ("a/b/c/..", "a/b"),
            ("a/b/c/.", "a/b/c"),
            ("a//b///c", "a/b/c"),
            ("a/b/c/../../d", "a/d"),
            ("/../a/b", "/a/b"),
            ("../a/b", "../a/b"),
        ];
        for (input, expected) in cases {
            let normalized = normalize_path(input);
            assert_eq!(normalized, std::path::PathBuf::from(expected));
        }
    }
}
