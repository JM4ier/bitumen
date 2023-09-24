use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    time::SystemTime,
};

mod crc32;
pub mod flags;

/// Randomly generated, every byte is unique
const MAGIC: u32 = 0x2f_96_8b_6a;

#[repr(C)]
#[derive(Clone, Default, Debug)]
struct Metadata {
    modified_at: u64,
    file_size: u64,
    path_len: u16,
    perms: u16,
    owner: u16,
    group: u16,
    magic: u32,
    flags: u32,
    /// metadata checksum
    checksum: u32,
}

impl Metadata {
    fn check(&mut self) -> Result<(), ()> {
        if self.magic == MAGIC {
            Ok(())
        } else {
            Err(())
        }
    }

    fn kind(&self) -> &'static str {
        let file_flag = self.flags & 3;
        match file_flag {
            0 => "File",
            1 => "Directory",
            2 => "Soft Link",
            3 => "Hard Link",
            _ => unreachable!(),
        }
    }

    fn compute_checksum(&self) -> u32 {
        let bytes = self.as_bytes_without_checksum();
        crc32::digest(bytes)
    }

    fn set_checksum(&mut self) {
        self.checksum = self.compute_checksum();
        self.assert_checksum_valid();
    }

    fn assert_checksum_valid(&self) {
        assert_eq!(self.checksum, self.compute_checksum())
    }

    fn as_bytes_without_checksum(&self) -> &[u8] {
        let this_ptr = self as *const Metadata as *const u8 as u64;
        let chck_ptr = (&self.checksum) as *const u32 as *const u8 as u64;
        unsafe {
            std::slice::from_raw_parts(
                self as *const Metadata as *const u8,
                (chck_ptr - this_ptr) as usize,
            )
        }
    }

    fn as_bytes(&self) -> &[u8] {
        self.assert_checksum_valid();
        unsafe {
            std::slice::from_raw_parts(
                self as *const Metadata as *const u8,
                std::mem::size_of::<Metadata>(),
            )
        }
    }
}

#[test]
/// Header is 40 bytes in size.
fn header_size_test() {
    assert_eq!(40, core::mem::size_of::<Metadata>());
}

#[test]
fn as_bytes_without_checksum() {
    let mut meta = Metadata::default();
    meta.file_size = 34343;
    meta.flags = 23232;

    let b1 = meta.as_bytes_without_checksum().to_vec();
    meta.checksum = 0xAA_BB_AA_BB;
    let b2 = meta.as_bytes_without_checksum().to_vec();

    assert_eq!(b1.len(), 32);

    assert_eq!(b1, b2);
}

struct ArchivedFile {
    meta: Metadata,
    path: Vec<u8>,
    file_body: Vec<u8>,
}

struct ArchivedDir {
    meta: Metadata,
    path: Vec<u8>,
}

pub fn append_to_archive(archive: &mut impl Write, path: &Path) -> io::Result<()> {
    let path_str = path.as_os_str().as_bytes().to_vec();

    let mut flags: u32;
    let mut file_size: u64;
    let mut open_file = None;

    let modified_at = path
        .metadata()?
        .modified()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if path.is_file() {
        flags = flags::FILE;

        let file = std::fs::File::open(path)?;
        file_size = file.metadata()?.len();
        open_file = Some(file);
    } else if path.is_dir() {
        flags = flags::DIR;
        file_size = 0;
    } else {
        todo!("can only handle files and directories for now");
    }

    let meta = Metadata {
        modified_at,
        file_size,
        path_len: path_str.len() as _,
        perms: 0,
        owner: 0,
        group: 0,
        magic: MAGIC,
        flags,
        // needs to be calculated for header and footer separately.
        checksum: 0,
    };

    let mut header_meta = meta.clone();
    header_meta.flags |= flags::HEADER;
    header_meta.set_checksum();

    let mut footer_meta = meta.clone();
    footer_meta.set_checksum();

    // actual writing of stuff down here.
    archive.write(header_meta.as_bytes())?;
    archive.write(&path_str)?;
    if let Some(ref mut file) = open_file {
        std::io::copy(file, archive)?;
    }
    archive.write(footer_meta.as_bytes())?;

    Ok(())
}

pub fn recursive_archive(archive: &mut impl Write, path: &Path) -> io::Result<()> {
    fn find(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
        files.push(path.into());

        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                find(&entry.path(), files)?;
            }
        }

        Ok(())
    }

    let mut entries = vec![];
    find(path, &mut entries)?;

    for e in entries.iter() {
        if e.is_dir() {
            append_to_archive(archive, &e)?;
        }
    }

    for e in entries.iter() {
        if !e.is_dir() {
            append_to_archive(archive, &e)?;
        }
    }

    Ok(())
}

enum DecodeError {
    /// no further entries
    Exhausted,
    /// Generic Header Error
    Header,
    /// Generic Footer Error
    Footer,
    /// Faulty checksum
    Checksum,
    /// Cut off mid-file
    Crop,
}

fn read_meta<R: Read + Seek>(name: &str, archive: &mut R) -> Result<Metadata, DecodeError> {
    let mut bytes = [0u8; std::mem::size_of::<Metadata>()];
    archive.read_exact(&mut bytes).map_err(|e| {
        log::error!("Failed to decode {name}: {e:?}");
        DecodeError::Exhausted
    })?;

    let mut meta: Metadata = unsafe { std::mem::transmute(bytes) };
    meta.check().map_err(|e| {
        log::error!("{name} check failed: {e:?}");
        DecodeError::Header
    })?;

    Ok(meta)
}

fn read1<R: Read + Seek>(archive: &mut R) -> Result<(), DecodeError> {
    let header = read_meta("Header", archive)?;
    log::trace!("{header:?}");

    let mut path = vec![0u8; header.path_len as usize];
    archive.read_exact(&mut path).map_err(|e| {
        log::error!("Failed to read path: {e:?}");
        DecodeError::Crop
    })?;
    let path = String::from_utf8_lossy(&path);

    archive
        .seek(io::SeekFrom::Current(header.file_size as _))
        .map_err(|e| {
            log::error!("Failed to seek past file contents: {e:?}");
            DecodeError::Crop
        })?;

    let footer = read_meta("Footer", archive)?;

    log::info!(
        "{kind: <9} : {path} : {size}B",
        kind = header.kind(),
        size = header.file_size
    );

    Ok(())
}
pub fn read<R: Read + Seek>(archive: &mut R) {
    while let Ok(..) = read1(archive) {}
}
