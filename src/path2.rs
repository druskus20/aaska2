use crate::internal_prelude::*;
use std::path::{Path, PathBuf};

/// Paths must be formed always relative to an anchor. Usually, that anchor is the cwd. The CWD is
/// also relative to the base dir.
///
/// In the case of a markdown file containing links, the links will be formed relative to the
/// markdown file. The markdown file itself is relative to the cwd (in this example). This means we
/// end up with:
///
///     base_site <- cwd <- mdfile <- link
///
/// The api i want to enable is
///
/// let link = "./../images/image.png"
/// let cwd = "./modules"
/// let mut path = AllPath::from(base_site);
/// path.enter("modules");
/// path.enter("mydir/myotherdir");
///
/// for file_name in path.list_file_names("*.md") {
///     process_md_file(file!(file.relative_to(path)));
/// }
/// fn process_md_file(file_path: FilePath) {
///
///     let md_file = read_markdown_file(file_path);
///     for link in md_file.list_internal_links() {
///         let link = if link.is_relative() {
///             let path: AllPath = path.parent_dir_path();
///             process(link.relative_to(path))
///         }
///     }
/// }
///
/// > Thigns to keep in mind: paths can be many things, but mainly file or dir.
/// > We treat them differently when including relative things. A file can point to siblings with
/// > ./sibling,  but a directory can include children with ./child, siblings with ../sibling, and
/// > parents with ../parent.  
///
/// Actually NO! what happens is that links that are found in a website (or md file) refer to the
/// directory where that file is found not the path of the file. So this should happen IN
/// process_md_file
///
///
///

//struct SrcPathParts {
//    path: PathBuf,
//    anchor: Anchor,
//}
//
//enum Anchor {
//    Abs(PathBuf),
//    Rel(SrcPathParts),
//}

pub struct SrcPath {
    anchors: Vec<usize>,
    buffer: PathBuf,
}

impl SrcPath {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let buffer = path.as_ref().to_path_buf();
        let byte_len = buffer.as_os_str().as_encoded_bytes().len();
        Self {
            anchors: vec![byte_len],
            buffer,
        }
    }

    pub fn enter(&mut self, segment: impl AsRef<Path>) {
        self.buffer.push(segment);
        let byte_len = self.buffer.as_os_str().as_encoded_bytes().len();
        self.anchors.push(byte_len);
    }

    pub fn as_path(&self) -> &Path {
        self.buffer.as_path()
    }

    pub fn is_relative(&self) -> bool {
        self.buffer.is_relative()
    }

    pub fn parent_dir_path(&self) -> SrcPath {
        let mut new_buffer = self.buffer.clone();
        new_buffer.pop();
        let mut new_anchors = self.anchors.clone();
        new_anchors.pop();
        SrcPath {
            anchors: new_anchors,
            buffer: new_buffer,
        }
    }

    pub fn relative_to(&mut self, base: &SrcPath) {
        // Assert that self is a relative path
        assert!(
            self.buffer.is_relative(),
            "relative_to() requires self to be a relative path, got: {:?}",
            self.buffer
        );

        // Save self's buffer and anchors before replacing
        let old_buffer = std::mem::take(&mut self.buffer);
        let old_anchors = std::mem::take(&mut self.anchors);

        // Build new buffer: base + old_buffer
        self.buffer = base.buffer.clone();
        self.buffer.push(&old_buffer);

        // Calculate offset: where does old_buffer's content start in new buffer?
        let final_byte_len = self.buffer.as_os_str().as_encoded_bytes().len();
        let old_buffer_byte_len = old_buffer.as_os_str().as_encoded_bytes().len();
        let offset = final_byte_len - old_buffer_byte_len;

        // Build new anchors: base's anchors + shifted relative's anchors
        self.anchors = base.anchors.clone();
        for &anchor in &old_anchors {
            self.anchors.push(anchor + offset);
        }
    }

    pub fn dbg_print(&self) {
        println!("SrcPath: {}", self.buffer.display());
        println!("Creation chain:");

        let bytes = self.buffer.as_os_str().as_encoded_bytes();
        for (i, &end) in self.anchors.iter().enumerate() {
            let full_slice = &bytes[0..end];
            // SAFETY: We only slice at boundaries created by valid PathBuf operations
            let full_os_str = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(full_slice) };
            let full_path = Path::new(full_os_str);

            if i == 0 {
                // First entry: show everything in bold since it's all new
                println!("  \x1b[1m{}\x1b[0m", full_path.display());
            } else {
                // Subsequent entries: show base path normally, new part in bold
                let prev_end = self.anchors[i - 1];
                let base_slice = &bytes[0..prev_end];
                let new_slice = &bytes[prev_end..end];

                let base_os_str = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(base_slice) };
                let new_os_str = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(new_slice) };

                let base_path = Path::new(base_os_str);
                let new_path = Path::new(new_os_str);

                println!("  → {}\x1b[1m{}\x1b[0m", base_path.display(), new_path.display());
            }
        }
    }

    pub fn list_file_names(&self, pattern: &str) -> Vec<String> {
        use std::fs;

        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.buffer) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Simple pattern matching - just check if pattern is "*" or matches
                    if pattern == "*" || pattern.contains('*') {
                        // For simplicity, just return all files if pattern contains *
                        results.push(file_name);
                    } else if file_name == pattern {
                        results.push(file_name);
                    }
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let path = SrcPath::new("/base/site");
        assert_eq!(path.as_path(), Path::new("/base/site"));
        assert_eq!(path.anchors.len(), 1);
        assert_eq!(path.anchors[0], "/base/site".as_bytes().len());
    }

    #[test]
    fn test_enter() {
        let mut path = SrcPath::new("/base/site");
        let initial_anchor = path.anchors[0];

        path.enter("modules");
        assert_eq!(path.as_path(), Path::new("/base/site/modules"));
        assert_eq!(path.anchors.len(), 2);
        assert_eq!(path.anchors[0], initial_anchor);
        assert_eq!(path.anchors[1], "/base/site/modules".as_bytes().len());

        path.enter("mydir");
        assert_eq!(path.as_path(), Path::new("/base/site/modules/mydir"));
        assert_eq!(path.anchors.len(), 3);
    }

    #[test]
    fn test_relative_to_basic() {
        let base = SrcPath::new("/base/site");
        let mut relative = SrcPath::new("my/path");

        relative.relative_to(&base);

        assert_eq!(relative.as_path(), Path::new("/base/site/my/path"));
        assert_eq!(relative.anchors.len(), 2);
        // First anchor should be the base
        assert_eq!(relative.anchors[0], "/base/site".as_bytes().len());
        // Second anchor should be the full path
        assert_eq!(relative.anchors[1], "/base/site/my/path".as_bytes().len());
    }

    #[test]
    fn test_relative_to_with_chain() {
        let mut base = SrcPath::new("/base/site");
        base.enter("modules");

        let mut relative = SrcPath::new("images/img.png");
        relative.relative_to(&base);

        assert_eq!(
            relative.as_path(),
            Path::new("/base/site/modules/images/img.png")
        );
        assert_eq!(relative.anchors.len(), 3);
        // Should preserve base's chain
        assert_eq!(relative.anchors[0], "/base/site".as_bytes().len());
        assert_eq!(relative.anchors[1], "/base/site/modules".as_bytes().len());
        assert_eq!(
            relative.anchors[2],
            "/base/site/modules/images/img.png".as_bytes().len()
        );
    }

    #[test]
    fn test_relative_to_preserves_both_chains() {
        // Base has a chain: /base/site -> /base/site/modules -> /base/site/modules/subdir
        let mut base = SrcPath::new("/base/site");
        base.enter("modules");
        base.enter("subdir");

        // Relative also has a chain with ./ and ../: ./docs -> ./docs/../images -> ./docs/../images/img.png
        let mut relative = SrcPath::new("./docs");
        relative.enter("../images");
        relative.enter("img.png");

        // Both chains should be preserved
        relative.relative_to(&base);

        println!("\n=== Test: relative_to_preserves_both_chains ===");
        relative.dbg_print();

        assert_eq!(
            relative.as_path(),
            Path::new("/base/site/modules/subdir/./docs/../images/img.png")
        );
        assert_eq!(relative.anchors.len(), 6);

        // Verify each anchor reconstructs correctly
        let bytes = relative.buffer.as_os_str().as_encoded_bytes();

        let path0 = unsafe {
            Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
                &bytes[0..relative.anchors[0]],
            ))
        };
        let path1 = unsafe {
            Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
                &bytes[0..relative.anchors[1]],
            ))
        };
        let path2 = unsafe {
            Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
                &bytes[0..relative.anchors[2]],
            ))
        };
        let path3 = unsafe {
            Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
                &bytes[0..relative.anchors[3]],
            ))
        };
        let path4 = unsafe {
            Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
                &bytes[0..relative.anchors[4]],
            ))
        };
        let path5 = unsafe {
            Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(
                &bytes[0..relative.anchors[5]],
            ))
        };

        assert_eq!(path0, Path::new("/base/site"));
        assert_eq!(path1, Path::new("/base/site/modules"));
        assert_eq!(path2, Path::new("/base/site/modules/subdir"));
        assert_eq!(path3, Path::new("/base/site/modules/subdir/./docs"));
        assert_eq!(path4, Path::new("/base/site/modules/subdir/./docs/../images"));
        assert_eq!(path5, Path::new("/base/site/modules/subdir/./docs/../images/img.png"));
    }

    #[test]
    #[should_panic(expected = "relative_to() requires self to be a relative path")]
    fn test_relative_to_panics_on_absolute() {
        let base = SrcPath::new("/base/site");
        let mut absolute = SrcPath::new("/absolute/path");

        absolute.relative_to(&base);
    }

    #[test]
    fn test_relative_to_with_parent_dir() {
        let mut base = SrcPath::new("/base/site");
        base.enter("modules");
        base.enter("subdir");

        let mut relative = SrcPath::new("../images/img.png");
        relative.relative_to(&base);

        // Note: we don't normalize, so the path contains ..
        assert_eq!(
            relative.as_path(),
            Path::new("/base/site/modules/subdir/../images/img.png")
        );
        assert_eq!(relative.anchors.len(), 4);

        relative.dbg_print();
    }

    #[test]
    fn test_relative_to_with_current_dir() {
        let mut base = SrcPath::new("/base/site");
        base.enter("modules");

        let mut relative = SrcPath::new("./images/img.png");
        relative.relative_to(&base);

        // ./ is preserved (not normalized)
        assert_eq!(
            relative.as_path(),
            Path::new("/base/site/modules/./images/img.png")
        );
        assert_eq!(relative.anchors.len(), 3);

        relative.dbg_print();
    }

    #[test]
    fn test_relative_to_complex_chain_with_dots() {
        // Base chain with multiple levels
        let mut base = SrcPath::new("/base/site");
        base.enter("modules");
        base.enter("subdir");

        // Relative chain with ./ and ..
        let mut relative = SrcPath::new("./docs");
        relative.enter("../images");
        relative.enter("img.png");

        relative.relative_to(&base);

        assert_eq!(
            relative.as_path(),
            Path::new("/base/site/modules/subdir/./docs/../images/img.png")
        );

        // Should have 3 base anchors + 3 relative anchors = 6 total
        assert_eq!(relative.anchors.len(), 6);

        println!("\nComplex chain with dots:");
        relative.dbg_print();

        // Verify all anchors reconstruct correctly
        let bytes = relative.buffer.as_os_str().as_encoded_bytes();
        for (i, &anchor) in relative.anchors.iter().enumerate() {
            let path = unsafe {
                Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(&bytes[0..anchor]))
            };
            println!("Anchor {}: {} (byte pos {})", i, path.display(), anchor);
        }
    }

    #[test]
    fn test_parent_dir_path() {
        let mut path = SrcPath::new("/base/site");
        path.enter("modules");
        path.enter("mydir");

        let parent = path.parent_dir_path();
        assert_eq!(parent.as_path(), Path::new("/base/site/modules"));
        assert_eq!(parent.anchors.len(), 2);
    }

    #[test]
    fn test_is_relative() {
        let abs_path = SrcPath::new("/base/site");
        assert!(!abs_path.is_relative());

        let rel_path = SrcPath::new("my/path");
        assert!(rel_path.is_relative());
    }

    #[test]
    fn test_dbg_print_chain() {
        let mut path = SrcPath::new("/base/site");
        path.enter("modules");
        path.enter("mydir");

        // This will print to stdout, but we can at least verify it doesn't panic
        path.dbg_print();

        // Verify the anchors reconstruct correctly
        let bytes = path.buffer.as_os_str().as_encoded_bytes();
        let slice1 = &bytes[0..path.anchors[0]];
        let slice2 = &bytes[0..path.anchors[1]];
        let slice3 = &bytes[0..path.anchors[2]];

        let path1 = unsafe { Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(slice1)) };
        let path2 = unsafe { Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(slice2)) };
        let path3 = unsafe { Path::new(std::ffi::OsStr::from_encoded_bytes_unchecked(slice3)) };

        assert_eq!(path1, Path::new("/base/site"));
        assert_eq!(path2, Path::new("/base/site/modules"));
        assert_eq!(path3, Path::new("/base/site/modules/mydir"));
    }
}
