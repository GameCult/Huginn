//! The writer's embedder: an ordered list of endpoints serving one model. The
//! first that answers with the run's model identity embeds; one that is
//! unreachable, failing, or serving another model (name, digest or length) is
//! skipped and named in the log, and is never written with. Daemons do not use
//! this: a query embeds on its daemon's own embedder.

use std::time::Duration;

use anyhow::{Result, bail};

use crate::index::{Embedder, ModelIdentity};

/// Endpoints in the order they are preferred, each with the name the log uses.
/// The first identity read of a run pins the model; every endpoint must then
/// report that identity before it is written with.
pub struct FailoverEmbedder<E: Embedder> {
    endpoints: Vec<(String, E)>,
    pinned: Option<ModelIdentity>,
}

impl<E: Embedder> FailoverEmbedder<E> {
    pub fn new(endpoints: Vec<(String, E)>) -> Self {
        Self { endpoints, pinned: None }
    }

    /// The first endpoint at or after `from` that answers with the pinned
    /// identity. The others are named in the log and in the error if none does.
    fn serving(&mut self, from: usize, within: Duration) -> Result<usize> {
        let mut skipped = Vec::new();
        for at in from..self.endpoints.len() {
            let name = self.endpoints[at].0.clone();
            match self.endpoints[at].1.model_identity(within) {
                Ok(identity) => {
                    let pinned = self.pinned.get_or_insert_with(|| identity.clone());
                    if *pinned == identity {
                        return Ok(at);
                    }
                    eprintln!("huginn-sources: embedder {name} serves {identity:?}, not {pinned:?}; skipped");
                    skipped.push(format!("{name}: another model"));
                }
                Err(error) => {
                    eprintln!("huginn-sources: embedder {name} is unavailable: {error:#}; trying the next");
                    skipped.push(format!("{name}: {error:#}"));
                }
            }
        }
        bail!("no embedder endpoint can serve this run: {}", skipped.join("; "))
    }
}

impl<E: Embedder> Embedder for FailoverEmbedder<E> {
    fn model_identity(&mut self, within: Duration) -> Result<ModelIdentity> {
        self.serving(0, within)?;
        Ok(self.pinned.clone().expect("serving pins the model"))
    }

    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>> {
        let mut failures = Vec::new();
        let mut from = 0;
        while from < self.endpoints.len() {
            let at = match self.serving(from, within) {
                Ok(at) => at,
                Err(error) => bail!("no embedder endpoint embedded the batch: {}{error:#}", failures.join("; ")),
            };
            match self.endpoints[at].1.embed(texts, within) {
                Ok(vectors) => return Ok(vectors),
                Err(error) => {
                    let name = &self.endpoints[at].0;
                    eprintln!("huginn-sources: embedder {name} failed an embed: {error:#}; trying the next");
                    failures.push(format!("{name}: {error:#}; "));
                    from = at + 1;
                }
            }
        }
        bail!("no embedder endpoint embedded the batch: {}", failures.concat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::fakes::FakeEmbedder;

    const WITHIN: Duration = Duration::from_secs(1);

    fn failover(spec: &[(&str, &str)]) -> (FailoverEmbedder<FakeEmbedder>, Vec<FakeEmbedder>) {
        let fakes: Vec<FakeEmbedder> = spec.iter().map(|(_, digest)| FakeEmbedder::new(digest)).collect();
        let endpoints = spec.iter().zip(&fakes).map(|((name, _), fake)| ((*name).to_owned(), fake.clone())).collect();
        (FailoverEmbedder::new(endpoints), fakes)
    }

    fn used(fake: &FakeEmbedder) -> usize {
        fake.texts().len()
    }

    /// The first healthy endpoint in order embeds; the others are not touched.
    #[test]
    fn the_first_healthy_endpoint_embeds() {
        let (mut embedder, fakes) = failover(&[("raven", "d1"), ("starfire", "d1"), ("yggdrasil", "d1")]);
        embedder.embed(&["a".into()], WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[1]), used(&fakes[2])), (1, 0, 0));
    }

    /// A down endpoint hands the batch to the next, the CPU box embeds when
    /// both desktops are down, and the preferred one is used again once it
    /// answers.
    #[test]
    fn a_down_endpoint_hands_over_to_the_next_and_comes_back() {
        let (mut embedder, fakes) = failover(&[("raven", "d1"), ("starfire", "d1"), ("yggdrasil", "d1")]);
        fakes[0].state.lock().unwrap().down = true;
        embedder.embed(&["a".into()], WITHIN).unwrap();
        assert_eq!((used(&fakes[0]), used(&fakes[1]), used(&fakes[2])), (0, 1, 0));
        fakes[1].state.lock().unwrap().down = true;
        embedder.embed(&["b".into()], WITHIN).unwrap();
        assert_eq!(used(&fakes[2]), 1, "both desktops down, the CPU embeds");
        fakes[0].state.lock().unwrap().down = false;
        embedder.embed(&["c".into()], WITHIN).unwrap();
        assert_eq!(used(&fakes[0]), 1, "the preferred endpoint is used again when it answers");
    }

    /// An endpoint serving another digest or another length is skipped and
    /// never written with, even though it answers; with nothing left the run
    /// fails naming each endpoint.
    #[test]
    fn a_mismatched_endpoint_is_skipped_and_never_embeds() {
        let (mut embedder, fakes) = failover(&[("raven", "d1"), ("starfire", "d2"), ("yggdrasil", "d1")]);
        embedder.model_identity(WITHIN).unwrap();
        fakes[0].state.lock().unwrap().down = true;
        embedder.embed(&["a".into()], WITHIN).unwrap();
        assert_eq!((used(&fakes[1]), used(&fakes[2])), (0, 1), "starfire serves d2 and is skipped");
        fakes[2].state.lock().unwrap().identity.dimensions = 8;
        let error = format!("{:#}", embedder.embed(&["b".into()], WITHIN).unwrap_err());
        assert!(error.contains("raven") && error.contains("starfire: another model") && error.contains("yggdrasil: another model"), "{error}");
        assert_eq!(used(&fakes[1]), 0);
    }

    /// With every endpoint down the identity read fails naming them all, and
    /// the first one back up serves.
    #[test]
    fn with_every_endpoint_down_the_run_fails_naming_them_all() {
        let (mut embedder, fakes) = failover(&[("raven", "d1"), ("yggdrasil", "d1")]);
        for fake in &fakes {
            fake.state.lock().unwrap().down = true;
        }
        let error = format!("{:#}", embedder.model_identity(WITHIN).unwrap_err());
        assert!(error.contains("raven") && error.contains("yggdrasil"), "{error}");
        fakes[1].state.lock().unwrap().down = false;
        assert_eq!(embedder.model_identity(WITHIN).unwrap().digest, "d1");
    }
}
