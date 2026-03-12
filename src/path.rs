use crate::internal_prelude::*;
use eyre::bail;

use std::{
    ops::Deref,
    path::{Component, Path, PathBuf},
};

pub enum SourcePath {
    Relative { anchor: String, rel_path: String },
    Absolute(String),
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct SrcPath {
    path: PathBuf,
    anchor_i: usize, // can be 0 if no anchor, otherwise it's the index of the relative path start
    filename_i: usize, // includes the e
    ext_i: usize,
}
impl SrcPath {
    pub fn from_relaxed_path(
        path: impl AsRef<Path>,
        anchor: impl AsRef<Path>,
    ) -> eyre::Result<Self> {
        dbg!(path.as_ref(), anchor.as_ref());
        let anchor_i = if path.as_ref().is_absolute() && anchor.as_ref().as_os_str().is_empty() {
            0
        } else if path.as_ref().is_relative() && !anchor.as_ref().as_os_str().is_empty() {
            anchor.as_ref().to_str().unwrap().len()
        } else {
            bail!(
                "Invalid path and anchor combination: path must be absolute if 
                 anchor is empty, and relative if anchor is non-empty"
            );
        };

        // remove any possible "./" and join
        let path = if anchor_i > 0 {
            let joined = anchor.as_ref().join(path.as_ref());
            collapse_path(joined)
        } else {
            collapse_path(path)
        };

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

        Ok(SrcPath {
            path,
            anchor_i,
            filename_i,
            ext_i: ext_index,
        })
    }
    /// Returns a string representation of the path up to the filename, which can be used as an
    /// anchor for resolving relative paths.
    pub fn as_anchor(&self) -> &str {
        &self.path.to_str().unwrap()[..self.filename_i]
    }
    pub fn filename(&self) -> &str {
        &self.path.to_str().unwrap()[self.filename_i..]
    }
    pub fn filename_no_ext(&self) -> &str {
        &self.path.to_str().unwrap()[self.filename_i..self.ext_i]
    }
    pub fn ext(&self) -> &str {
        &self.path.to_str().unwrap()[self.ext_i..]
    }
}

impl Deref for SrcPath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for SrcPath {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

// https://github.com/rust-lang/cargo/blob/e490fd421fd85efd5cd31a4aacbd0bb42a567d3e/crates/cargo-util/src/paths.rs#L84
// - Does not fail on invalid paths like std::fs::canonicalize
// - Does not resolve symlinks
// - Collapses `.` and `..` components
// - Does not access the filesystem (faster)
pub fn collapse_path(path: impl AsRef<Path>) -> PathBuf {
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
pub fn soft_cannonicalize_rel(path: impl AsRef<Path>, pwd: impl AsRef<Path>) -> PathBuf {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        pwd.as_ref().join(path.as_ref())
    };
    collapse_path(path)
}
pub fn soft_cannonicalize_cwd(path: impl AsRef<Path>) -> PathBuf {
    soft_cannonicalize_rel(path, std::env::current_dir().unwrap())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_collapse_path() {
        use super::collapse_path;
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
            let normalized = collapse_path(input);
            assert_eq!(normalized, std::path::PathBuf::from(expected));
        }
    }

    #[test]
    fn test_from_relaxed_path() {
        struct Case {
            input_path: &'static str,
            anchor: &'static str,
            expected_full: &'static str,
            expected_anchor: &'static str,
            expected_filename_no_ext: &'static str,
            expected_ext: &'static str,
        }
        let cases = vec![
            Case {
                input_path: "image.png",
                anchor: "content/post/",
                expected_full: "content/post/image.png",
                expected_anchor: "content/post/",
                expected_filename_no_ext: "image",
                expected_ext: "png",
            },
            Case {
                input_path: "/absolute/path/image.png",
                anchor: "",
                expected_full: "/absolute/path/image.png",
                expected_anchor: "/absolute/path/",
                expected_filename_no_ext: "image",
                expected_ext: "png",
            },
            Case {
                input_path: "image.png",
                anchor: "",
                expected_full: "image.png",
                expected_anchor: "",
                expected_filename_no_ext: "image",
                expected_ext: "png",
            },
        ];
        for case in cases {
            let src_path = super::SrcPath::from_relaxed_path(case.input_path, case.anchor).unwrap();
            assert_eq!(src_path.path, std::path::PathBuf::from(case.expected_full));
            assert_eq!(src_path.as_anchor(), case.expected_anchor);
            assert_eq!(src_path.filename_no_ext(), case.expected_filename_no_ext);
            assert_eq!(src_path.ext(), case.expected_ext);
        }
    }
}
