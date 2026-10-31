//! The operator's brakes, and the one reader of them.
//!
//! `Released` carries a `Grant`, the only value that opens a launch: it cannot
//! be built outside this module, so a launch exists only as the reading of a
//! released instance brake and a released swarm brake. The Grant names the
//! instance whose store was read and carries nothing else: no money, no cap. The store is at a fixed place, `CONTROL_ROOT`,
//! and the one reader takes the instance only: no function, argument, variable
//! or flag lets a caller choose a root, a path or a source, so a Grant is the
//! proof that this instance's own root-owned store released. Only a
//! `#[cfg(test)]` override inside this crate moves the root.
//!
//! State is a CultCache single-file store at
//! `/etc/gamecult/minds/<instance>/control.cc`, root-owned and world-readable.
//! The operator's CLI replaces a document whole; every organ that acts calls
//! `read_effective` before the consequence and takes its answer as given. Who wakes and when is the
//! heartbeat's.
//!
//! Fail closed: the reader's only route to `Released` is a decodable instance
//! store holding a released brake and a released swarm envelope. Anything else
//! (no file, no brake, a held brake, bytes that do not decode, a store
//! holding any document but a brake, a file that cannot be read) is `Held`, with the reason.
//!
//! The reader opens the file read-only through CultCache's read-only snapshot
//! door: it takes no lock and creates no sibling file, so the units she runs as
//! need no write authority anywhere under `/etc`. Writing is the operator's;
//! the filesystem is the authority. The writer only refuses to write through an
//! existing directory, lock or store that is not root's or that group or world
//! can write (`UnsafeStore`); it checks no caller identity.
//!
//! The swarm has one more store of the same kind, `CONTROL_ROOT/swarm.cc`: a
//! file beside the instance directories, so no instance name reaches it. It
//! holds the swarm's brake and nothing else: a store still holding a swarm dial
//! or a share from the dollar-metering days reads `Held(Undecodable)` and is
//! never migrated. `read_envelope` is its one reader, fail closed like
//! `read_effective`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry, SingleFileMessagePackBackingStore};
use eureka_pipeline::Slug;

/// Where every instance's control directory lives. The reader reads only here;
/// the operator's CLI takes its own `--root` for writing and display.
pub const CONTROL_ROOT: &str = "/etc/gamecult/minds";

/// The one key each control document has in the store.
pub const BRAKE_KEY: &str = "brake";

/// The swarm's store, a file directly under `CONTROL_ROOT`.
pub const SWARM_FILE: &str = "swarm.cc";

/// `<root>/swarm.cc`.
pub fn swarm_path(root: &Path) -> PathBuf {
    root.join(SWARM_FILE)
}

/// `<root>/<instance>/control.cc`, after the leaf's slug grammar has passed, so
/// an instance name cannot walk out of the root.
pub fn control_path(root: &Path, instance: &Slug) -> Result<PathBuf> {
    instance.validate_slug().map_err(|_| anyhow::anyhow!("the instance is not a slug"))?;
    Ok(root.join(&instance.0).join("control.cc"))
}

/// The brake: released means the operator allows her to act.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.brake.v1", schema = "ControlBrake")]
pub struct Brake {
    #[cultcache(key = 0)]
    pub released: bool,
    #[cultcache(key = 1)]
    pub set_at: DateTime<Utc>,
    #[cultcache(key = 2)]
    pub set_by: String,
}

/// Why the reader said held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldReason {
    BrakeAbsent,
    BrakeHeld,
    Undecodable,
}

/// What the operator's store allows right now.
#[derive(Debug, PartialEq, Eq)]
pub enum Effective {
    Held { reason: HeldReason },
    Released(Grant),
}

/// The proof that the instance brake and the swarm brake were released when
/// read, in the root-owned stores of `instance`. Its fields are private and only
/// `read_effective` makes one; `open_and_launch` takes it by value and refuses
/// one read for another instance, so one reading opens at most one run, of the
/// instance it was read for.
///
/// Outside this crate the type system leaves no way to make one: the fields are
/// private, there is no public constructor, and the derives give no path (no
/// `Default`, `Clone`, `Deserialize` or `From`). Each way is tried here and
/// refused:
///
/// ```compile_fail,E0451
/// use mind_body::control::Grant;
/// // a struct literal
/// let _ = Grant { instance: eureka_pipeline::Slug("eureka".into()) };
/// ```
///
/// ```compile_fail,E0277
/// // not Default
/// let _: mind_body::control::Grant = Default::default();
/// ```
///
/// ```compile_fail,E0599
/// // not Clone, so a reading cannot be copied into a second run
/// fn copy(grant: &mind_body::control::Grant) -> mind_body::control::Grant {
///     grant.clone()
/// }
/// ```
///
/// ```compile_fail,E0277
/// // not Deserialize
/// fn read<T: serde::de::DeserializeOwned>() {}
/// read::<mind_body::control::Grant>();
/// ```
///
/// ```compile_fail,E0277
/// // not From its parts
/// use mind_body::control::Grant;
/// let _ = Grant::from(eureka_pipeline::Slug("eureka".into()));
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Grant {
    instance: Slug,
}

impl Grant {
    /// The instance whose control store this was read from.
    pub fn instance(&self) -> &Slug {
        &self.instance
    }

    /// A Grant for a launch test that has no store. It names its instance like
    /// any other, so the launch's instance check judges it.
    #[cfg(test)]
    pub(crate) fn for_test(instance: Slug) -> Self {
        Self { instance }
    }
}

/// The instance store's contents, decoded and not yet judged.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ControlState {
    pub brake: Option<Brake>,
}

impl ControlState {
    /// The one derivation. Order: the instance brake, then the swarm brake;
    /// each failing rung is its own reason.
    pub(crate) fn effective(&self, instance: &Slug, swarm: SwarmEffective) -> Effective {
        let held = |reason| Effective::Held { reason };
        let Some(brake) = &self.brake else {
            return held(HeldReason::BrakeAbsent);
        };
        if !brake.released {
            return held(HeldReason::BrakeHeld);
        }
        match swarm {
            SwarmEffective::Held(reason) => held(reason),
            SwarmEffective::Released => Effective::Released(Grant { instance: instance.clone() }),
        }
    }
}

/// The reader's port: the store's envelopes as one snapshot. An absent store is
/// an empty snapshot; an unreadable one is an error.
pub trait ControlSource {
    fn snapshot(&self) -> Result<Vec<CultCacheEnvelope>>;
}

/// The real source: the control file, read without locks and without writes.
pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ControlSource for FileSource {
    fn snapshot(&self) -> Result<Vec<CultCacheEnvelope>> {
        SingleFileMessagePackBackingStore::new(&self.path).pull_all_read_only_snapshot()
    }
}

fn registered_cache() -> Result<CultCache> {
    let mut cache = CultCache::new();
    cache.register_entry_type::<Brake>()?;
    Ok(cache)
}

fn swarm_cache() -> Result<CultCache> {
    let mut cache = CultCache::new();
    cache.register_entry_type::<Brake>()?;
    Ok(cache)
}

/// Decodes the instance store into its brake. Any envelope the cache does not
/// know, or any payload that does not decode, is an error: a store the reader
/// cannot fully read is not one it may act on.
pub fn load_state(source: &impl ControlSource) -> Result<ControlState> {
    let mut cache = registered_cache()?;
    for envelope in source.snapshot().context("control store cannot be read")? {
        cache.put_raw_envelope(envelope)?;
    }
    Ok(ControlState { brake: cache.get::<Brake>(BRAKE_KEY)? })
}

/// The only reader every acting organ uses (the heartbeat, the Persona organ).
/// The stores are `instance`'s own brake at `CONTROL_ROOT/<instance>/control.cc`
/// and the swarm's at `CONTROL_ROOT/swarm.cc`; the caller supplies neither a
/// root, a path nor a source. An instance that is not a slug reads as held.
///
/// Nothing outside this crate can name another place:
///
/// ```compile_fail
/// use eureka_pipeline::Slug;
/// // a root is not an argument
/// let _ = mind_body::control::read_effective(std::path::Path::new("/tmp"), &Slug("eureka".into()));
/// ```
///
/// ```compile_fail
/// // there is no reader over a source of the caller's
/// let _ = mind_body::control::read_effective_from;
/// ```
///
/// ```compile_fail
/// // and the test constructor is not in the library build
/// let _ = mind_body::control::Grant::for_test;
/// ```
///
/// The same preamble with the real call compiles:
///
/// ```
/// use eureka_pipeline::Slug;
/// let _ = mind_body::control::read_effective(&Slug("eureka".into()));
/// ```
pub fn read_effective(instance: &Slug) -> Effective {
    let root = control_root();
    match control_path(&root, instance) {
        Ok(path) => derive(&FileSource::new(path), &FileSource::new(swarm_path(&root)), instance),
        Err(_) => Effective::Held { reason: HeldReason::Undecodable },
    }
}

#[cfg(not(test))]
fn control_root() -> PathBuf {
    PathBuf::from(CONTROL_ROOT)
}

// The test seam: this crate's own tests move the root for their thread. It
// does not exist in a library build, and it is not a cargo feature.
#[cfg(test)]
thread_local! {
    static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn control_root() -> PathBuf {
    TEST_ROOT.with(|root| root.borrow().clone()).unwrap_or_else(|| PathBuf::from(CONTROL_ROOT))
}

/// Runs `body` with the reader pointed at `root` on this thread.
#[cfg(test)]
pub(crate) fn with_root<T>(root: &Path, body: impl FnOnce() -> T) -> T {
    let before = TEST_ROOT.with(|cell| cell.replace(Some(root.to_path_buf())));
    let result = body();
    TEST_ROOT.with(|cell| cell.replace(before));
    result
}

/// The derivation over any two sources, the instance's and the swarm's. The
/// reader, the organs' gates and the CLI's `show` (display only: its output is
/// text) are its callers; nothing public returns its result for a source of the
/// caller's.
pub(crate) fn derive(source: &impl ControlSource, swarm: &impl ControlSource, instance: &Slug) -> Effective {
    match load_state(source) {
        Ok(state) => state.effective(instance, derive_swarm(swarm)),
        Err(_) => Effective::Held { reason: HeldReason::Undecodable },
    }
}

#[cfg(test)]
mod tests;

/// The operator's writer over one control file. Each setter replaces its
/// document whole; the other document is untouched. The file is created on the
/// first write.
pub struct ControlWriter {
    cache: CultCache,
    path: PathBuf,
}

/// The modes the store is created with, whatever the operator's umask: the
/// directory and the files under it are world-readable and writable by their
/// owner alone, which is what makes the filesystem the write authority.
const DIR_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

#[cfg(unix)]
pub(crate) fn set_mode_world_readable(path: &Path) -> Result<()> {
    set_mode(path, FILE_MODE)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).with_context(|| format!("failed to set the mode of {}", path.display()))
}

/// A control path the writer will not write through: it is not root's, or
/// group or world can write it. Names the path, its mode and its owner.
#[derive(Debug)]
pub struct UnsafeStore {
    pub path: PathBuf,
    pub mode: u32,
    pub uid: u32,
}

impl std::fmt::Display for UnsafeStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "refusing to write: {} is mode {:04o} owned by uid {}; it must be owned by root (uid 0) and not group- or world-writable",
            self.path.display(),
            self.mode,
            self.uid
        )
    }
}

impl std::error::Error for UnsafeStore {}

#[cfg(unix)]
fn require_root_only_writable(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    let (mode, uid) = (meta.mode() & 0o7777, meta.uid());
    if uid != 0 || mode & 0o022 != 0 {
        return Err(UnsafeStore { path: path.to_path_buf(), mode, uid }.into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_root_only_writable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn set_mode_world_readable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Before the first write: the control directory exists at 0755 and the
/// lock file at 0644, set explicitly because creation takes the umask.
/// A directory, lock or store that already exists must be root's and not
/// group- or world-writable, else the write is refused (`UnsafeStore`): the
/// operator fixes the install. Called after a setter's own refusals, so a
/// refused write creates nothing.
fn prepare_store(path: &Path) -> Result<()> {
    let directory = path.parent().context("control path has no directory")?;
    let mut lock_name = path.file_name().context("control path has no file name")?.to_os_string();
    lock_name.push(".lock");
    let lock = path.with_file_name(lock_name);
    for existing in [directory, lock.as_path(), path] {
        if existing.exists() {
            require_root_only_writable(existing)?;
        }
    }
    // Every directory this write creates is 0755, ancestors included: a
    // restrictive process umask would otherwise leave /etc/gamecult/minds
    // untraversable by the units that only read.
    let missing: Vec<&Path> = directory.ancestors().take_while(|ancestor| !ancestor.exists()).collect();
    if !missing.is_empty() {
        std::fs::create_dir_all(directory).with_context(|| format!("failed to create {}", directory.display()))?;
        for created in missing {
            set_mode(created, DIR_MODE)?;
        }
    }
    if !lock.exists() {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock)
            .with_context(|| format!("failed to open {}", lock.display()))?;
        set_mode(&lock, FILE_MODE)?;
    }
    Ok(())
}

impl ControlWriter {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut cache = registered_cache()?;
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&path))?;
        cache.pull_all_backing_stores()?;
        Ok(Self { cache, path })
    }

    fn prepare(&self) -> Result<()> {
        prepare_store(&self.path)
    }

    /// After a write: the store file the atomic rename put in place takes the
    /// umask-free mode. CultCache stages with the process umask and offers no
    /// way to pass a mode, so this is where the mode can be set.
    fn settle(&self) -> Result<()> {
        set_mode(&self.path, FILE_MODE)
    }

    pub fn set_brake(&mut self, released: bool, set_at: DateTime<Utc>, set_by: &str) -> Result<()> {
        let brake = Brake { released, set_at, set_by: set_by.to_string() };
        self.prepare()?;
        self.cache.put(BRAKE_KEY, &brake)?;
        self.settle()
    }
}

// ---------------------------------------------------------------------------
// The swarm store: `CONTROL_ROOT/swarm.cc`.
// ---------------------------------------------------------------------------

/// The swarm store's contents, decoded and not yet judged.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SwarmState {
    pub brake: Option<Brake>,
}

/// What the swarm store allows right now.
#[derive(Debug, PartialEq, Eq)]
pub enum SwarmEffective {
    Held(HeldReason),
    Released,
}

impl SwarmState {
    /// The one derivation: the swarm brake, released.
    pub(crate) fn effective(&self) -> SwarmEffective {
        match &self.brake {
            None => SwarmEffective::Held(HeldReason::BrakeAbsent),
            Some(brake) if !brake.released => SwarmEffective::Held(HeldReason::BrakeHeld),
            Some(_) => SwarmEffective::Released,
        }
    }
}

/// Decodes the swarm store. An envelope the cache does not know, or a payload
/// that does not decode, is an error.
pub fn load_swarm_state(source: &impl ControlSource) -> Result<SwarmState> {
    let mut cache = swarm_cache()?;
    for envelope in source.snapshot().context("swarm store cannot be read")? {
        cache.put_raw_envelope(envelope)?;
    }
    Ok(SwarmState { brake: cache.get::<Brake>(BRAKE_KEY)? })
}

/// The one reader of the swarm store, at `CONTROL_ROOT/swarm.cc`; the caller
/// supplies neither a root, a path nor a source. Anything but a decodable store
/// holding a released brake is `Held`, with the reason.
pub fn read_envelope() -> SwarmEffective {
    derive_swarm(&FileSource::new(swarm_path(&control_root())))
}

/// The derivation over any source; the CLI's `show` is its other caller.
pub(crate) fn derive_swarm(source: &impl ControlSource) -> SwarmEffective {
    match load_swarm_state(source) {
        Ok(state) => state.effective(),
        Err(_) => SwarmEffective::Held(HeldReason::Undecodable),
    }
}

/// The operator's writer over the swarm file, with the same root-only refusals
/// as `ControlWriter`.
pub struct SwarmWriter {
    cache: CultCache,
    path: PathBuf,
}

impl SwarmWriter {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut cache = swarm_cache()?;
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&path))?;
        cache.pull_all_backing_stores()?;
        Ok(Self { cache, path })
    }

    fn settle(&self) -> Result<()> {
        set_mode(&self.path, FILE_MODE)
    }

    pub fn set_brake(&mut self, released: bool, set_at: DateTime<Utc>, set_by: &str) -> Result<()> {
        let brake = Brake { released, set_at, set_by: set_by.to_string() };
        prepare_store(&self.path)?;
        self.cache.put(BRAKE_KEY, &brake)?;
        self.settle()
    }
}
