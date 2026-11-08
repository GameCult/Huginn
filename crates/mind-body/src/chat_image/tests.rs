//! The image guards, each against the one input it exists for. Every test builds a real store on disk; a guard
//! removed from `load` lets exactly its test's input through.

use std::os::unix::fs::symlink;

use super::*;

const ROOM: &str = "!room:gamecult.test";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot a real picture, only its magic bytes and a tail";

struct Store {
    dir: tempfile::TempDir,
}

impl Store {
    fn new() -> Self {
        Self { dir: tempfile::tempdir().unwrap() }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Stores `bytes` the way the mouth does, in `room`'s directory, and names them honestly.
    fn put(&self, room: &str, bytes: &[u8]) -> ChatImage {
        let sha256 = hex(&Sha256::digest(bytes));
        let room = room_dir(room);
        std::fs::create_dir_all(self.root().join(&room)).unwrap();
        std::fs::write(self.root().join(&room).join(&sha256), bytes).unwrap();
        ChatImage { path: format!("{room}/{sha256}"), mime: sniff(bytes).unwrap_or("image/png").into(), size: bytes.len() as u64, sha256 }
    }

    fn load(&self, image: &ChatImage) -> Result<Image, Skip> {
        load(Some(self.root()), ROOM, image)
    }
}

#[test]
fn a_stored_image_loads_as_its_exact_bytes_in_base64() {
    let store = Store::new();
    let image = store.load(&store.put(ROOM, PNG)).unwrap();
    assert_eq!(image.media_type, "image/png");
    assert_eq!(STANDARD.decode(&image.base64).unwrap(), PNG);
}

#[test]
fn the_four_formats_load_as_their_own_media_type() {
    let store = Store::new();
    for (bytes, expected) in [
        (&b"\xff\xd8\xff\xe0 jpeg tail"[..], "image/jpeg"),
        (&b"GIF89a gif tail"[..], "image/gif"),
        (&b"GIF87a gif tail"[..], "image/gif"),
        (&b"RIFF\x10\0\0\0WEBPVP8 tail"[..], "image/webp"),
    ] {
        assert_eq!(store.load(&store.put(ROOM, bytes)).unwrap().media_type, expected);
    }
}

#[test]
fn a_file_without_an_images_magic_bytes_is_refused_whatever_it_claims() {
    let store = Store::new();
    let mut image = store.put(ROOM, b"<html><script>alert(1)</script></html>");
    image.mime = "image/png".into();
    assert_eq!(store.load(&image), Err(Skip::NotAnImage));
    // RIFF without WEBP is a wav, not an image.
    assert_eq!(store.load(&store.put(ROOM, b"RIFF\x10\0\0\0WAVEfmt ")), Err(Skip::NotAnImage));
}

#[test]
fn a_declared_type_that_is_not_the_sniffed_one_is_refused() {
    let store = Store::new();
    let mut image = store.put(ROOM, PNG);
    image.mime = "image/jpeg".into();
    assert_eq!(store.load(&image), Err(Skip::MimeMismatch));
}

#[test]
fn an_absolute_path_is_refused_even_when_it_names_the_real_file() {
    let store = Store::new();
    let mut image = store.put(ROOM, PNG);
    image.path = store.root().join(&image.path).to_string_lossy().into_owned();
    assert_eq!(store.load(&image), Err(Skip::BadReference));
}

#[test]
fn traversal_is_refused_and_a_file_outside_the_store_is_never_read() {
    let store = Store::new();
    let outside = tempfile::tempdir().unwrap();
    let sha256 = hex(&Sha256::digest(PNG));
    std::fs::write(outside.path().join(&sha256), PNG).unwrap();
    let real = store.put(ROOM, PNG);
    let room = room_dir(ROOM);
    let up = format!("../{}", outside.path().file_name().unwrap().to_string_lossy());
    for path in [
        format!("{room}/../{room}/{sha256}"),
        format!("{room}/./{sha256}"),
        format!("../{room}/{sha256}"),
        format!("{up}/{sha256}"),
        format!("{room}/{up}/{sha256}"),
        format!("{room}/{sha256}/"),
        format!("{room}//{sha256}"),
        format!("{room}/sub/{sha256}"),
        format!("{room}\\{sha256}"),
        format!("{room}/{sha256}\0"),
        sha256.clone(),
        String::new(),
    ] {
        assert_eq!(store.load(&ChatImage { path: path.clone(), ..real.clone() }), Err(Skip::BadReference), "{path:?}");
    }
}

#[test]
fn another_rooms_image_is_not_this_rooms() {
    let store = Store::new();
    let image = store.put("!other:gamecult.test", PNG);
    assert_eq!(store.load(&image), Err(Skip::OutsideStore));
}

#[test]
fn a_symlink_on_the_way_is_refused_though_its_target_is_a_good_image() {
    let store = Store::new();
    let elsewhere = tempfile::tempdir().unwrap();
    let sha256 = hex(&Sha256::digest(PNG));
    std::fs::write(elsewhere.path().join("target"), PNG).unwrap();
    let room = room_dir(ROOM);
    std::fs::create_dir_all(store.root().join(&room)).unwrap();
    symlink(elsewhere.path().join("target"), store.root().join(&room).join(&sha256)).unwrap();
    let image = ChatImage { path: format!("{room}/{sha256}"), mime: "image/png".into(), size: PNG.len() as u64, sha256: sha256.clone() };
    assert_eq!(store.load(&image), Err(Skip::OutsideStore), "a symlinked file");

    let linked = Store::new();
    symlink(elsewhere.path(), linked.root().join(&room)).unwrap();
    std::fs::write(elsewhere.path().join(&sha256), PNG).unwrap();
    assert_eq!(linked.load(&image), Err(Skip::OutsideStore), "a symlinked room directory");
}

#[test]
fn a_name_that_is_not_the_declared_hash_is_refused() {
    let store = Store::new();
    let real = store.put(ROOM, PNG);
    let other = "0".repeat(64);
    for name in [other.clone(), format!("{}.png.exe", real.sha256), format!("{}.", real.sha256), format!("{}.toolong", real.sha256), format!("x{}", real.sha256)] {
        let image = ChatImage { path: format!("{}/{name}", room_dir(ROOM)), ..real.clone() };
        assert_eq!(store.load(&image), Err(Skip::BadReference), "{name}");
    }
    for sha in ["short", &real.sha256.to_uppercase()] {
        assert_eq!(store.load(&ChatImage { sha256: sha.into(), ..real.clone() }), Err(Skip::BadReference), "{sha}");
    }
}

#[test]
fn an_extension_is_allowed_after_the_hash() {
    let store = Store::new();
    let real = store.put(ROOM, PNG);
    let named = format!("{}.png", real.sha256);
    std::fs::rename(store.root().join(&real.path), store.root().join(room_dir(ROOM)).join(&named)).unwrap();
    assert!(store.load(&ChatImage { path: format!("{}/{named}", room_dir(ROOM)), ..real }).is_ok());
}

#[test]
fn over_five_mib_is_refused_declared_or_actual_and_before_it_is_read() {
    let store = Store::new();
    let mut big = PNG.to_vec();
    big.resize(MAX_BYTES as usize + 1, 0);
    let honest = store.put(ROOM, &big);
    assert_eq!(store.load(&honest), Err(Skip::TooLarge), "a file over the cap, honestly declared");

    let small = store.put(ROOM, PNG);
    assert_eq!(store.load(&ChatImage { size: MAX_BYTES + 1, ..small.clone() }), Err(Skip::TooLarge), "a declaration over the cap");

    let lying = ChatImage { size: 10, ..honest };
    assert_eq!(store.load(&lying), Err(Skip::TooLarge), "an oversize file declared small is refused on its length, unread");

    let mut exact = PNG.to_vec();
    exact.resize(MAX_BYTES as usize, 0);
    assert!(store.load(&store.put(ROOM, &exact)).is_ok(), "exactly the cap passes");
}

#[test]
fn the_declared_size_and_hash_must_be_the_files() {
    let store = Store::new();
    let image = store.put(ROOM, PNG);
    assert_eq!(store.load(&ChatImage { size: image.size + 1, ..image.clone() }), Err(Skip::SizeMismatch));
    // Same length, other bytes: swapped after the mouth stored it.
    let mut swapped = PNG.to_vec();
    *swapped.last_mut().unwrap() ^= 1;
    std::fs::write(store.root().join(&image.path), &swapped).unwrap();
    assert_eq!(store.load(&image), Err(Skip::HashMismatch));
}

#[test]
fn no_store_and_no_file_are_skips_not_panics() {
    let store = Store::new();
    let image = store.put(ROOM, PNG);
    assert_eq!(load(None, ROOM, &image), Err(Skip::NoStore));
    std::fs::remove_file(store.root().join(&image.path)).unwrap();
    assert_eq!(store.load(&image), Err(Skip::Missing));
    assert_eq!(load(Some(&store.root().join("absent")), ROOM, &image), Err(Skip::Missing));
}

#[test]
fn a_directory_where_the_file_should_be_is_refused() {
    let store = Store::new();
    let sha256 = hex(&Sha256::digest(PNG));
    let room = room_dir(ROOM);
    std::fs::create_dir_all(store.root().join(&room).join(&sha256)).unwrap();
    let image = ChatImage { path: format!("{room}/{sha256}"), mime: "image/png".into(), size: PNG.len() as u64, sha256 };
    assert_eq!(store.load(&image), Err(Skip::Missing));
}

#[test]
fn a_fifo_where_the_file_should_be_is_refused_without_opening_it() {
    let store = Store::new();
    let sha256 = hex(&Sha256::digest(PNG));
    let room = room_dir(ROOM);
    std::fs::create_dir_all(store.root().join(&room)).unwrap();
    assert!(std::process::Command::new("mkfifo").arg(store.root().join(&room).join(&sha256)).status().unwrap().success());
    let image = ChatImage { path: format!("{room}/{sha256}"), mime: "image/png".into(), size: PNG.len() as u64, sha256 };
    let (tx, rx) = std::sync::mpsc::channel();
    let root = store.root().to_path_buf();
    std::thread::spawn(move || tx.send(load(Some(&root), ROOM, &image)).ok());
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).expect("load returned without waiting on the FIFO"), Err(Skip::Missing));
}
