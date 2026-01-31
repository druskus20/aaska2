use aaska2::{Md, MdFile};

fn main() {
    aaska2::init();
    let test_file = MdFile::new_from_str(
        "# Hello, Aaska2!\nThis is a simple markdown file.",
        "example.md",
    );
    let chonk = aaska2::md_to_chonk(test_file);
    dbg!(chonk);
}
