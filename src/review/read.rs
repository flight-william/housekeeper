//! Reading the record folder. Only a regular file named `<YYYY-MM>.jsonl` is read;
//! any other entry with such a name (a folder, a FIFO, a socket, a symbolic link,
//! dangling or not) is skipped and counted, never followed or opened in a way that
//! could block. A regular file that cannot be opened or read is an error.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// `open(2)` flags: never follow a link that replaced the file after it was listed,
/// and never wait for a writer if a FIFO took its place.
#[cfg(target_os = "macos")]
const SAFE_OPEN: i32 = 0x0100 | 0x0004; // O_NOFOLLOW | O_NONBLOCK
#[cfg(target_os = "macos")]
const ELOOP: i32 = 62;
#[cfg(target_os = "linux")]
const SAFE_OPEN: i32 = libc_linux::O_NOFOLLOW | libc_linux::O_NONBLOCK;
#[cfg(target_os = "linux")]
const ELOOP: i32 = 40;

#[cfg(target_os = "linux")]
mod libc_linux {
    #[cfg(target_arch = "aarch64")]
    pub const O_NOFOLLOW: i32 = 0o100000;
    #[cfg(not(target_arch = "aarch64"))]
    pub const O_NOFOLLOW: i32 = 0o400000;
    pub const O_NONBLOCK: i32 = 0o4000;
}

/// What reading the folder found besides its lines.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Folder {
    /// Entries named like a month's records that are not regular files.
    pub skipped_files: usize,
}

/// Hand `each` every line of every month's record file in `dir`, oldest month
/// first; a folder that does not exist holds no records.
pub fn each_line(dir: &Path, mut each: impl FnMut(&[u8])) -> std::io::Result<Folder> {
    let mut folder = Folder::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(folder),
        Err(e) => return Err(e),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(month_file) {
            // The entry's own type: a symbolic link is not followed.
            if entry.file_type()?.is_file() {
                files.push(entry.path());
            } else {
                folder.skipped_files += 1;
            }
        }
    }
    files.sort();
    for path in files {
        let Some(mut file) = open_regular(&path)? else {
            folder.skipped_files += 1;
            continue;
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        bytes.split(|b| *b == b'\n').for_each(&mut each);
    }
    Ok(folder)
}

/// `path` opened for reading when it is still a regular file, `None` when a link,
/// a FIFO or anything else took its place since it was listed.
fn open_regular(path: &Path) -> std::io::Result<Option<File>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(SAFE_OPEN)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.raw_os_error() == Some(ELOOP) => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(file.metadata()?.is_file().then_some(file))
}

/// `YYYY-MM.jsonl`, the name house gives a month's records.
fn month_file(name: &str) -> bool {
    let b = name.as_bytes();
    name.len() == 13
        && name.ends_with(".jsonl")
        && b[4] == b'-'
        && b[..4].iter().chain(&b[5..7]).all(u8::is_ascii_digit)
}
