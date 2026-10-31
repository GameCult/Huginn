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
//! The model of the pool is the first identity any endpoint reports, primaries
//! first. An endpoint's identity is read when it is admitted, not on every
//! batch: a model re-pulled under a running pool is noticed at the next
//! admission, and a writer is a short-lived job.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::{Embedder, ModelIdentity};

/// How long an endpoint that dropped out is left alone by default.
pub const RETRY_AFTER: Duration = Duration::from_secs(60);

struct Member<E> {
    name: String,
    embedder: E,
    reserve: bool,
    /// `None` while the endpoint is in the pool; otherwise when it may be read
    /// again, and why it left.
    out: Option<(Instant, String)>,
}

pub struct EmbedderPool<E: Embedder + Send> {
    members: Vec<Member<E>>,
    pinned: Option<ModelIdentity>,
    retry_after: Duration,
}

impl<E: Embedder + Send> EmbedderPool<E> {
    /// `primaries` share the work; `reserve` serves only when none of them can.
    /// Every endpoint starts out, to be admitted by its first identity read.
    pub fn new(primaries: Vec<(String, E)>, reserve: Vec<(String, E)>) -> Self {
        let member = |reserve: bool| {
            move |(name, embedder): (String, E)| Member { name, embedder, reserve, out: Some((Instant::now(), "not yet read".into())) }
        };
        let members = primaries.into_iter().map(member(false)).chain(reserve.into_iter().map(member(true))).collect();
        Self { members, pinned: None, retry_after: RETRY_AFTER }
    }

    pub fn retrying_after(mut self, retry_after: Duration) -> Self {
        self.retry_after = retry_after;
        self
    }

    /// Reads the identity of every endpoint that is out and due, in list order,
    /// and lets in those that serve the pool's model.
    fn admit(&mut self, within: Duration) {
        let now = Instant::now();
        for at in 0..self.members.len() {
            if !self.members[at].out.as_ref().is_some_and(|(due, _)| *due <= now) {
                continue;
            }
            let name = self.members[at].name.clone();
            let outcome = match self.members[at].embedder.model_identity(within) {
                Ok(identity) => {
                    let pinned = self.pinned.get_or_insert_with(|| identity.clone());
                    if *pinned == identity {
                        None
                    } else {
                        Some(format!("serves {identity:?}, not {pinned:?}"))
                    }
                }
                Err(error) => Some(format!("{error:#}")),
            };
            if let Some(why) = &outcome {
                eprintln!("huginn-sources: embedder {name} is out of the pool: {why}");
            }
            self.members[at].out = outcome.map(|why| (Instant::now() + self.retry_after, why));
        }
    }

    fn drop_out(&mut self, at: usize, why: String) {
        eprintln!("huginn-sources: embedder {} is out of the pool: {why}", self.members[at].name);
        self.members[at].out = Some((Instant::now() + self.retry_after, why));
    }

    /// The members to work with now: the healthy primaries, or if none, the
    /// healthy reserve.
    fn active(&self) -> Vec<usize> {
        let healthy = |reserve: bool| -> Vec<usize> {
            (0..self.members.len()).filter(|at| self.members[*at].reserve == reserve && self.members[*at].out.is_none()).collect()
        };
        let primaries = healthy(false);
        if primaries.is_empty() { healthy(true) } else { primaries }
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

impl<E: Embedder + Send> Embedder for EmbedderPool<E> {
    fn model_identity(&mut self, within: Duration) -> Result<ModelIdentity> {
        self.admit(within);
        match &self.pinned {
            Some(pinned) if !self.active().is_empty() => Ok(pinned.clone()),
            _ => Err(self.unavailable()),
        }
    }

    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>> {
        self.admit(within);
        let mut vectors: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        // Pieces still owed, as (first index, length) over `texts`.
        let mut owed = vec![(0, texts.len())];
        while !owed.is_empty() {
            let active = self.active();
            if active.is_empty() {
                bail!("{}", self.unavailable());
            }
            // Each owed piece is cut evenly over the active members.
            let mut jobs: Vec<(usize, usize, usize)> = Vec::new(); // (member, first, length)
            for (first, length) in owed.drain(..) {
                let share = length.div_ceil(active.len()).max(1);
                for (n, start) in (first..first + length).step_by(share).enumerate() {
                    jobs.push((active[n % active.len()], start, share.min(first + length - start)));
                }
            }
            let mut answers: Vec<(usize, usize, usize, Result<Vec<Vec<f32>>>)> = Vec::new();
            std::thread::scope(|scope| {
                let mut taken: Vec<Option<&mut Member<E>>> = self.members.iter_mut().map(Some).collect();
                let mut running = Vec::new();
                for (member, first, length) in jobs {
                    // A member holds one job per round; later pieces for it wait for the next.
                    if let Some(held) = taken[member].take() {
                        let piece = &texts[first..first + length];
                        running.push((member, first, length, scope.spawn(move || held.embedder.embed(piece, within))));
                    } else {
                        owed.push((first, length));
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
    use crate::index::fakes::{FakeEmbedder, Gate};

    const WITHIN: Duration = Duration::from_secs(1);

    fn pool(primaries: &[(&str, &str)], reserve: &[(&str, &str)]) -> (EmbedderPool<FakeEmbedder>, Vec<FakeEmbedder>) {
        let build = |spec: &[(&str, &str)]| -> Vec<(String, FakeEmbedder)> {
            spec.iter().map(|(name, digest)| ((*name).to_owned(), FakeEmbedder::new(digest))).collect()
        };
        let (primaries, reserve) = (build(primaries), build(reserve));
        let handles = primaries.iter().chain(&reserve).map(|(_, fake)| fake.clone()).collect();
        (EmbedderPool::new(primaries, reserve).retrying_after(Duration::ZERO), handles)
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
        pool.model_identity(WITHIN).unwrap();
        down(&fakes[0], true);
        let batch = texts(6);
        let vectors = pool.embed(&batch, WITHIN).unwrap();
        assert_eq!(used(&fakes[1]), 6, "starfire took raven's piece as well as its own");
        for (text, vector) in batch.iter().zip(&vectors) {
            assert_eq!(vector[0], text.len() as f32);
        }
        let mut waiting = EmbedderPool::new(vec![("raven".into(), fakes[0].clone()), ("starfire".into(), fakes[1].clone())], vec![]);
        down(&fakes[0], false);
        waiting.embed(&texts(2), WITHIN).unwrap();
        assert_eq!(used(&fakes[0]), 1, "back in the pool at the next admission");
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
}
