//! The operator's brake and burn-rate dial, and the one reader of them.
//!
//! `Released` carries a `Grant`, the only value that hands a run cap to the
//! launch: it cannot be built outside this module, so a cap exists only as the
//! reading of a released brake and an in-bounds dial. The Grant names the
//! instance whose store was read. The store is at a fixed place, `CONTROL_ROOT`,
//! and the one reader takes the instance only: no function, argument, variable
//! or flag lets a caller choose a root, a path or a source, so a Grant is the
//! proof that this instance's own root-owned store released. Only a
//! `#[cfg(test)]` override inside this crate moves the root.
//!
//! State is a CultCache single-file store at
//! `/etc/gamecult/minds/<instance>/control.cc`, root-owned and world-readable.
//! The operator's CLI replaces a document whole; every organ that acts calls
//! `read_effective` before the consequence and takes its answer as given. The
//! cadence and the run cap are derived here and nowhere else.
//!
//! Fail closed: the reader's only route to `Released` is a decodable store
//! holding a released brake and an in-bounds dial. Anything else (no file, no
//! brake, a held brake, no dial, a dial out of bounds, bytes that do not
//! decode, a file that cannot be read) is `Held`, with the reason.
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
//! holds the swarm's brake, the swarm dial (heat, the spend rate, the per-run
//! cap and the concurrency) and each instance's share. `read_envelope` is its
//! one reader, fail closed like `read_effective`; `Envelope::grant_for` is the
//! only way to narrow the envelope into a `Grant`, and it never widens one.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry, SingleFileMessagePackBackingStore};
use eureka_pipeline::Slug;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Where every instance's control directory lives. The reader reads only here;
/// the operator's CLI takes its own `--root` for writing and display.
pub const CONTROL_ROOT: &str = "/etc/gamecult/minds";

/// The one key each control document has in the store.
pub const BRAKE_KEY: &str = "brake";
pub const DIAL_KEY: &str = "dial";

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

/// The burn-rate dial: `heat` scales how active she is; the base values say
/// what heat 1.0 means. Cadence and run cap are derived from it, never stored.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.burn_rate.v1", schema = "ControlBurnRate")]
pub struct BurnRate {
    #[cultcache(key = 0)]
    pub heat: Decimal,
    #[cultcache(key = 1)]
    pub base_cooldown_s: u32,
    #[cultcache(key = 2)]
    pub base_run_usd: Decimal,
    #[cultcache(key = 3)]
    pub set_at: DateTime<Utc>,
    #[cultcache(key = 4)]
    pub set_by: String,
}

/// Heat's inclusive bounds.
pub fn heat_min() -> Decimal {
    Decimal::new(5, 2)
}
pub fn heat_max() -> Decimal {
    Decimal::new(20, 1)
}

impl BurnRate {
    /// The one bounds rule: the reader holds on it and the CLI refuses on it.
    pub fn in_bounds(&self) -> bool {
        self.heat >= heat_min() && self.heat <= heat_max() && self.base_cooldown_s > 0 && self.base_run_usd > Decimal::ZERO
    }

    /// Wake cadence: `base_cooldown_s / heat`, to the nanosecond.
    fn cadence(&self) -> Option<Duration> {
        let seconds = Decimal::from(self.base_cooldown_s).checked_div(self.heat)?;
        let whole = seconds.trunc();
        let nanos = ((seconds - whole) * Decimal::from(1_000_000_000u32)).round();
        Some(Duration::new(whole.to_u64()?, nanos.to_u32()?))
    }

    /// Per-run cap: `base_run_usd x heat`.
    fn run_cap_usd(&self) -> Option<Decimal> {
        self.base_run_usd.checked_mul(self.heat)
    }
}

/// Why the reader said held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldReason {
    BrakeAbsent,
    BrakeHeld,
    DialAbsent,
    DialOutOfBounds,
    /// The swarm dial's heat is 0: the operator stopped everything.
    HeatZero,
    /// A share in the swarm store is out of bounds or not keyed by its instance.
    ShareOutOfBounds,
    Undecodable,
}

/// What the operator's store allows right now.
#[derive(Debug, PartialEq, Eq)]
pub enum Effective {
    Held { reason: HeldReason },
    Released(Grant),
}

/// The proof that the brake was released and the dial in bounds when read, in
/// the root-owned store of `instance`. Its fields are private and only
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
/// let _ = Grant { instance: eureka_pipeline::Slug("eureka".into()), cadence: std::time::Duration::ZERO, run_cap_usd: rust_decimal::Decimal::ONE };
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
/// let _ = Grant::from((eureka_pipeline::Slug("eureka".into()), std::time::Duration::ZERO, rust_decimal::Decimal::ONE));
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Grant {
    instance: Slug,
    cadence: Duration,
    run_cap_usd: Decimal,
}

impl Grant {
    /// The instance whose control store this was read from.
    pub fn instance(&self) -> &Slug {
        &self.instance
    }

    /// How often she may wake.
    pub fn cadence(&self) -> Duration {
        self.cadence
    }

    /// The most one run may spend.
    pub fn run_cap_usd(&self) -> Decimal {
        self.run_cap_usd
    }

    /// A Grant for a launch test that has no store. It names its instance like
    /// any other, so the launch's instance check judges it.
    #[cfg(test)]
    pub(crate) fn for_test(instance: Slug, cadence: Duration, run_cap_usd: Decimal) -> Self {
        Self { instance, cadence, run_cap_usd }
    }
}

/// The store's contents, decoded and not yet judged.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ControlState {
    pub brake: Option<Brake>,
    pub dial: Option<BurnRate>,
}

impl ControlState {
    /// The one derivation. Order: brake first, then dial; each failing rung is
    /// its own reason.
    pub(crate) fn effective(&self, instance: &Slug) -> Effective {
        let held = |reason| Effective::Held { reason };
        let Some(brake) = &self.brake else {
            return held(HeldReason::BrakeAbsent);
        };
        if !brake.released {
            return held(HeldReason::BrakeHeld);
        }
        let Some(dial) = &self.dial else {
            return held(HeldReason::DialAbsent);
        };
        if !dial.in_bounds() {
            return held(HeldReason::DialOutOfBounds);
        }
        match (dial.cadence(), dial.run_cap_usd()) {
            (Some(cadence), Some(run_cap_usd)) => Effective::Released(Grant { instance: instance.clone(), cadence, run_cap_usd }),
            _ => held(HeldReason::DialOutOfBounds),
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
    cache.register_entry_type::<BurnRate>()?;
    Ok(cache)
}

fn swarm_cache() -> Result<CultCache> {
    let mut cache = CultCache::new();
    cache.register_entry_type::<Brake>()?;
    cache.register_entry_type::<SwarmDial>()?;
    cache.register_entry_type::<Share>()?;
    Ok(cache)
}

/// Decodes the store into its two documents. Any envelope the cache does not
/// know, or any payload that does not decode, is an error: a store the reader
/// cannot fully read is not one it may act on.
pub fn load_state(source: &impl ControlSource) -> Result<ControlState> {
    let mut cache = registered_cache()?;
    for envelope in source.snapshot().context("control store cannot be read")? {
        cache.put_raw_envelope(envelope)?;
    }
    Ok(ControlState { brake: cache.get::<Brake>(BRAKE_KEY)?, dial: cache.get::<BurnRate>(DIAL_KEY)? })
}

/// The only reader every organ uses (the waker now; the permit issuer and the
/// Persona organ later). The store is `instance`'s own at
/// `CONTROL_ROOT/<instance>/control.cc`; the caller supplies neither a root, a
/// path nor a source. An instance that is not a slug reads as held.
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
    match control_path(&control_root(), instance) {
        Ok(path) => derive(&FileSource::new(path), instance),
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

/// The derivation over any source. The reader and the CLI's `show` (display
/// only: its output is text) are its callers; nothing public returns its result
/// for a source of the caller's.
pub(crate) fn derive(source: &impl ControlSource, instance: &Slug) -> Effective {
    match load_state(source) {
        Ok(state) => state.effective(instance),
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

    /// Refuses an out-of-bounds dial before anything is written, so the store
    /// never holds one that the CLI put there.
    pub fn set_dial(&mut self, dial: BurnRate) -> Result<()> {
        if !dial.in_bounds() {
            anyhow::bail!("dial refused: heat must be within {}..={}, base_cooldown_s above 0 and base_run_usd above 0", heat_min(), heat_max());
        }
        self.prepare()?;
        self.cache.put(DIAL_KEY, &dial)?;
        self.settle()
    }
}

// ---------------------------------------------------------------------------
// The swarm store: `CONTROL_ROOT/swarm.cc`.
// ---------------------------------------------------------------------------

/// The swarm dial: `heat` scales the whole swarm's activity, 0 stops it. The
/// base values say what heat 1.0 means: the total spend rate, the most one run
/// may spend, and `k`, how many turns may run at once. The envelope derives
/// rate and run cap from it, never stored.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.swarm_dial.v1", schema = "ControlSwarmDial")]
pub struct SwarmDial {
    #[cultcache(key = 0)]
    pub heat: Decimal,
    #[cultcache(key = 1)]
    pub base_rate_usd_per_h: Decimal,
    #[cultcache(key = 2)]
    pub base_run_usd: Decimal,
    #[cultcache(key = 3)]
    pub k: u32,
    #[cultcache(key = 4)]
    pub set_at: DateTime<Utc>,
    #[cultcache(key = 5)]
    pub set_by: String,
}

impl SwarmDial {
    /// The one bounds rule for the swarm dial: the reader holds on it and the
    /// CLI refuses on it. Heat 0 is in bounds (it is the stop); the reader
    /// answers it as held.
    pub fn in_bounds(&self) -> bool {
        self.heat >= Decimal::ZERO && self.heat <= heat_max() && self.base_rate_usd_per_h > Decimal::ZERO && self.base_run_usd > Decimal::ZERO && self.k >= 1
    }
}

/// One instance's share of the swarm, keyed in the store by the instance name.
/// An instance with no share has weight 1.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.share.v1", schema = "ControlShare")]
pub struct Share {
    #[cultcache(key = 0)]
    pub instance: String,
    #[cultcache(key = 1)]
    pub weight: Decimal,
}

impl Share {
    /// A share is a slug-named instance with a positive weight.
    pub fn in_bounds(&self) -> bool {
        Slug(self.instance.clone()).validate_slug().is_ok() && self.weight > Decimal::ZERO
    }
}

/// The swarm store's contents, decoded and not yet judged.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SwarmState {
    pub brake: Option<Brake>,
    pub dial: Option<SwarmDial>,
    pub shares: Vec<Share>,
}

/// What the swarm store allows right now.
#[derive(Debug, PartialEq, Eq)]
pub enum SwarmEffective {
    Held(HeldReason),
    Released(Envelope),
}

/// The swarm's budget while the brake is released and heat is above 0. Its
/// fields are private and only `read_envelope` makes one, so a run cap derived
/// from it exists only as the reading of the root-owned swarm store.
#[derive(Debug, PartialEq, Eq)]
pub struct Envelope {
    rate_usd_per_h: Decimal,
    run_cap_usd: Decimal,
    k: u32,
    shares: std::collections::BTreeMap<String, Decimal>,
}

impl Envelope {
    /// The swarm's total spend rate: `base_rate_usd_per_h x heat`.
    pub fn rate_usd_per_h(&self) -> Decimal {
        self.rate_usd_per_h
    }

    /// The most any one run may spend: `base_run_usd x heat`.
    pub fn run_cap_usd(&self) -> Decimal {
        self.run_cap_usd
    }

    /// How many turns may run at once.
    pub fn k(&self) -> u32 {
        self.k
    }

    /// The instance's weight; 1 when the store holds no share for it.
    pub fn share(&self, instance: &Slug) -> Decimal {
        self.shares.get(&instance.0).copied().unwrap_or(Decimal::ONE)
    }

    /// Narrows this envelope and the instance's own reading into the Grant for
    /// one run of `budget_usd`. It is the only new way to build a Grant and it
    /// never widens: `None` unless the instance's own reading is released for
    /// this instance and `0 < budget_usd` is within both the swarm's run cap and
    /// the instance's own. The Grant's cap is the budget.
    pub fn grant_for(&self, instance: &Slug, instance_effective: &Effective, budget_usd: Decimal) -> Option<Grant> {
        let Effective::Released(own) = instance_effective else {
            return None;
        };
        if own.instance != *instance || budget_usd <= Decimal::ZERO || budget_usd > self.run_cap_usd || budget_usd > own.run_cap_usd {
            return None;
        }
        Some(Grant { instance: instance.clone(), cadence: own.cadence, run_cap_usd: budget_usd })
    }
}

impl SwarmState {
    /// The one derivation. Order: brake, dial bounds, heat, shares.
    pub(crate) fn effective(&self) -> SwarmEffective {
        let held = SwarmEffective::Held;
        let Some(brake) = &self.brake else {
            return held(HeldReason::BrakeAbsent);
        };
        if !brake.released {
            return held(HeldReason::BrakeHeld);
        }
        let Some(dial) = &self.dial else {
            return held(HeldReason::DialAbsent);
        };
        if !dial.in_bounds() {
            return held(HeldReason::DialOutOfBounds);
        }
        if dial.heat.is_zero() {
            return held(HeldReason::HeatZero);
        }
        if !self.shares.iter().all(Share::in_bounds) {
            return held(HeldReason::ShareOutOfBounds);
        }
        match (dial.base_rate_usd_per_h.checked_mul(dial.heat), dial.base_run_usd.checked_mul(dial.heat)) {
            (Some(rate_usd_per_h), Some(run_cap_usd)) => SwarmEffective::Released(Envelope {
                rate_usd_per_h,
                run_cap_usd,
                k: dial.k,
                shares: self.shares.iter().map(|share| (share.instance.clone(), share.weight)).collect(),
            }),
            _ => held(HeldReason::DialOutOfBounds),
        }
    }
}

/// Decodes the swarm store. An envelope the cache does not know, a payload that
/// does not decode or a share not keyed by its own instance is an error.
pub fn load_swarm_state(source: &impl ControlSource) -> Result<SwarmState> {
    let mut cache = swarm_cache()?;
    for envelope in source.snapshot().context("swarm store cannot be read")? {
        cache.put_raw_envelope(envelope)?;
    }
    let mut shares = Vec::new();
    for (key, share) in cache.get_all_with_keys::<Share>()? {
        anyhow::ensure!(key == share.instance, "a share is not keyed by its instance");
        shares.push(share);
    }
    Ok(SwarmState { brake: cache.get::<Brake>(BRAKE_KEY)?, dial: cache.get::<SwarmDial>(DIAL_KEY)?, shares })
}

/// The one reader of the swarm store, at `CONTROL_ROOT/swarm.cc`; the caller
/// supplies neither a root, a path nor a source. Anything but a decodable store
/// holding a released brake, an in-bounds dial with heat above 0 and in-bounds
/// shares is `Held`, with the reason.
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
/// as `ControlWriter`. Each setter replaces its document whole.
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

    /// Refuses an out-of-bounds dial before anything is written.
    pub fn set_dial(&mut self, dial: SwarmDial) -> Result<()> {
        if !dial.in_bounds() {
            anyhow::bail!(
                "swarm dial refused: heat must be within 0..={}, base_rate_usd_per_h and base_run_usd above 0, k at least 1",
                heat_max()
            );
        }
        prepare_store(&self.path)?;
        self.cache.put(DIAL_KEY, &dial)?;
        self.settle()
    }

    /// Refuses a share for a non-slug instance or a weight at or below 0.
    pub fn set_share(&mut self, share: Share) -> Result<()> {
        if !share.in_bounds() {
            anyhow::bail!("share refused: the instance must be a slug and the weight above 0");
        }
        prepare_store(&self.path)?;
        self.cache.put(share.instance.clone(), &share)?;
        self.settle()
    }
}
