use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut archive = vec![];
    bitumen::recursive_archive(&mut archive, &PathBuf::from("src"))?;
    std::fs::write("src.bit", &archive)?;
    Ok(())
}
