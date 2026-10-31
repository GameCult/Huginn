//! A pool of embedder endpoints serving one model, as one `Embedder`.
//!
//! The endpoints are two tiers. The primaries (the GPU hosts) all take work at
//! once: a batch is cut into contiguous pieces, one per healthy primary, and
//! the pieces are embedded concurrently and joined back in order. The reserve
//! (the CPU host) takes work only while no primary is healthy. An endpoint is
//! admitted only after its model identity (name, digest, dimensions) equals the
//! pool's; one that is unreachable, failing or serving another model drops out,
//! is named in the log, is never written with, and is read again once
//! `retry_after` has passed. A piece that fails on one endpoint is embedded
//! again on another, so a batch fails only when no endpoint is left to take it.
//!
//! The model of the pool is the one the collection was built with: the caller
//! hands it over with `built_under` before it reads the identity, and an
//! endpoint serving anything else never takes work, even when it is the only
//! one up, so a run cannot rebuild an index around a foreign model. Only when
//! there is no collection yet does the pool adopt a model: the first one
//! reported in list order, primaries first, once every endpoint has answered.
//!
//! An endpoint's identity is read off the batch's clock. Each read runs on its
//! own thread under `IDENTITY_WITHIN`, and a batch waits for the reads only
//! while no healthy primary exists to start on: a primary being read keeps the
//! reserve waiting, so a returning GPU is used and not passed over for the CPU,
//! but an endpoint that drops packets never delays work a healthy one can do.

use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::{Embedder, ModelIdentity};

/// How long an endpoint that dropped out is left alone by default.
pub const RETRY_AFTER: Duration = Duration::from_secs(60);

/// The bound on one identity read, connect and answer together. Reading a
/// model's identity loads nothing, so an endpoint that needs longer is dark.
pub const IDENTITY_WITHIN: Duration = Duration::from_secs(5);

/// One piece's outcome: the member that took it, where it starts, how long it is,
/// and what came back.
type Answer = (usize, usize, usize, Result<Vec<Vec<f32>>>);

struct Health {
    name: String,
    reserve: bool,
    /// `None` while the endpoint is in the pool; otherwise when it may be read
    /// again, and why it left.
    out: Option<(Instant, String)>,
    probing: bool,
    /// What the last read found, until the roster has judged it.
    seen: Option<Result<ModelIdentity, String>>,
}

struct Roster {
    pinned: Option<ModelIdentity>,
    members: Vec<Health>,
}

struct Shared {
    roster: Mutex<Roster>,
    changed: Condvar,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Roster {
    fn healthy(&self, reserve: bool) -> Vec<usize> {
        (0..self.members.len()).filter(|at| self.members[*at].reserve == reserve && self.members[*at].out.is_none()).collect()
    }

    /// The members to work with now: the healthy primaries, or if none, the
    /// healthy reserve.
    fn active(&self) -> Vec<usize> {
        let primaries = self.healthy(false);
        if primaries.is_empty() { self.healthy(true) } else { primaries }
    }

    fn reading(&self, reserve: bool) -> bool {
        self.members.iter().any(|member| member.reserve == reserve && member.probing)
    }

    /// Whether a batch may start: a healthy primary is there to start on; else
    /// no primary is still being read, and the reserve is healthy or nothing is
    /// left being read. Without a model to hold endpoints to, it waits for every
    /// read, so the model adopted does not depend on who answered first.
    fn ready(&self) -> bool {
        if self.pinned.is_none() && self.members.iter().any(|member| member.probing) {
            return false;
        }
        if !self.healthy(false).is_empty() {
            return true;
        }
        if self.reading(false) {
            return false;
        }
        !self.healthy(true).is_empty() || !self.reading(true)
    }

    /// Adopts a model when there is none and every read is in, then lets in the
    /// endpoints that serve it and puts out the rest.
    fn judge(&mut self, retry_after: Duration) {
        if self.pinned.is_none() && !self.members.iter().any(|member| member.probing) {
            self.pinned =
                self.members.iter().find_map(|member| member.seen.as_ref().and_then(|seen| seen.as_ref().ok())).cloned();
        }
        let pinned = self.pinned.clone();
        for member in &mut self.members {
            let why = match (&pinned, &member.seen) {
                (_, None) | (None, Some(Ok(_))) => continue,
                (_, Some(Err(why))) => why.clone(),
                (Some(pinned), Some(Ok(identity))) if pinned == identity => {
                    member.out = None;
                    member.seen = None;
                    continue;
                }
                (Some(pinned), Some(Ok(identity))) => format!("serves {identity:?}, not {pinned:?}"),
            };
            member.seen = None;
            eprintln!("huginn embedder pool: {} is out of the pool: {why}", member.name);
            member.out = Some((Instant::now() + retry_after, why));
        }
    }

    fn unavailable(&self) -> anyhow::Error {
        let named: Vec<String> = self
            .members
            .iter()
            .map(|member| format!("{}: {}", member.name, member.out.as_ref().map_or("failed this batch", |(_, why)| why.as_str())))
            .collect();
        anyhow::anyhow!("no embedder endpoint can take the batch: {}", named.join("; "))
    }
}

/// Reads one endpoint's identity and tells the roster. Runs on its own thread.
fn probe<E: Embedder>(shared: &Shared, embedder: &Mutex<E>, at: usize, retry_after: Duration) {
    let read = std::panic::catch_unwind(AssertUnwindSafe(|| locked(embedder).model_identity(IDENTITY_WITHIN)))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("the embedder panicked")));
    let mut roster = locked(&shared.roster);
    roster.members[at].probing = false;
    roster.members[at].seen = Some(read.map_err(|error| format!("{error:#}")));
    roster.judge(retry_after);
    drop(roster);
    shared.changed.notify_all();
}

pub struct EmbedderPool<E: Embedder + Send + 'static> {
    embedders: Vec<Arc<Mutex<E>>>,
    shared: Arc<Shared>,
    retry_after: Duration,
}

impl<E: Embedder + Send + 'static> EmbedderPool<E> {
    /// `primaries` share the work; `reserve` serves only when none of them can.
    /// Every endpoint starts out, to be admitted by its first identity read.
    pub fn new(primaries: Vec<(String, E)>, reserve: Vec<(String, E)>) -> Self {
        let mut embedders = Vec::new();
        let mut members = Vec::new();
        let tiers = primaries.into_iter().map(|primary| (false, primary)).chain(reserve.into_iter().map(|spare| (true, spare)));
        for (is_reserve, (name, embedder)) in tiers {
            embedders.push(Arc::new(Mutex::new(embedder)));
            members.push(Health { name, reserve: is_reserve, out: Some((Instant::now(), "not yet read".into())), probing: false, seen: None });
        }
        let shared = Arc::new(Shared { roster: Mutex::new(Roster { pinned: None, members }), changed: Condvar::new() });
        Self { embedders, shared, retry_after: RETRY_AFTER }
    }

    pub fn retrying_after(mut self, retry_after: Duration) -> Self {
        self.retry_after = retry_after;
        self
    }

    /// Starts a read of every endpoint that is out and due, each on its own
    /// thread, and waits for them only until a batch can start (`Roster::ready`)
    /// or `within` has passed.
    fn admit(&self, within: Duration) {
        let started = Instant::now();
        let mut roster = locked(&self.shared.roster);
        for at in 0..self.embedders.len() {
            let member = &mut roster.members[at];
            if member.probing || member.out.as_ref().is_none_or(|(due, _)| *due > started) {
                continue;
            }
            member.probing = true;
            let (shared, embedder, retry_after) = (Arc::clone(&self.shared), Arc::clone(&self.embedders[at]), self.retry_after);
            std::thread::spawn(move || probe(&shared, &embedder, at, retry_after));
        }
        while !roster.ready() {
            let left = (started + within).saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            roster = self.shared.changed.wait_timeout(roster, left).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    fn drop_out(&self, at: usize, why: String) {
        let mut roster = locked(&self.shared.roster);
        eprintln!("huginn embedder pool: {} is out of the pool: {why}", roster.members[at].name);
        roster.members[at].out = Some((Instant::now() + self.retry_after, why));
    }

    fn active(&self) -> Vec<usize> {
        locked(&self.shared.roster).active()
    }

    /// Waits until no read is running: a test's way to say the pool has heard
    /// every endpoint.
    #[cfg(test)]
    fn settle(&self) {
        let mut roster = locked(&self.shared.roster);
        while roster.members.iter().any(|member| member.probing) {
            roster = self.shared.changed.wait_timeout(roster, Duration::from_millis(50)).unwrap_or_else(PoisonError::into_inner).0;
        }
    }
}

impl<E: Embedder + Send + 'static> Embedder for EmbedderPool<E> {
    fn built_under(&mut self, built: Option<&ModelIdentity>) {
        let mut roster = locked(&self.shared.roster);
        if roster.pinned.as_ref() == built {
            return;
        }
        roster.pinned = built.cloned();
        for member in &mut roster.members {
            member.seen = None;
            member.out = Some((Instant::now(), "not yet read under the collection's model".into()));
        }
    }

    fn model_identity(&mut self, within: Duration) -> Result<ModelIdentity> {
        self.admit(within);
        let roster = locked(&self.shared.roster);
        match &roster.pinned {
            Some(pinned) if !roster.active().is_empty() => Ok(pinned.clone()),
            _ => Err(roster.unavailable()),
        }
    }

    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>> {
        self.admit(within);
        let mut vectors: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        // Pieces still owed, as (first index, length) over `texts`.
        let mut owed = vec![(0, texts.len())];
        while !owed.is_empty() {
            let mut active = self.active();
            if active.is_empty() {
                // Every member that took the last round failed; the reserve may
                // still be on its first read.
                self.admit(within);
                active = self.active();
            }
            if active.is_empty() {
                bail!("{}", locked(&self.shared.roster).unavailable());
            }
            // Each owed piece is cut evenly over the active members.
            let mut jobs: Vec<(usize, usize, usize)> = Vec::new(); // (member, first, length)
            for (first, length) in owed.drain(..) {
                let share = length.div_ceil(active.len()).max(1);
                for (n, start) in (first..first + length).step_by(share).enumerate() {
                    jobs.push((active[n % active.len()], start, share.min(first + length - start)));
                }
            }
            let mut answers: Vec<Answer> = Vec::new();
            std::thread::scope(|scope| {
                let mut busy = vec![false; self.embedders.len()];
                let mut running = Vec::new();
                for (member, first, length) in jobs {
                    // A member holds one job per round; later pieces for it wait for the next.
                    if std::mem::replace(&mut busy[member], true) {
                        owed.push((first, length));
                    } else {
                        let (piece, embedder) = (&texts[first..first + length], &self.embedders[member]);
                        running.push((member, first, length, scope.spawn(move || locked(embedder).embed(piece, within))));
                    }
                }
                for (member, first, length, handle) in running {
                    let answer = handle.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the embedder thread panicked")));
                    answers.push((member, first, length, answer));
                }
            });
            for (member, first, length, answer) in answers {
                match answer {
                    Ok(found) if found.len() == length => {
                        for (offset, vector) in found.into_iter().enumerate() {
                            vectors[first + offset] = Some(vector);
                        }
                    }
                    Ok(found) => {
                        self.drop_out(member, format!("returned {} vectors for {length} texts", found.len()));
                        owed.push((first, length));
                    }
                    Err(error) => {
                        self.drop_out(member, format!("failed an embed: {error:#}"));
                        owed.push((first, length));
                    }
                }
            }
        }
        Ok(vectors.into_iter().map(|vector| vector.expect("every piece was embedded")).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::fakes::{FakeEmbedder, Gate, identity};

    const WITHIN: Duration = Duration::from_secs(1);

    fn pool(primaries: &[(&str, &str)], reserve: &[(&str, &str)]) -> (EmbedderPool<FakeEmbedder>, Vec<FakeEmbedder>) {
        let build = |spec: &[(&str, &str)]| -> Vec<(String, FakeEmbedder)> {
            spec.iter().map(|(name, digest)| ((*name).to_owned(), FakeEmbedder::new(digest))).collect()
        };
        let (primaries, reserve) = (build(primaries), build(reserve));
        let handles = primaries.iter().chain(&reserve).map(|(_, fake)| fake.clone()).collect();
        (EmbedderPool::new(primaries, reserve).retrying_after(Duration::ZERO), handles)
    }

    /// Hears every endpoint once, so a test starts from a pool that knows who is up.
    fn warm(pool: &EmbedderPool<FakeEmbedder>) {
        pool.admit(WITHIN);
        pool.settle();
    }

    fn texts(count: usize) -> Vec<String> {
        (0..count).map(|n| "x".repeat(n + 1)).collect()
    }

    fn used(fake: &FakeEmbedder) -> usize {
        fake.texts().len()
    }

    fn down(fake: &FakeEmbedder, is_down: bool) {
        fake.state.lock().unwrap().down = is_down;
    }

    /// Both primaries take a batch at the same moment: each is inside its embed
    /// before either is let go, and the vectors come back in the texts' order.
    #[test]
    fn both_primaries_embed_one_batch_concurrently_and_the_order_is_kept() {
        let gate = Gate::shut();
        let a = FakeEmbedder::new("d1").behind(&gate);
        let b = FakeEmbedder::new("d1").behind(&gate);
        let reserve = FakeEmbedder::new("d1");
        let mut pool = EmbedderPool::new(vec![("raven".into(), a.clone()), ("starfire".into(), b.clone())], vec![("yggdrasil".into(), reserve.clone())]);
        warm(&pool);
        let batch = texts(8);
        let opener = {
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait_arrived(2);
                gate.open();
            })
        };
        let vectors = pool.embed(&batch, WITHIN).unwrap();
        opener.join().unwrap();
        assert_eq!((used(&a), used(&b), used(&reserve)), (4, 4, 0), "an even share each, the reserve idle");
        assert_eq!(vectors.len(), 8);
        for (text, vector) in batch.iter().zip(&vectors) {
            assert_eq!(vector[0], text.len() as f32, "the vector at each position is that text's");
        }
    }


    /// The reserve takes work only while no primary is in the pool, and gives
    /// it back when a primary returns.
    #[test]
    fn the_reserve_serves_only_while_both_primaries_are_out() {
        let (mut pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d1")], &[("yggdrasil", "d1")]);
        down(&fakes[0], true);
        warm(&pool);
        pool.embed(&texts(4), WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[1]), used(&fakes[2])), (0, 4, 0), "one GPU left takes all of it");
        down(&fakes[1], true);
        pool.embed(&texts(3), WITHIN).unwrap();
        assert_eq!(used(&fakes[2]), 3, "both GPUs out, the CPU takes it");
        down(&fakes[0], false);
        pool.embed(&texts(2), WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[2])), (2, 3), "a GPU back in the pool, the CPU is left alone");
    }

    /// A piece that fails on one endpoint is embedded on another within the
    /// same call, in order, and the failed endpoint is not asked again until
    /// its retry time.
    #[test]
    fn a_failing_endpoint_drops_out_its_piece_moves_on_and_it_is_retried_later() {
        let (mut pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d1")], &[("yggdrasil", "d1")]);
        warm(&pool);
        down(&fakes[0], true);
        let batch = texts(6);
        let vectors = pool.embed(&batch, WITHIN).unwrap();
        assert_eq!(used(&fakes[1]), 6, "starfire took raven's piece as well as its own");
        for (text, vector) in batch.iter().zip(&vectors) {
            assert_eq!(vector[0], text.len() as f32);
        }
        down(&fakes[0], false);
        warm(&pool);
        pool.embed(&texts(2), WITHIN).unwrap();
        assert_eq!(used(&fakes[0]), 1, "back in the pool at the next admission");
    }

    /// With a retry time that has not come, a dropped endpoint is not read and
    /// not used; once it has come, it is.
    #[test]
    fn a_dropped_endpoint_stays_out_until_its_retry_time() {
        let (pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d1")], &[]);
        let mut pool = pool.retrying_after(Duration::from_millis(400));
        warm(&pool);
        down(&fakes[0], true);
        pool.embed(&texts(2), WITHIN).unwrap();
        down(&fakes[0], false);
        let reads = fakes[0].state.lock().unwrap().identity_bounds.len();
        pool.embed(&texts(2), WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), fakes[0].state.lock().unwrap().identity_bounds.len()), (0, reads), "not read, not used");
        std::thread::sleep(Duration::from_millis(450));
        warm(&pool);
        pool.embed(&texts(2), WITHIN).unwrap();
        assert_eq!(used(&fakes[0]), 1, "due, read and used");
    }

    /// An endpoint serving another digest or another length is never written
    /// with, even when it is the only one answering; the log names it.
    #[test]
    fn a_mismatched_endpoint_is_never_written_with() {
        let (mut pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d2")], &[("yggdrasil", "d1")]);
        pool.embed(&texts(4), WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[1]), used(&fakes[2])), (4, 0, 0));
        down(&fakes[0], true);
        pool.embed(&texts(2), WITHIN).unwrap();
        assert_eq!((used(&fakes[1]), used(&fakes[2])), (0, 2), "starfire is skipped for the CPU, which serves the pool's model");
        fakes[2].state.lock().unwrap().identity.dimensions = 8;
        down(&fakes[2], true);
        assert!(pool.embed(&texts(1), WITHIN).is_err(), "nothing left that serves the pool's model");
        down(&fakes[2], false);
        let error = format!("{:#}", pool.embed(&texts(1), WITHIN).unwrap_err());
        assert!(error.contains("raven") && error.contains("starfire") && error.contains("yggdrasil"), "{error}");
        assert_eq!(used(&fakes[1]), 0);
    }

    /// With every endpoint down, the identity read and the embed both fail
    /// naming each; the pool's model is what the first endpoint reported.
    #[test]
    fn with_every_endpoint_down_the_pool_fails_naming_them_all() {
        let (mut pool, fakes) = pool(&[("raven", "d1")], &[("yggdrasil", "d1")]);
        assert_eq!(pool.model_identity(WITHIN).unwrap().digest, "d1");
        for fake in &fakes {
            down(fake, true);
        }
        let error = format!("{:#}", pool.embed(&texts(2), WITHIN).unwrap_err());
        assert!(error.contains("raven") && error.contains("yggdrasil"), "{error}");
        let (mut cold, cold_fakes) = self::pool(&[("raven", "d1")], &[("yggdrasil", "d1")]);
        down(&cold_fakes[0], true);
        down(&cold_fakes[1], true);
        assert!(format!("{:#}", cold.model_identity(WITHIN).unwrap_err()).contains("raven"));
    }

    /// An endpoint that never answers an identity read costs a batch nothing:
    /// across several retry windows every batch finishes at the healthy
    /// endpoint's pace, however long the caller's budget is, and the dark
    /// endpoint is read under the pool's own short bound, never the caller's.
    #[test]
    fn a_dark_endpoint_never_delays_a_batch_a_healthy_one_can_take() {
        let (pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d1")], &[("yggdrasil", "d1")]);
        let mut pool = pool.retrying_after(Duration::from_millis(100));
        fakes[1].state.lock().unwrap().hang_identity = true;
        pool.built_under(Some(&identity("d1")));
        let budget = Duration::from_secs(300);
        for _ in 0..4 {
            let started = Instant::now();
            pool.embed(&texts(4), budget).unwrap();
            assert!(started.elapsed() < Duration::from_millis(1500), "a batch waited {:?} on a dark endpoint", started.elapsed());
            std::thread::sleep(Duration::from_millis(150));
        }
        assert_eq!((used(&fakes[0]), used(&fakes[1]), used(&fakes[2])), (16, 0, 0));
        let bounds = fakes[1].state.lock().unwrap().identity_bounds.clone();
        assert!(!bounds.is_empty() && bounds.iter().all(|bound| *bound == IDENTITY_WITHIN), "{bounds:?}");
    }

    /// The model of the pool is the collection's: an endpoint serving another
    /// is not written with, however alone it is, and the identity is not given.
    #[test]
    fn the_pool_holds_endpoints_to_the_collections_model() {
        let (mut pool, fakes) = pool(&[("raven", "d2"), ("starfire", "d1")], &[("yggdrasil", "d2")]);
        pool.built_under(Some(&identity("d1")));
        pool.embed(&texts(4), WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[1]), used(&fakes[2])), (0, 4, 0), "only the endpoint serving the collection's model works");
        down(&fakes[1], true);
        assert!(pool.embed(&texts(1), WITHIN).is_err(), "a different model is not a substitute");
        assert!(pool.model_identity(WITHIN).is_err(), "and the pool does not claim the model of another");
        assert_eq!((used(&fakes[0]), used(&fakes[2])), (0, 0));
        down(&fakes[1], false);
        assert_eq!(pool.model_identity(WITHIN).unwrap().digest, "d1");
    }

    /// With no collection to hold to, the model is the first in list order that
    /// answers, whichever answers first; handing the pool a collection's model
    /// changes it, and handing it the same one again reads nothing.
    #[test]
    fn the_pool_adopts_a_model_only_without_a_collection_and_a_new_pin_rereads() {
        let (slow, quick) = (FakeEmbedder::new("d1"), FakeEmbedder::new("d2"));
        slow.state.lock().unwrap().identity_delay = Duration::from_millis(200);
        let mut pool = EmbedderPool::new(vec![("raven".into(), slow.clone()), ("starfire".into(), quick.clone())], vec![]);
        assert_eq!(pool.model_identity(WITHIN).unwrap().digest, "d1", "list order, not the quickest");
        pool.built_under(Some(&identity("d1")));
        let reads = quick.state.lock().unwrap().identity_bounds.len();
        pool.model_identity(WITHIN).unwrap();
        assert_eq!(quick.state.lock().unwrap().identity_bounds.len(), reads, "the same model again reads nothing");
        pool.built_under(Some(&identity("d2")));
        assert_eq!(pool.model_identity(WITHIN).unwrap().digest, "d2");
        pool.embed(&texts(2), WITHIN).unwrap();
        assert_eq!((used(&slow), used(&quick)), (0, 2));
        pool.built_under(None);
        assert_eq!(pool.model_identity(WITHIN).unwrap().digest, "d1", "no collection, the pool adopts again");
    }

    /// A batch waits only for what is coming: with every primary out and not due,
    /// the reserve takes it at once, and with nothing left it fails at once,
    /// however long the caller's budget.
    #[test]
    fn a_batch_does_not_wait_for_reads_that_are_not_running() {
        let (pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d1")], &[("yggdrasil", "d1")]);
        let mut pool = pool.retrying_after(Duration::from_secs(3600));
        warm(&pool);
        let budget = Duration::from_secs(10);
        down(&fakes[0], true);
        down(&fakes[1], true);
        let started = Instant::now();
        pool.embed(&texts(2), budget).unwrap();
        assert_eq!(used(&fakes[2]), 2);
        assert!(started.elapsed() < Duration::from_secs(2), "the reserve waited {:?} for GPUs that are not being read", started.elapsed());
        down(&fakes[2], true);
        let started = Instant::now();
        assert!(pool.embed(&texts(1), budget).is_err());
        assert!(started.elapsed() < Duration::from_secs(2), "the failure waited {:?} for reads that are not running", started.elapsed());
    }

    /// An endpoint whose read failed is not read again before its retry time.
    #[test]
    fn an_endpoint_that_failed_its_read_is_not_read_again_before_its_retry_time() {
        let (pool, fakes) = pool(&[("raven", "d1"), ("starfire", "d1")], &[]);
        let pool = pool.retrying_after(Duration::from_secs(3600));
        down(&fakes[0], true);
        warm(&pool);
        let reads = fakes[0].state.lock().unwrap().identity_bounds.len();
        warm(&pool);
        assert_eq!(fakes[0].state.lock().unwrap().identity_bounds.len(), reads);
    }

    /// A primary that is still being read keeps the reserve waiting, so the
    /// first batch goes to the GPU and not to the CPU that answered first.
    #[test]
    fn a_primary_still_being_read_is_waited_for_before_the_reserve_is_used() {
        let (mut pool, fakes) = pool(&[("raven", "d1")], &[("yggdrasil", "d1")]);
        fakes[0].state.lock().unwrap().identity_delay = Duration::from_millis(200);
        pool.built_under(Some(&identity("d1")));
        pool.embed(&texts(3), WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[1])), (3, 0));
    }
}
