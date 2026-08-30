use std::io::Read;
use std::path::{Path, PathBuf};

pub fn select_binary() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Select libil2cpp binary")
        .pick_file()
}

pub fn select_metadata() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Select global-metadata.dat")
        .pick_file()
}

pub fn select_dump_destination() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export dump.cs")
        .set_file_name("dump.cs")
        .save_file()
}

pub fn dropped_target(path: &Path) -> Option<TargetFile> {
    let mut bytes = [0; 4];
    std::fs::File::open(path)
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    if bytes == *b"\x7fELF" {
        Some(TargetFile::Binary(path.to_owned()))
    } else if bytes == 0xFAB1_1BAFu32.to_le_bytes() {
        Some(TargetFile::Metadata(path.to_owned()))
    } else {
        None
    }
}

pub enum TargetFile {
    Binary(PathBuf),
    Metadata(PathBuf),
}
