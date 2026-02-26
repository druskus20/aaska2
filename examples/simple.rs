use aaska2::path::SrcPath;
use crossbeam_channel::unbounded;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    aaska2::init();
    let (tx, _rx) = crossbeam_channel::unbounded();
    let db = aaska2::db::Database::new_with_watcher(tx);
    let test_file = aaska2::db::File::new(
        &db,
        SrcPath::from_relaxed_path("examples/simple.rs"),
        String::from(
            "# Hello, Aaska2!\nThis is a simple markdown file.\n![An example image](image-old.png)\n",
        ),
    )?;

    let chonk = aaska2::db::render_chonk(&db, test_file).await?;
    dbg!(&chonk);
    dbg!(&chonk.html);
    Ok(())
}
