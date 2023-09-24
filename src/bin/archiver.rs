use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    env_logger::init();

    let mut archive = vec![];
    bitumen::recursive_archive(&mut archive, &PathBuf::from("src"))?;
    std::fs::write("src.bit", &archive)?;

    let mut archive = std::io::Cursor::new(archive);
    bitumen::read(&mut archive);

    Ok(())
}
