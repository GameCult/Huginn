//! In-memory ports for the projector's tests. Each fake is a handle on shared
//! state, so a test keeps observing after it has moved the fake into a
//! projector or a worker thread.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{Result, bail};

use super::{CollectionMeta, Described, Embedder, ModelIdentity, Point, VectorIndex};

pub(crate) fn identity(digest: &str) -> ModelIdentity {
    ModelIdentity { name: "model".into(), digest: digest.into(), dimensions: 4 }
}

/// A door a test holds shut: `embed` announces it has arrived and waits.
#[derive(Clone, Default)]
pub(crate) struct Gate(Arc<(Mutex<(u32, bool)>, Condvar)>);

impl Gate {
    pub(crate) fn shut() -> Self {
        Self::default()
    }

    /// Blocks until `count` calls to `embed` have arrived at the gate.
    pub(crate) fn wait_arrived(&self, count: u32) {
        let mut state = self.0.0.lock().unwrap();
        while state.0 < count {
            state = self.0.1.wait(state).unwrap();
        }
    }

    pub(crate) fn open(&self) {
        self.0.0.lock().unwrap().1 = true;
        self.0.1.notify_all();
    }

    fn pass(&self) {
        let mut state = self.0.0.lock().unwrap();
        state.0 += 1;
        self.0.1.notify_all();
        while !state.1 {
            state = self.0.1.wait(state).unwrap();
        }
    }
}

pub(crate) struct EmbedderState {
    pub(crate) identity: ModelIdentity,
    pub(crate) down: bool,
    /// The texts of every completed call, in order.
    pub(crate) embedded: Vec<Vec<String>>,
}

#[derive(Clone)]
pub(crate) struct FakeEmbedder {
    pub(crate) state: Arc<Mutex<EmbedderState>>,
    gate: Option<Gate>,
}

impl FakeEmbedder {
    pub(crate) fn new(digest: &str) -> Self {
        let state = EmbedderState { identity: identity(digest), down: false, embedded: Vec::new() };
        Self { state: Arc::new(Mutex::new(state)), gate: None }
    }

    pub(crate) fn behind(mut self, gate: &Gate) -> Self {
        self.gate = Some(gate.clone());
        self
    }

    pub(crate) fn texts(&self) -> Vec<String> {
        self.state.lock().unwrap().embedded.iter().flatten().cloned().collect()
    }
}

impl Embedder for FakeEmbedder {
    fn model_identity(&mut self) -> Result<ModelIdentity> {
        let state = self.state.lock().unwrap();
        if state.down {
            bail!("the embedder is down");
        }
        Ok(state.identity.clone())
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if let Some(gate) = &self.gate {
            gate.pass();
        }
        let mut state = self.state.lock().unwrap();
        if state.down {
            bail!("the embedder is down");
        }
        state.embedded.push(texts.to_vec());
        let dimensions = state.identity.dimensions as usize;
        Ok(texts.iter().map(|text| (0..dimensions).map(|at| (text.len() + at) as f32).collect()).collect())
    }
}

pub(crate) struct Stored {
    pub(crate) label: Described,
    pub(crate) points: BTreeMap<String, Point>,
}

#[derive(Default)]
pub(crate) struct IndexState {
    pub(crate) collections: BTreeMap<String, Stored>,
    pub(crate) down: bool,
    pub(crate) recreated: Vec<(String, CollectionMeta)>,
    /// The point ids of every upsert call, in order.
    pub(crate) upserts: Vec<Vec<String>>,
}

#[derive(Clone, Default)]
pub(crate) struct FakeIndex {
    pub(crate) state: Arc<Mutex<IndexState>>,
}

impl FakeIndex {
    pub(crate) fn holding(&self, collection: &str) -> BTreeSet<String> {
        let state = self.state.lock().unwrap();
        state.collections.get(collection).map(|stored| stored.points.keys().cloned().collect()).unwrap_or_default()
    }

    /// Plants a collection this organ did not make, or made for another mind.
    pub(crate) fn plant(&self, collection: &str, label: Described) {
        self.state.lock().unwrap().collections.insert(collection.into(), Stored { label, points: BTreeMap::new() });
    }
}

impl VectorIndex for FakeIndex {
    fn describe(&mut self, collection: &str) -> Result<Described> {
        let state = self.state.lock().unwrap();
        if state.down {
            bail!("the vector store is down");
        }
        Ok(state.collections.get(collection).map_or(Described::Absent, |stored| stored.label.clone()))
    }

    fn recreate(&mut self, collection: &str, meta: &CollectionMeta) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.down {
            bail!("the vector store is down");
        }
        state.recreated.push((collection.into(), meta.clone()));
        state
            .collections
            .insert(collection.into(), Stored { label: Described::Labelled(meta.clone()), points: BTreeMap::new() });
        Ok(())
    }

    fn ids(&mut self, collection: &str) -> Result<BTreeSet<String>> {
        let state = self.state.lock().unwrap();
        if state.down {
            bail!("the vector store is down");
        }
        Ok(state.collections.get(collection).map(|stored| stored.points.keys().cloned().collect()).unwrap_or_default())
    }

    fn upsert(&mut self, collection: &str, points: &[Point]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.down {
            bail!("the vector store is down");
        }
        state.upserts.push(points.iter().map(|point| point.id.clone()).collect());
        let Some(stored) = state.collections.get_mut(collection) else { bail!("no collection {collection}") };
        for point in points {
            stored.points.insert(point.id.clone(), point.clone());
        }
        Ok(())
    }
}

/// A working embedder and vector store, for tests whose subject is elsewhere.
pub(crate) fn pair() -> (FakeEmbedder, FakeIndex) {
    (FakeEmbedder::new("d1"), FakeIndex::default())
}
