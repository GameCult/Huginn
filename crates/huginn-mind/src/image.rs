//! The mind's one decoded, indexed image: a pure fold of the commit receipts
//! in ordinal order. It is built only by replaying the receipts and advanced
//! only by the function the replay calls, so a held image and a fresh fold of
//! the same store are equal by construction. Nothing here is stored: a restart
//! re-folds the store, which is the truth.
//!
//! A row is the only decoded copy of its document. Status is derived through
//! the closing resolutions at an `as_of` and is never stored in a document;
//! a row holds the head's answer and `status_changed` says from which
//! snapshot that answer holds. The postings, closers, citers and kind maps
//! only narrow which rows a read hands the substrate: `universe` is a superset
//! of what `select` can match, and `select` still decides membership, order,
//! cursors and edges.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cultcache_rs::DatabaseEntry;
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, PipelineResolution, ResolutionOutcome, Short};

use crate::docs::{Docs, citations, kind_of_id, kind_of_type};
use crate::mind::Envelopes;
use crate::persona::HuginnPersonaEntry;
use crate::query::{AdmissionFacts, AdmissionIndex, PipelineDocumentView, PipelineStatus};
use crate::receipt::HuginnCommitReceipt;
use crate::refusal::MindRefusal;
use crate::rows::{Alias, STATUS_ALIASES, SelectionRow, root_and_local};

type Postings = BTreeMap<PipelineKind, BTreeMap<&'static str, HashMap<String, BTreeSet<u32>>>>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Image {
    /// The newest receipt's ordinal: `N` of a dense `1..=N`.
    pub(crate) head: u64,
    /// Append-only; a row's index is its id. Receipts' order, then identity order within one.
    pub(crate) rows: Vec<SelectionRow>,
    /// Per row, the newest ordinal at which its status changed.
    pub(crate) status_changed: Vec<u64>,
    /// Kind, then key, to row id, in key order: the image's identity lookup and its per-kind order.
    by_kind: BTreeMap<PipelineKind, BTreeMap<String, u32>>,
    /// (kind, alias, value) to the rows answering it.
    postings: Postings,
    /// Subject kind and id to the resolutions naming it, by resolution key.
    closers: BTreeMap<PipelineKind, HashMap<String, BTreeMap<String, u32>>>,
    /// Target id to the (role, citer) edges that name it.
    citers: HashMap<String, BTreeSet<(&'static str, u32)>>,
    /// Root to the newest row written under it.
    roots: BTreeMap<String, u32>,
    /// Whether every citation edge names a target of its role's kinds. When one does not, a hop
    /// selection reads every row, so the substrate refuses it exactly as it would over the whole mind.
    pub(crate) edges_in_target: bool,
}

fn integrity(type_id: &str, key: &str, fault: &str) -> MindRefusal {
    MindRefusal::Unavailable { detail: format!("document {type_id}/{key} {fault}") }
}

pub(crate) fn status_of(closing: Option<(&str, &PipelineResolution)>) -> PipelineStatus {
    match closing {
        Some((key, record)) => PipelineStatus::Resolved {
            resolution: PipelineRef { kind: PipelineKind::Resolution, id: Short(key.to_string()) },
            record: record.clone(),
        },
        None => PipelineStatus::InForce,
    }
}

impl Image {
    pub(crate) fn empty() -> Self {
        Self {
            head: 0,
            rows: Vec::new(),
            status_changed: Vec::new(),
            by_kind: BTreeMap::new(),
            postings: BTreeMap::new(),
            closers: BTreeMap::new(),
            citers: HashMap::new(),
            roots: BTreeMap::new(),
            edges_in_target: true,
        }
    }

    /// The fold of a store's envelopes. Faults, in order: a pipeline document that does not decode,
    /// the receipts (undecodable, not dense, a document written twice), a document no receipt wrote.
    /// There is no other way to make an image: this replays the receipts through `advance`.
    pub(crate) fn build(
        envelopes: &Envelopes,
        receipts: &Result<Vec<HuginnCommitReceipt>, MindRefusal>,
    ) -> Result<Self, MindRefusal> {
        let mut decoded: HashMap<(&str, &str), PipelineDocument> = HashMap::new();
        for (type_id, keyed) in envelopes {
            if kind_of_type(type_id).is_none() {
                continue;
            }
            for (key, envelope) in keyed {
                let document = PipelineDocument::decode(envelope).map_err(|refusal| MindRefusal::Unavailable {
                    detail: format!("stored document {key} does not decode: {refusal}"),
                })?;
                decoded.insert((type_id.as_str(), key.as_str()), document);
            }
        }
        let receipts = receipts.clone()?;
        let index = AdmissionIndex::build(&receipts)?;
        for (type_id, keyed) in envelopes {
            if kind_of_type(type_id).is_some()
                && let Some(key) = keyed.keys().find(|key| !index.contains(type_id, key))
            {
                return Err(integrity(type_id, key, "has no commit receipt"));
            }
        }
        let mut ordered: Vec<&HuginnCommitReceipt> = receipts.iter().collect();
        ordered.sort_by_key(|receipt| receipt.ordinal);
        let mut image = Self::empty();
        for receipt in ordered {
            let mut documents = Vec::new();
            for write in receipt.writes.iter().filter(|write| write.document_type != HuginnPersonaEntry::TYPE) {
                let Some(kind) = kind_of_type(&write.document_type) else { continue };
                // A receipted document the store no longer holds gets no row; read_back names it.
                if let Some(document) = decoded.remove(&(write.document_type.as_str(), write.document_key.as_str())) {
                    documents.push((kind, write.document_key.clone(), document));
                }
            }
            image.advance(&AdmissionFacts::of(receipt), documents)?;
        }
        Ok(image)
    }

    pub(crate) fn id_of(&self, kind: PipelineKind, key: &str) -> Option<u32> {
        self.by_kind.get(&kind)?.get(key).copied()
    }

    pub(crate) fn row(&self, id: u32) -> &SelectionRow {
        &self.rows[id as usize]
    }

    /// The ids of one kind in key order.
    pub(crate) fn of_kind(&self, kind: PipelineKind) -> impl Iterator<Item = (&str, u32)> {
        self.by_kind.get(&kind).into_iter().flat_map(|keyed| keyed.iter().map(|(key, id)| (key.as_str(), *id)))
    }

    /// The resolutions naming one subject, in resolution-key order.
    pub(crate) fn closers_of(&self, kind: PipelineKind, id: &str) -> impl Iterator<Item = (&str, u32)> {
        self.closers.get(&kind).and_then(|named| named.get(id)).into_iter().flat_map(|keyed| keyed.iter().map(|(key, id)| (key.as_str(), *id)))
    }

    /// Whether any document under `root` was admitted natively rather than imported.
    pub(crate) fn root_holds_native_write(&self, root: &str) -> bool {
        PipelineKind::ALL.iter().any(|kind| {
            self.posting(*kind, Alias::Root.name(), root)
                .into_iter()
                .flatten()
                .any(|id| self.row(*id).view.admission.provenance.imported_from.is_none())
        })
    }

    /// Every root with the newest row under it.
    pub(crate) fn roots(&self) -> impl Iterator<Item = (&str, &SelectionRow)> {
        self.roots.iter().map(|(root, id)| (root.as_str(), self.row(*id)))
    }

    /// Folds one receipt's documents, given in the receipt's write order, into the image. The same
    /// function replays the log and follows each commit.
    pub(crate) fn advance(
        &mut self,
        facts: &AdmissionFacts,
        documents: Vec<(PipelineKind, String, PipelineDocument)>,
    ) -> Result<(), MindRefusal> {
        let first = self.rows.len() as u32;
        // Phase 1: every document of the receipt gets a row, so a subject that sorts after its
        // resolution in one batch is found by phase 2.
        for (kind, key, document) in documents {
            if self.id_of(kind, &key).is_some() {
                return Err(integrity(kind.type_id(), &key, "is written by two receipts"));
            }
            let id = self.rows.len() as u32;
            let edges = citations(&document);
            for (role, target) in &edges {
                let in_target = kind_of_id(&target.id.0).is_some_and(|named| role.target_kinds().contains(&named));
                self.edges_in_target &= in_target;
                self.citers.entry(target.id.0.clone()).or_default().insert((role.name(), id));
            }
            if let PipelineDocument::Resolution(resolution) = &document {
                self.closers
                    .entry(resolution.subject.kind)
                    .or_default()
                    .entry(resolution.subject.id.0.clone())
                    .or_default()
                    .insert(key.clone(), id);
            }
            self.roots.insert(root_and_local(&key).0.to_string(), id);
            self.by_kind.entry(kind).or_default().insert(key.clone(), id);
            let view = PipelineDocumentView {
                id: PipelineRef { kind, id: Short(key.clone()) },
                document,
                admission: facts.clone(),
                status: PipelineStatus::InForce,
            };
            self.rows.push(SelectionRow::new(kind, &key, None, view, edges));
            self.status_changed.push(facts.ordinal);
        }
        self.head = facts.ordinal;
        // Phase 2: a new resolution changes its subject's status, and a subject that is itself a
        // resolution reopens or closes its own subject in turn, so the chain is walked to the first
        // document that is not a resolution.
        let mut touched: BTreeSet<u32> = (first..self.rows.len() as u32).collect();
        for id in first..self.rows.len() as u32 {
            let mut current = id;
            while let PipelineDocument::Resolution(resolution) = &self.row(current).view.document {
                let Some(subject) = self.id_of(resolution.subject.kind, &resolution.subject.id.0) else { break };
                touched.insert(subject);
                current = subject;
            }
        }
        // A Moved closes every document under its campaign's root, none of which it names.
        let moved: Vec<String> = (first..self.rows.len() as u32)
            .filter_map(|id| match &self.row(id).view.document {
                PipelineDocument::Resolution(PipelineResolution { subject, outcome: ResolutionOutcome::Moved { .. }, .. }) => {
                    Some(root_and_local(&subject.id.0).0.to_string())
                }
                _ => None,
            })
            .collect();
        for root in &moved {
            for kind in PipelineKind::ALL {
                touched.extend(self.posting(*kind, Alias::Root.name(), root).into_iter().flatten().copied());
            }
        }
        for id in touched {
            self.rebuild(id, id >= first, facts.ordinal);
        }
        Ok(())
    }

    /// The row with `status`, its base walked through the subjects held.
    pub(crate) fn make_row(&self, id: u32, status: PipelineStatus) -> SelectionRow {
        let old = self.row(id);
        let mut view = old.view.clone();
        view.status = status;
        let mut current = id;
        while let PipelineDocument::Resolution(resolution) = &self.row(current).view.document {
            let Some(subject) = self.id_of(resolution.subject.kind, &resolution.subject.id.0) else { break };
            current = subject;
        }
        let base = self.row(current);
        SelectionRow::new(old.kind, &old.key, Some((&base.key, &base.view.document)), view, citations(&old.view.document))
    }

    fn rebuild(&mut self, id: u32, fresh: bool, ordinal: u64) {
        let (kind, key) = {
            let row = self.row(id);
            (row.kind, row.key.clone())
        };
        let status = status_of(Docs::at(self, u64::MAX).closing_resolution(kind, &key));
        let row = self.make_row(id, status);
        if !fresh {
            self.unindex(id);
        }
        if row.view.status != self.row(id).view.status {
            self.status_changed[id as usize] = ordinal;
        }
        self.rows[id as usize] = row;
        self.index(id);
    }

    fn index(&mut self, id: u32) {
        let row = &self.rows[id as usize];
        for (alias, values) in row.alias_values() {
            for value in values {
                self.postings
                    .entry(row.kind)
                    .or_default()
                    .entry(alias)
                    .or_default()
                    .entry(value.clone())
                    .or_default()
                    .insert(id);
            }
        }
    }

    fn unindex(&mut self, id: u32) {
        let row = &self.rows[id as usize];
        for (alias, values) in row.alias_values() {
            for value in values {
                if let Some(posting) = self.postings.get_mut(&row.kind).and_then(|aliases| aliases.get_mut(alias)).and_then(|values| values.get_mut(value)) {
                    posting.remove(&id);
                }
            }
        }
    }

    fn posting(&self, kind: PipelineKind, alias: &str, value: &str) -> Option<&BTreeSet<u32>> {
        self.postings.get(&kind)?.get(alias)?.get(value)
    }

    /// The row ids a selection can match at `as_of`, ascending, with those written after `as_of`
    /// among them for the caller to drop: a superset of what `select` matches, never a verdict.
    pub(crate) fn universe(&self, selection: &Selection, as_of: u64) -> Vec<u32> {
        let hop = selection.has_hop();
        if hop && !self.edges_in_target {
            return (0..self.rows.len() as u32).collect();
        }
        let hop_only_cites = selection.cites.is_some() && selection.cited.is_none();
        let base = if hop_only_cites { BTreeSet::new() } else { self.base_candidates(selection, as_of) };
        let mut ids: BTreeSet<u32> = BTreeSet::new();
        let citers_of = |key: &str, role: Option<&str>| -> Vec<u32> {
            self.citers
                .get(key)
                .into_iter()
                .flatten()
                .filter(|(named, _)| role.is_none_or(|role| role == *named))
                .map(|(_, citer)| *citer)
                .collect()
        };
        match (&selection.cites, &selection.cited) {
            (None, None) => ids = base,
            (Some(cites), None) => {
                let key = cites.target.record_key.as_str();
                // A row that passes the hop cites the target, so the citers are the superset; the kinds,
                // keys and fields only the substrate decides.
                ids.extend(citers_of(key, cites.role.as_deref()));
                ids.extend(self.any_kind(key));
            }
            (cites, Some(cited)) => {
                for id in &base {
                    ids.extend(citers_of(&self.row(*id).key, Some(&cited.role)));
                }
                if let Some(cites) = cites {
                    ids.extend(self.any_kind(&cites.target.record_key));
                }
                ids.extend(base);
            }
        }
        ids.into_iter().collect()
    }

    fn any_kind(&self, key: &str) -> Vec<u32> {
        match kind_of_id(key) {
            Some(kind) => self.id_of(kind, key).into_iter().collect(),
            None => PipelineKind::ALL.iter().filter_map(|kind| self.id_of(*kind, key)).collect(),
        }
    }

    /// The smallest set the kinds, keys and any_of fields of a selection allow.
    fn base_candidates(&self, selection: &Selection, as_of: u64) -> BTreeSet<u32> {
        if let Some(keys) = &selection.keys {
            return keys.iter().flat_map(|key| self.any_kind(key)).collect();
        }
        let kinds: Vec<PipelineKind> = match &selection.schemas {
            Some(schemas) => schemas.iter().filter_map(|schema| kind_of_type(schema)).collect(),
            None => PipelineKind::ALL.to_vec(),
        };
        let mut out = BTreeSet::new();
        for kind in kinds {
            let mut best: Option<(usize, Vec<&BTreeSet<u32>>)> = None;
            for field in selection.fields.iter().flatten() {
                if field.op != "any_of" {
                    continue;
                }
                let Some(alias) = Alias::from_name(&field.index) else { continue };
                if STATUS_ALIASES.contains(&alias) && as_of != self.head {
                    continue;
                }
                let sets: Vec<&BTreeSet<u32>> =
                    field.values.iter().flatten().filter_map(|value| self.posting(kind, alias.name(), value)).collect();
                let size = sets.iter().map(|set| set.len()).sum();
                if best.as_ref().is_none_or(|(smallest, _)| size < *smallest) {
                    best = Some((size, sets));
                }
            }
            let whole = self.by_kind.get(&kind).map_or(0, BTreeMap::len);
            match best {
                Some((size, sets)) if size < whole => out.extend(sets.into_iter().flatten().copied()),
                _ => out.extend(self.by_kind.get(&kind).into_iter().flat_map(|keyed| keyed.values().copied())),
            }
        }
        out
    }
}

#[cfg(test)]
impl Image {
    /// An image of hand-made documents, one receipt each in the order given.
    pub(crate) fn of_documents(documents: Vec<(PipelineKind, String, PipelineDocument)>) -> Self {
        let mut image = Self::empty();
        for (n, document) in documents.into_iter().enumerate() {
            let facts = AdmissionFacts {
                receipt_id: format!("receipt-{}", n + 1),
                admitted_at: "2026-09-16T12:00:00Z".into(),
                provenance: crate::receipt::PipelineProvenance {
                    faculty: crate::receipt::Faculty::Hands,
                    agent: Short("claude".into()),
                    session: Short("session-1".into()),
                    tool: Short("admit".into()),
                    imported_from: None,
                },
                ordinal: n as u64 + 1,
            };
            image.advance(&facts, vec![document]).unwrap();
        }
        image
    }
}
