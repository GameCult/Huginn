//! The attachment saver: a verified chat image becomes one file in the Persona's sandbox.
//!
//! `save` re-verifies what the mouth stored through `chat_image::load_bytes` (the one verifier; nothing here sniffs, hashes or
//! sizes a file itself) and writes `attachments/<sha256>.<ext>` below the sandbox root. The name derives from the verified hash
//! and one of four extensions; no component comes from the event, the reply, the mouth or the stored path.
//!
//! Containment is by directory descriptor, not by a path check. The sandbox root is opened once by its configured path (the
//! operator's, not hers); every open below it is relative to a held descriptor and refuses a symlink, so a shell that swaps
//! `attachments/` for a link between two calls finds the next call refusing it. The bytes go to a temporary name in that directory
//! and are linked to the final name, which fails where a rename would replace her file; a crash leaves only the temporary.
//!
//! The file_received receipt means only that the mouth fetched and stored the file for her. It does not prove the file reached her sandbox. The receipt stays inside the mouth's fetch, so a failed save (Exists, WriteFailed, RepoTree, a Load skip) leaves the receipt standing; she is told the save failed in her own turn.

use std::path::{Path, PathBuf};

use crate::chat_client::ChatImage;
use crate::chat_image::{self, Skip};

/// Why a verified image was not saved. `Load` carries the verifier's own refusal, told by its own fixed reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveFault {
    /// A git work tree at the sandbox root or in `attachments/`: a Matrix file does not go into one.
    RepoTree,
    /// The sandbox root or `attachments/` is a link or not a directory.
    OutsideSandbox,
    /// A different file already holds the name.
    Exists,
    WriteFailed,
    Load(Skip),
}

/// The extension of a verified media type: one of four, never from outside.
fn extension(media_type: &str) -> Option<&'static str> {
    match media_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// Saves `image`, which the mouth stored for `room_id` under `store`, into `sandbox`. The path is relative to the sandbox:
/// `attachments/<sha256>.<ext>`. An identical file already there counts as saved.
pub(crate) fn save(store: &Path, sandbox: &Path, room_id: &str, image: &ChatImage) -> Result<PathBuf, SaveFault> {
    let (bytes, media_type) = chat_image::load_bytes(store, room_id, image).map_err(SaveFault::Load)?;
    let extension = extension(media_type).ok_or(SaveFault::Load(Skip::NotAnImage))?;
    let name = format!("{}.{extension}", image.sha256);
    write(sandbox, &name, &bytes)?;
    Ok(Path::new("attachments").join(name))
}

#[cfg(windows)]
fn write(_sandbox: &Path, _name: &str, _bytes: &[u8]) -> Result<(), SaveFault> {
    // The organ runs under bubblewrap on Linux; there is no descriptor-relative write here.
    Err(SaveFault::WriteFailed)
}

#[cfg(unix)]
fn write(sandbox: &Path, name: &str, bytes: &[u8]) -> Result<(), SaveFault> {
    use rustix::fs::{self, AtFlags, Mode, OFlags};
    use rustix::io::Errno;

    let dir_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let refused = |errno: Errno| if matches!(errno, Errno::LOOP | Errno::NOTDIR) { SaveFault::OutsideSandbox } else { SaveFault::WriteFailed };
    let work_tree = |dir: &rustix::fd::OwnedFd| match fs::statat(dir, ".git", AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Err(SaveFault::RepoTree),
        Err(Errno::NOENT) => Ok(()),
        Err(_) => Err(SaveFault::WriteFailed),
    };

    let root = fs::open(sandbox, dir_flags, Mode::empty()).map_err(refused)?;
    work_tree(&root)?;
    match fs::mkdirat(&root, "attachments", Mode::RWXU) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(_) => return Err(SaveFault::WriteFailed),
    }
    let dir = fs::openat(&root, "attachments", dir_flags, Mode::empty()).map_err(refused)?;
    work_tree(&dir)?;

    let temp = format!(".{name}.part");
    // A crashed save's leavings, even a FIFO or a link, are removed without being followed.
    match fs::unlinkat(&dir, temp.as_str(), AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => {}
        Err(_) => return Err(SaveFault::WriteFailed),
    }
    #[cfg(test)]
    tests::between_unlink_and_create();
    let file = fs::openat(&dir, temp.as_str(), OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::RUSR | Mode::WUSR)
        .map_err(|_| SaveFault::WriteFailed)?;
    let landed = (|| {
        let mut rest = bytes;
        while !rest.is_empty() {
            #[cfg(test)]
            if tests::crash_after(bytes.len() - rest.len()) {
                return Err(Errno::IO);
            }
            let chunk = &rest[..rest.len().min(8192)];
            match rustix::io::write(&file, chunk) {
                Ok(0) => return Err(Errno::IO),
                Ok(n) => rest = &rest[n..],
                Err(Errno::INTR) => {}
                Err(e) => return Err(e),
            }
        }
        fs::fsync(&file)?;
        fs::linkat(&dir, temp.as_str(), &dir, name, AtFlags::empty())
    })();
    match landed {
        Ok(()) => {
            let _ = fs::unlinkat(&dir, temp.as_str(), AtFlags::empty());
            Ok(())
        }
        Err(Errno::EXIST) => {
            let _ = fs::unlinkat(&dir, temp.as_str(), AtFlags::empty());
            same_file(&dir, name, bytes)
        }
        // A simulated crash leaves the temporary as a real one would.
        #[cfg(test)]
        Err(Errno::IO) if tests::crashed() => Err(SaveFault::WriteFailed),
        Err(_) => {
            let _ = fs::unlinkat(&dir, temp.as_str(), AtFlags::empty());
            Err(SaveFault::WriteFailed)
        }
    }
}

/// Whether the file at `name` holds exactly `bytes`. It is a name her shell controls, so it is opened without following a link
/// and without waiting (a FIFO opens), must be a regular file of exactly the image's length, and is read for at most that length
/// plus one byte. Anything else is `Exists` with nothing read.
#[cfg(unix)]
fn same_file(dir: &rustix::fd::OwnedFd, name: &str, bytes: &[u8]) -> Result<(), SaveFault> {
    use rustix::fs::{self, FileType, Mode, OFlags};

    let file = fs::openat(dir, name, OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC, Mode::empty()).map_err(|_| SaveFault::Exists)?;
    let stat = fs::fstat(&file).map_err(|_| SaveFault::Exists)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || u64::try_from(stat.st_size) != Ok(bytes.len() as u64) {
        return Err(SaveFault::Exists);
    }
    let mut held = vec![0u8; bytes.len() + 1];
    let mut filled = 0;
    while filled < held.len() {
        match rustix::io::read(&file, &mut held[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => return Err(SaveFault::Exists),
        }
    }
    if held[..filled] == *bytes { Ok(()) } else { Err(SaveFault::Exists) }
}

#[cfg(test)]
mod tests;
