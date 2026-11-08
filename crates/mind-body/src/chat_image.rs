//! A stored chat image, loaded for a model: the one place an image reference from the mouth becomes bytes.
//!
//! The mouth (Bifrost) fetches an image event's media with the Persona's own credential and stores it under that
//! Persona's image store, `<store>/<hex sha256 of the room id>/<hex sha256 of the bytes>[.<ext>]`. The delivery carries
//! a `ChatImage` reference, never bytes. Nothing in it is trusted here: the path is checked to be that exact shape, to
//! name this delivery's room and to resolve to itself with every link followed (so no `..`, no absolute path and no
//! symlink gets out); the file's length is held to `MAX_BYTES` and to the declared size, its sha256 to the declared one,
//! and its magic bytes to png, jpeg, gif or webp and to the declared mime. Only then is it base64. A reference that
//! fails any of it is a `Skip`, a fixed reason the Persona is told in words, and the bytes are never in a log, an
//! error or a message.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

use crate::chat_client::ChatImage;

/// The most a stored image may weigh, declared and actual.
pub(crate) const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// The most images one line to a session carries; the rest are skipped.
pub(crate) const MAX_PER_LINE: usize = 4;

/// An image ready for a model's user message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) media_type: &'static str,
    pub(crate) base64: String,
}

/// Why an image is not shown. Fixed words: nothing the sender or the mouth wrote reaches a Persona through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skip {
    NoStore,
    BadReference,
    OutsideStore,
    Missing,
    TooLarge,
    SizeMismatch,
    HashMismatch,
    NotAnImage,
    MimeMismatch,
    TooMany,
}

impl Skip {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::NoStore => "this Persona has no image store",
            Self::BadReference => "the image reference is malformed",
            Self::OutsideStore => "the image reference leaves this room's store",
            Self::Missing => "the stored image cannot be read",
            Self::TooLarge => "the image is over 5 MiB",
            Self::SizeMismatch => "the stored image is not the declared size",
            Self::HashMismatch => "the stored image is not the declared bytes",
            Self::NotAnImage => "the stored file is not a png, jpeg, gif or webp",
            Self::MimeMismatch => "the stored image is not the declared type",
            Self::TooMany => "too many images in one line",
        }
    }
}

/// Loads `image`, which was delivered for `room_id`, from `store`.
pub(crate) fn load(store: Option<&Path>, room_id: &str, image: &ChatImage) -> Result<Image, Skip> {
    let store = store.ok_or(Skip::NoStore)?;
    if image.size > MAX_BYTES {
        return Err(Skip::TooLarge);
    }
    let relative = Path::new(&image.path);
    let room = room_dir(room_id);
    let [Component::Normal(room_part), Component::Normal(file_part)] = relative.components().collect::<Vec<_>>()[..] else {
        return Err(Skip::BadReference);
    };
    let (Some(room_part), Some(file_part)) = (room_part.to_str(), file_part.to_str()) else {
        return Err(Skip::BadReference);
    };
    if relative.as_os_str() != format!("{room_part}/{file_part}").as_str() || !is_stored_name(file_part, &image.sha256) {
        return Err(Skip::BadReference);
    }
    if room_part != room {
        return Err(Skip::OutsideStore);
    }
    let root = store.canonicalize().map_err(|_| Skip::Missing)?;
    let wanted = root.join(room_part).join(file_part);
    // Every link followed, the file must still be exactly where the reference says: a symlink anywhere on it moves it.
    if wanted.canonicalize().map_err(|_| Skip::Missing)? != wanted {
        return Err(Skip::OutsideStore);
    }
    // Checked before the open: opening a FIFO would wait for a writer.
    let meta = std::fs::symlink_metadata(&wanted).map_err(|_| Skip::Missing)?;
    if !meta.is_file() {
        return Err(Skip::Missing);
    }
    if meta.len() > MAX_BYTES {
        return Err(Skip::TooLarge);
    }
    let mut bytes = Vec::new();
    // A file that grew since the check reads as another size than the declared one.
    File::open(&wanted).map_err(|_| Skip::Missing)?.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| Skip::Missing)?;
    if bytes.len() as u64 != image.size {
        return Err(Skip::SizeMismatch);
    }
    if hex(&Sha256::digest(&bytes)) != image.sha256 {
        return Err(Skip::HashMismatch);
    }
    let media_type = sniff(&bytes).ok_or(Skip::NotAnImage)?;
    if image.mime != media_type {
        return Err(Skip::MimeMismatch);
    }
    Ok(Image { media_type, base64: STANDARD.encode(&bytes) })
}

/// The media type the first bytes say, among the four a model takes.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// The file's name: the bytes' sha256, then at most a short alphanumeric extension.
fn is_stored_name(name: &str, sha256: &str) -> bool {
    name.strip_prefix(sha256).is_some_and(|rest| rest.is_empty() || rest.strip_prefix('.').is_some_and(|ext| (1..=5).contains(&ext.len()) && ext.bytes().all(|byte| byte.is_ascii_alphanumeric())))
}

/// The name of a room's directory in the store: the hex sha256 of its Matrix room id.
pub(crate) fn room_dir(room_id: &str) -> String {
    hex(&Sha256::digest(room_id.as_bytes()))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
