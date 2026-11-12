//! The saver against a real store and a real sandbox on disk: each guard against the one input it exists for.

use std::cell::Cell;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::*;
use crate::chat_image::{hex, room_dir};

const ROOM: &str = "!room:gamecult.test";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot a real picture, only its magic bytes and a tail";

thread_local! {
    static CRASH_AT: Cell<Option<usize>> = const { Cell::new(None) };
    static CRASHED: Cell<bool> = const { Cell::new(false) };
}

/// The write loop's seam: true once `written` bytes have gone out and a crash is armed on this thread.
pub(super) fn crash_after(written: usize) -> bool {
    let fire = CRASH_AT.get().is_some_and(|at| written >= at);
    if fire {
        CRASHED.set(true);
    }
    fire
}

thread_local! {
    static GAP: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
}

/// The seam between the unlink of the temporary's name and its create: where her shell could plant something at that name.
pub(super) fn between_unlink_and_create() {
    GAP.with(|gap| {
        if let Some(plant) = gap.borrow().as_ref() {
            plant();
        }
    });
}

pub(super) fn crashed() -> bool {
    CRASHED.get()
}

struct World {
    store: tempfile::TempDir,
    sandbox: tempfile::TempDir,
}

impl World {
    fn new() -> Self {
        Self { store: tempfile::tempdir().unwrap(), sandbox: tempfile::tempdir().unwrap() }
    }

    /// Stores `bytes` the way the mouth does under `room`, named by `file_ext`, and describes them honestly.
    fn put(&self, room: &str, bytes: &[u8], file_ext: &str) -> ChatImage {
        let sha256 = hex(&Sha256::digest(bytes));
        let room = room_dir(room);
        std::fs::create_dir_all(self.store.path().join(&room)).unwrap();
        let name = if file_ext.is_empty() { sha256.clone() } else { format!("{sha256}.{file_ext}") };
        std::fs::write(self.store.path().join(&room).join(&name), bytes).unwrap();
        let mime = match bytes {
            [0xff, 0xd8, ..] => "image/jpeg",
            [b'G', b'I', b'F', ..] => "image/gif",
            [b'R', b'I', b'F', b'F', ..] => "image/webp",
            _ => "image/png",
        };
        ChatImage { path: format!("{room}/{name}"), mime: mime.into(), size: bytes.len() as u64, sha256 }
    }

    fn save(&self, image: &ChatImage) -> Result<PathBuf, SaveFault> {
        save(self.store.path(), self.sandbox.path(), ROOM, image)
    }

    fn attachments(&self) -> PathBuf {
        self.sandbox.path().join("attachments")
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.attachments()).map(|dir| dir.map(|e| e.unwrap().file_name().into_string().unwrap()).collect()).unwrap_or_default();
        names.sort();
        names
    }
}

#[test]
fn the_saved_name_is_the_hash_and_one_of_four_extensions() {
    let world = World::new();
    for (bytes, ext) in [(PNG, "png"), (&b"\xff\xd8\xff\xe0 jpeg tail"[..], "jpg"), (&b"GIF89a gif tail"[..], "gif"), (&b"RIFF\x10\0\0\0WEBPVP8 tail"[..], "webp")] {
        // The stored file's own extension is the mouth's and is not used.
        let image = world.put(ROOM, bytes, "exe");
        let path = world.save(&image).unwrap();
        assert_eq!(path, Path::new("attachments").join(format!("{}.{ext}", image.sha256)));
        assert_eq!(std::fs::read(world.sandbox.path().join(&path)).unwrap(), bytes);
    }
    assert_eq!(world.files().len(), 4, "{:?}", world.files());
}

#[test]
fn a_receipt_path_that_leaves_the_store_or_names_no_stored_file_is_refused_and_writes_nothing() {
    let world = World::new();
    let good = world.put(ROOM, PNG, "png");
    for path in [format!("../{}", good.path), format!("/{}", good.path), good.path.replace('/', "/../"), format!("{}/x", good.path), String::from("x")] {
        let image = ChatImage { path, ..good.clone() };
        assert!(matches!(world.save(&image), Err(SaveFault::Load(Skip::BadReference | Skip::OutsideStore))), "{image:?}");
    }
    assert!(!world.attachments().exists(), "a refused reference leaves not even the directory");
}

#[test]
fn the_saver_reverifies_what_the_mouth_stored() {
    let world = World::new();
    let image = world.put(ROOM, PNG, "png");
    let stored = world.store.path().join(&image.path);
    // Wrong bytes under the right name.
    std::fs::write(&stored, b"\x89PNG\r\n\x1a\nsome other picture, same magic bytes, same-ish size.....").unwrap();
    assert!(matches!(world.save(&image), Err(SaveFault::Load(Skip::SizeMismatch | Skip::HashMismatch))));
    // Not an image at all, honestly hashed.
    let html = world.put(ROOM, b"<html>not an image</html>", "png");
    assert_eq!(world.save(&html), Err(SaveFault::Load(Skip::NotAnImage)));
    // A declared type that is not the sniffed one.
    let image = ChatImage { mime: "image/gif".into(), ..world.put(ROOM, PNG, "png") };
    assert_eq!(world.save(&image), Err(SaveFault::Load(Skip::MimeMismatch)));
    // Evicted by the mouth.
    let image = world.put(ROOM, PNG, "png");
    std::fs::remove_file(world.store.path().join(&image.path)).unwrap();
    assert_eq!(world.save(&image), Err(SaveFault::Load(Skip::Missing)));
    assert!(world.files().is_empty());
}

#[test]
fn a_receipt_path_under_another_rooms_hash_is_not_copied() {
    let world = World::new();
    let image = world.put("!other:gamecult.test", PNG, "png");
    // The room id the mouth resolved is ROOM, but the path sits under another room's hash.
    assert_eq!(world.save(&image), Err(SaveFault::Load(Skip::OutsideStore)));
    // The alias is not a room id: loading with it reaches no store.
    let own = world.put(ROOM, PNG, "png");
    assert_eq!(save(world.store.path(), world.sandbox.path(), "#room:gamecult.test", &own), Err(SaveFault::Load(Skip::OutsideStore)));
    assert!(world.files().is_empty());
}

#[test]
fn every_skip_the_verifier_returns_is_a_save_fault_load() {
    let world = World::new();
    let big = world.put(ROOM, PNG, "png");
    assert_eq!(world.save(&ChatImage { size: 6 * 1024 * 1024, ..big.clone() }), Err(SaveFault::Load(Skip::TooLarge)));
    assert_eq!(world.save(&ChatImage { size: 3, ..big }), Err(SaveFault::Load(Skip::SizeMismatch)));
}

#[test]
fn the_saver_refuses_a_git_work_tree() {
    let at_root = World::new();
    std::fs::create_dir(at_root.sandbox.path().join(".git")).unwrap();
    assert_eq!(at_root.save(&at_root.put(ROOM, PNG, "png")), Err(SaveFault::RepoTree));
    assert!(!at_root.attachments().exists());

    let inside = World::new();
    std::fs::create_dir_all(inside.attachments().join(".git")).unwrap();
    assert_eq!(inside.save(&inside.put(ROOM, PNG, "png")), Err(SaveFault::RepoTree));
    assert_eq!(inside.files(), [".git"]);

    // A .git file (a worktree or submodule) is a work tree too, and a link named .git counts without being followed.
    let linked = World::new();
    symlink("/nonexistent", linked.sandbox.path().join(".git")).unwrap();
    assert_eq!(linked.save(&linked.put(ROOM, PNG, "png")), Err(SaveFault::RepoTree));
}

#[test]
fn a_symlinked_attachments_directory_or_sandbox_is_outside() {
    let world = World::new();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), world.attachments()).unwrap();
    assert_eq!(world.save(&world.put(ROOM, PNG, "png")), Err(SaveFault::OutsideSandbox));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);

    let file = World::new();
    std::fs::write(file.attachments(), b"a file where the directory goes").unwrap();
    assert_eq!(file.save(&file.put(ROOM, PNG, "png")), Err(SaveFault::OutsideSandbox));

    let linked_root = World::new();
    let link = linked_root.sandbox.path().join("link");
    symlink(outside.path(), &link).unwrap();
    let image = linked_root.put(ROOM, PNG, "png");
    assert_eq!(save(linked_root.store.path(), &link, ROOM, &image), Err(SaveFault::OutsideSandbox));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn the_saver_cannot_be_walked_out_by_a_swapping_sandbox() {
    let world = World::new();
    let outside = tempfile::tempdir().unwrap();
    let image = world.put(ROOM, PNG, "png");
    let attachments = world.attachments();
    let parked = world.sandbox.path().join("parked");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let swapper = {
        let (stop, outside) = (stop.clone(), outside.path().to_path_buf());
        let (attachments, parked) = (attachments.clone(), parked.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                // Between a real directory and a link to the outside, as her shell could.
                let _ = std::fs::rename(&attachments, &parked);
                let _ = symlink(&outside, &attachments);
                let _ = std::fs::remove_file(&attachments);
                let _ = std::fs::rename(&parked, &attachments);
            }
        })
    };
    for _ in 0..200 {
        match world.save(&image) {
            Ok(_) | Err(SaveFault::OutsideSandbox | SaveFault::WriteFailed | SaveFault::Exists) => {}
            Err(other) => panic!("{other:?}"),
        }
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    swapper.join().unwrap();
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0, "nothing reached the outside directory");
}

#[test]
fn an_identical_existing_file_counts_as_saved_and_leaves_no_part() {
    let world = World::new();
    let image = world.put(ROOM, PNG, "png");
    let path = world.save(&image).unwrap();
    assert_eq!(world.save(&image), Ok(path.clone()), "a replayed request is not an error");
    assert_eq!(world.files(), [path.file_name().unwrap().to_str().unwrap()]);

    // Same length, other bytes under the name: Exists, and the file is left as it was.
    let held = world.sandbox.path().join(&path);
    let mut other = PNG.to_vec();
    *other.last_mut().unwrap() ^= 1;
    std::fs::write(&held, &other).unwrap();
    assert_eq!(world.save(&image), Err(SaveFault::Exists));
    assert_eq!(std::fs::read(&held).unwrap(), other);
    // Another length.
    std::fs::write(&held, b"short").unwrap();
    assert_eq!(world.save(&image), Err(SaveFault::Exists));
    assert_eq!(world.files().len(), 1, "no .part left behind");
}

/// Runs `save` on a thread and fails the test if it does not return: a FIFO opened for reading would wait forever.
fn save_within(world: &World, image: &ChatImage) -> Result<PathBuf, SaveFault> {
    let (store, sandbox, image) = (world.store.path().to_path_buf(), world.sandbox.path().to_path_buf(), image.clone());
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(save(&store, &sandbox, ROOM, &image));
    });
    receive.recv_timeout(Duration::from_secs(10)).expect("the saver hung")
}

#[test]
fn a_fifo_or_odd_file_at_the_final_name_does_not_block_the_organ() {
    use rustix::fs::{CWD, FileType, Mode, mknodat};
    let world = World::new();
    let image = world.put(ROOM, PNG, "png");
    let held = world.attachments().join(format!("{}.png", image.sha256));
    std::fs::create_dir(world.attachments()).unwrap();

    mknodat(CWD, &held, FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
    assert_eq!(save_within(&world, &image), Err(SaveFault::Exists));
    std::fs::remove_file(&held).unwrap();

    symlink("/dev/zero", &held).unwrap();
    assert_eq!(save_within(&world, &image), Err(SaveFault::Exists));
    std::fs::remove_file(&held).unwrap();

    std::fs::create_dir(&held).unwrap();
    assert_eq!(save_within(&world, &image), Err(SaveFault::Exists));
    std::fs::remove_dir(&held).unwrap();

    // A regular file of another length is not read at all; of the same length, only that many bytes plus one.
    std::fs::write(&held, vec![0u8; 5 * 1024 * 1024]).unwrap();
    assert_eq!(save_within(&world, &image), Err(SaveFault::Exists));
    std::fs::write(&held, vec![0u8; PNG.len()]).unwrap();
    assert_eq!(save_within(&world, &image), Err(SaveFault::Exists));
}

#[test]
fn a_crashed_write_leaves_no_partial_final_name_and_a_retry_saves_in_full() {
    use rustix::fs::{CWD, FileType, Mode, mknodat};
    let world = World::new();
    let big: Vec<u8> = b"\x89PNG\r\n\x1a\n".iter().copied().chain((0..20_000u32).map(|i| (i % 251) as u8)).collect();
    let image = world.put(ROOM, &big, "png");
    let final_name = format!("{}.png", image.sha256);
    let part = format!(".{final_name}.part");

    CRASH_AT.set(Some(8192));
    assert_eq!(world.save(&image), Err(SaveFault::WriteFailed));
    CRASH_AT.set(None);
    assert_eq!(world.files(), [part.as_str()], "only the temporary is left, and the final name is absent");
    assert!(std::fs::metadata(world.attachments().join(&part)).unwrap().len() < big.len() as u64);

    // The retry clears the leavings and saves in full.
    assert!(world.save(&image).is_ok());
    assert_eq!(world.files(), [final_name.as_str()]);
    assert_eq!(std::fs::read(world.attachments().join(&final_name)).unwrap(), big);

    // A FIFO or a link at the temporary name does not stop the next save either.
    std::fs::remove_file(world.attachments().join(&final_name)).unwrap();
    mknodat(CWD, world.attachments().join(&part), FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
    assert!(save_within(&world, &image).is_ok());
    symlink("/dev/zero", world.attachments().join(&part)).unwrap();
    std::fs::remove_file(world.attachments().join(&final_name)).unwrap();
    assert!(save_within(&world, &image).is_ok());
    assert_eq!(world.files(), [final_name]);
}

/// The create call carries 0600, so the runner's umask can only narrow it: a looser mode in that call shows as 0644 or 0666 under the
/// common umask 022, and a tighter one as less than 0600. (The library reads no environment and forbids unsafe, so the test does not
/// set a umask; the source has no chmod, which the negative scan pins.)
#[test]
fn the_saved_file_is_mode_0600_and_the_directory_0700() {
    let world = World::new();
    let image = world.put(ROOM, PNG, "png");
    let path = world.save(&image).unwrap();
    assert_eq!(std::fs::metadata(world.sandbox.path().join(path)).unwrap().permissions().mode() & 0o7777, 0o600);
    assert_eq!(std::fs::metadata(world.attachments()).unwrap().permissions().mode() & 0o7777, 0o700);
}

#[test]
fn a_file_or_link_planted_at_the_temporary_name_after_its_unlink_is_refused_and_never_written_through() {
    let world = World::new();
    let image = world.put(ROOM, PNG, "png");
    let victim = world.sandbox.path().join("victim");
    let part = world.attachments().join(format!(".{}.png.part", image.sha256));
    for plant_a_link in [false, true] {
        std::fs::write(&victim, b"keep me").unwrap();
        let (victim_path, part_path) = (victim.clone(), part.clone());
        let plant: Box<dyn Fn()> = Box::new(move || {
            if plant_a_link {
                symlink(&victim_path, &part_path).unwrap();
            } else {
                // A regular file of her own: an create without O_EXCL would open it and truncate.
                std::fs::write(&part_path, b"hers").unwrap();
            }
        });
        GAP.with(|gap| *gap.borrow_mut() = Some(plant));
        let saved = world.save(&image);
        GAP.with(|gap| *gap.borrow_mut() = None);
        assert_eq!(saved, Err(SaveFault::WriteFailed), "link: {plant_a_link}");
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep me", "nothing was written through the planted name (link: {plant_a_link})");
        if !plant_a_link {
            assert_eq!(std::fs::read(&part).unwrap(), b"hers", "her file is not truncated");
        }
        assert!(!world.attachments().join(format!("{}.png", image.sha256)).exists(), "no final name from a refused create");
        let _ = std::fs::remove_file(&part);
    }
}

#[test]
fn a_link_to_a_same_length_regular_file_at_the_final_name_is_exists_not_saved() {
    let world = World::new();
    let image = world.put(ROOM, PNG, "png");
    let outside = world.sandbox.path().join("outside.bin");
    std::fs::write(&outside, PNG).unwrap();
    std::fs::create_dir(world.attachments()).unwrap();
    symlink(&outside, world.attachments().join(format!("{}.png", image.sha256))).unwrap();
    assert_eq!(world.save(&image), Err(SaveFault::Exists), "a link is not a file in her sandbox, whatever it points at");
}
