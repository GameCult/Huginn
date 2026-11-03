//! The read side's definition, frozen: decode every stored document and every
//! receipt on each call, derive status by a linear recursive scan, build every
//! row, and let the substrate select over all of them. It shares nothing with
//! the held `Image` or with `Docs`, so a test that sets the two side by side
//! compares two derivations, not one with itself. No production consumer.
//!
//! Also the synthetic history the floor test scales a store with.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{TimeZone, Utc};
use cultcache_rs::{CultCacheEnvelope, DatabaseEntry};
use cultnet_rs::{Selection, select, validate};
use eureka_pipeline::{
    CodeLocation, CommitRange, Date, Evidence, EvidenceKind, FindingConfidence, FindingOrigin, FindingSeverity, Label, MutationRecord,
    PipelineCutReport, PipelineDocument, PipelineFinding, PipelineKind, PipelineRef, PipelineResolution, ReportCommit, ResolutionOutcome, Sha,
    Line, Para, Short, Slug, StructuralDelta, pipeline_key,
};

use crate::docs::{citations, kind_of_type};
use crate::mind::{Mind, schema_cache};
use crate::persona::HuginnPersonaEntry;
use crate::query::{AdmissionFacts, PipelineDocumentView, PipelineSelectionPage, PipelineStatus};
use crate::receipt::{self, Faculty, HuginnCommitReceipt, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::rows::{SelectionRow, Vocabulary, refuse_values};
use crate::store::MindStore;

struct Doc {
    kind: PipelineKind,
    key: String,
    document: PipelineDocument,
    facts: AdmissionFacts,
}

/// The mind as the store holds it, derived from nothing but the store.
struct Fresh {
    docs: Vec<Doc>,
}

fn fault(detail: String) -> MindRefusal {
    MindRefusal::Unavailable { detail }
}

impl Fresh {
    fn pull<S: MindStore>(mind: &Mind<S>) -> Result<Self, MindRefusal> {
        let mut raw = mind.store().pull_all().map_err(|error| fault(format!("{error:#}")))?;
        raw.sort_by(|left, right| (&left.r#type, &left.key).cmp(&(&right.r#type, &right.key)));
        let mut held = Vec::new();
        for envelope in &raw {
            let Some(kind) = kind_of_type(&envelope.r#type) else { continue };
            let document = PipelineDocument::decode(envelope)
                .map_err(|refusal| fault(format!("stored document {} does not decode: {refusal}", envelope.key)))?;
            held.push((kind, envelope.key.clone(), document));
        }
        let mut receipts = Vec::new();
        for envelope in raw.iter().filter(|envelope| envelope.r#type == HuginnCommitReceipt::TYPE) {
            receipts.push(rmp_serde::from_slice::<HuginnCommitReceipt>(&envelope.payload).map_err(|error| {
                fault(format!("receipt {} does not decode: {error}", envelope.key))
            })?);
        }
        let mut ordinals: Vec<u64> = receipts.iter().map(|receipt| receipt.ordinal).collect();
        ordinals.sort_unstable();
        if !ordinals.iter().copied().eq(1..=ordinals.len() as u64) {
            return Err(fault("receipt ordinals are not 1..=N: found a duplicate or a gap".into()));
        }
        let mut facts: BTreeMap<(String, String), AdmissionFacts> = BTreeMap::new();
        for receipt in &receipts {
            for write in receipt.writes.iter().filter(|write| write.document_type != HuginnPersonaEntry::TYPE) {
                let landed = AdmissionFacts {
                    receipt_id: receipt.receipt_id.clone(),
                    admitted_at: receipt.committed_at.clone(),
                    provenance: receipt.provenance.clone(),
                    ordinal: receipt.ordinal,
                };
                if facts.insert((write.document_type.clone(), write.document_key.clone()), landed).is_some() {
                    return Err(fault(format!("document {}/{} is written by two receipts", write.document_type, write.document_key)));
                }
            }
        }
        let mut docs = Vec::new();
        for (kind, key, document) in held {
            let type_id = kind.type_id();
            let landed = facts
                .get(&(type_id.to_string(), key.clone()))
                .cloned()
                .ok_or_else(|| fault(format!("document {type_id}/{key} has no commit receipt")))?;
            docs.push(Doc { kind, key, document, facts: landed });
        }
        Ok(Self { docs })
    }

    fn visible(&self, as_of: u64) -> Vec<&Doc> {
        self.docs.iter().filter(|doc| doc.facts.ordinal <= as_of).collect()
    }
}

/// The recursive derivation, by linear scan over the visible documents.
fn closing<'a>(visible: &[&'a Doc], kind: PipelineKind, key: &str) -> Option<(&'a str, &'a PipelineResolution)> {
    visible.iter().find_map(|doc| match &doc.document {
        PipelineDocument::Resolution(resolution)
            if resolution.subject.kind == kind
                && resolution.subject.id.0 == key
                && closing(visible, PipelineKind::Resolution, &doc.key).is_none() =>
        {
            Some((doc.key.as_str(), resolution))
        }
        _ => None,
    })
}

fn view_of(visible: &[&Doc], doc: &Doc) -> PipelineDocumentView {
    let status = match closing(visible, doc.kind, &doc.key) {
        Some((key, record)) => PipelineStatus::Resolved {
            resolution: PipelineRef { kind: PipelineKind::Resolution, id: Short(key.to_string()) },
            record: record.clone(),
        },
        None => PipelineStatus::InForce,
    };
    PipelineDocumentView {
        id: PipelineRef { kind: doc.kind, id: Short(doc.key.clone()) },
        document: doc.document.clone(),
        admission: doc.facts.clone(),
        status,
    }
}

fn base<'a>(visible: &[&'a Doc], doc: &'a Doc) -> &'a Doc {
    let mut current = doc;
    while let PipelineDocument::Resolution(resolution) = &current.document {
        let Some(subject) = visible.iter().find(|other| other.kind == resolution.subject.kind && other.key == resolution.subject.id.0) else {
            return current;
        };
        current = subject;
    }
    current
}

fn row_of(visible: &[&Doc], doc: &Doc) -> SelectionRow {
    let base = base(visible, doc);
    SelectionRow::new(doc.kind, &doc.key, Some((&base.key, &base.document)), view_of(visible, doc), citations(&doc.document))
}

/// The page a selection earns at `as_of`, derived from the store alone.
pub fn oracle_page<S: MindStore>(mind: &Mind<S>, selection: &Selection, as_of: u64) -> Result<PipelineSelectionPage, MindRefusal> {
    validate(selection, &Vocabulary).map_err(cultnet_rs::SelectionRefusal::Invalid)?;
    let selection = refuse_values(selection)?;
    let fresh = Fresh::pull(mind)?;
    let visible = fresh.visible(as_of);
    let rows: Vec<SelectionRow> = visible.iter().map(|doc| row_of(&visible, doc)).collect();
    let rows: Vec<&SelectionRow> = rows.iter().collect();
    let evaluation = select(&Vocabulary, &rows, &selection, as_of, mind.cursor_key())?;
    Mind::<S>::page(&selection, evaluation, as_of, None)
}

/// The view of one document at `as_of`, derived from the store alone.
pub fn oracle_view<S: MindStore>(
    mind: &Mind<S>,
    id: &PipelineRef,
    as_of: u64,
) -> Result<Option<PipelineDocumentView>, MindRefusal> {
    let fresh = Fresh::pull(mind)?;
    let visible = fresh.visible(as_of);
    Ok(visible.iter().find(|doc| doc.kind == id.kind && doc.key == id.id.0).map(|doc| view_of(&visible, doc)))
}

/// Every pipeline identity the store holds, in the snapshot's order.
pub fn oracle_ids<S: MindStore>(mind: &Mind<S>, as_of: u64) -> Result<Vec<PipelineRef>, MindRefusal> {
    let fresh = Fresh::pull(mind)?;
    Ok(fresh.visible(as_of).iter().map(|doc| PipelineRef { kind: doc.kind, id: Short(doc.key.clone()) }).collect())
}

// ---------------------------------------------------------------------------------------------
// The synthetic history
// ---------------------------------------------------------------------------------------------

const BATCH_RECEIPTS: usize = 700;

fn pad(text: &str, bytes: usize) -> String {
    let mut out = text.to_string();
    while out.len() < bytes {
        out.push_str(" padding for payload size");
    }
    out.truncate(bytes.max(text.len()));
    out
}

fn range() -> CommitRange {
    CommitRange { base: Sha("5f98228d".into()), head: Sha("a1b2c3d4".into()) }
}

fn finding(root: &str, unit: usize, n: usize) -> PipelineDocument {
    PipelineDocument::Finding(PipelineFinding {
        campaign: Slug(root.into()),
        verdict: Short(format!("{root}:verdict:cut-u{unit}.s1")),
        label: Label(format!("f{unit}-{n}")),
        range: range(),
        confidence: FindingConfidence::Confirmed,
        severity: FindingSeverity::Medium,
        claim: Line(pad("A synthetic claim.", 500)),
        invariants: vec![Label("synthetic".into())],
        locations: vec![CodeLocation { path: Short("crates/huginn-mind/src/query.rs".into()), line: 1, end_line: None, symbol: None }],
        failure_scenario: Para(pad("A synthetic failure scenario.", 700)),
        evidence: vec![Evidence { kind: EvidenceKind::Test, locator: "cargo test".into(), result: "ok".into() }],
        precedents: vec![],
        origin: FindingOrigin::Introduced,
    })
}

fn cut_report(root: &str, unit: usize) -> PipelineDocument {
    let delta = StructuralDelta {
        lines_added: 0,
        lines_removed: 0,
        dependencies_added: vec![],
        dependencies_removed: vec![],
        formats_added: vec![],
        formats_removed: vec![],
        targets_added: vec![],
        targets_removed: vec![],
    };
    PipelineDocument::CutReport(PipelineCutReport {
        campaign: Slug(root.into()),
        cut_spec: Short(format!("{root}:cut_spec:cut-u{unit}.r1")),
        attempt: 1,
        repo: eureka_pipeline::OrgRepo("GameCult/Huginn".into()),
        branch: Short("hands/synthetic".into()),
        commits: vec![ReportCommit { sha: Sha("5f98228d".into()), subject: "Land the cut".into(), builds: true }],
        range: range(),
        verification: (0..8)
            .map(|n| Evidence { kind: EvidenceKind::Command, locator: Line(format!("command {n}")), result: Line(pad("passed", 1000)) })
            .collect(),
        mutations: vec![MutationRecord {
            label: Label("M1".into()),
            rule: "the rule".into(),
            location: CodeLocation { path: Short("crates/huginn-mind/src/query.rs".into()), line: 1, end_line: None, symbol: None },
            before: "before".into(),
            after: "after".into(),
            commit: Sha("5f98228d".into()),
            failed_as_expected: true,
        }],
        deviations: vec![],
        forks: vec![],
        structural_delta: delta,
        landed_names: vec![],
        undone: vec![],
        promises: vec![],
    })
}

fn resolution(subject: PipelineRef) -> PipelineDocument {
    PipelineDocument::Resolution(PipelineResolution {
        subject,
        sequence: 1,
        outcome: ResolutionOutcome::Recorded { reason: "Synthetic.".into() },
        rationale: Para(pad("Resolved.", 100)),
        resolved_on: Date("2026-10-01".into()),
    })
}

/// Grows a store to `factor` times its pipeline documents with synthetic
/// history: findings, cut reports and the resolutions that close four records
/// in five, about three writes a receipt, through the store's own
/// compare-and-swap in dense receipts. `spread` puts each hundred units under
/// its own `synth-<n>` root; otherwise everything goes under
/// `eureka-substrate`, one root growing. Returns the documents written.
pub fn synth_history<S: MindStore>(store: &S, instance: &Slug, factor: u32, spread: bool) -> Result<usize, MindRefusal> {
    let pulled = store.pull_all().map_err(|error| fault(format!("{error:#}")))?;
    let existing = pulled.iter().filter(|envelope| kind_of_type(&envelope.r#type).is_some()).count();
    let mut ordinal = pulled.iter().filter(|envelope| envelope.r#type == HuginnCommitReceipt::TYPE).count() as u64;
    let want = existing * factor.saturating_sub(1) as usize;
    // A unit is five records (four findings, one cut report) and the four
    // resolutions that close four of them.
    let units = want / 9;
    let cache = schema_cache()?;
    let provenance = PipelineProvenance {
        faculty: Faculty::Hands,
        agent: Short("synth".into()),
        session: Short("synth".into()),
        tool: Short("synth".into()),
    };
    let now = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
    let mut pending: Vec<Vec<CultCacheEnvelope>> = Vec::new();
    let mut written = 0;
    let flush = |groups: &mut Vec<Vec<CultCacheEnvelope>>, ordinal: &mut u64| -> Result<(), MindRefusal> {
        if groups.is_empty() {
            return Ok(());
        }
        let mut replacements = Vec::new();
        for writes in groups.drain(..) {
            *ordinal += 1;
            let receipt = receipt::candidate(instance, provenance.clone(), &[], &writes, *ordinal, now)?;
            let (envelope, _) = cache.prepare_entry_named(&receipt.receipt_id, &receipt).map_err(|error| fault(format!("{error:#}")))?;
            replacements.extend(writes);
            replacements.push(envelope);
        }
        if !store.compare_and_swap_batch(&[], replacements).map_err(|error| fault(format!("{error:#}")))? {
            return Err(fault("the synthetic batch lost its swap".into()));
        }
        Ok(())
    };
    let prepare = |document: &PipelineDocument| -> Result<(PipelineRef, CultCacheEnvelope), MindRefusal> {
        let key = pipeline_key(document)?;
        let envelope = document.prepare(&cache).map_err(|error| fault(format!("{error:#}")))?;
        Ok((PipelineRef { kind: document.kind(), id: Short(key) }, envelope))
    };
    for unit in 0..units {
        let root = if spread { format!("synth-{}", unit / 100) } else { "eureka-substrate".to_string() };
        let mut records = Vec::new();
        for n in 0..4 {
            records.push(prepare(&finding(&root, unit, n))?);
        }
        records.push(prepare(&cut_report(&root, unit))?);
        let mut closers = Vec::new();
        for (subject, _) in records.iter().take(4) {
            closers.push(prepare(&resolution(subject.clone()))?);
        }
        let mut all = records.into_iter().chain(closers).map(|(_, envelope)| envelope).collect::<Vec<_>>();
        written += all.len();
        let tail = all.split_off(6);
        let middle = all.split_off(3);
        pending.extend([all, middle, tail]);
        if pending.len() >= BATCH_RECEIPTS {
            flush(&mut pending, &mut ordinal)?;
        }
    }
    flush(&mut pending, &mut ordinal)?;
    Ok(written)
}

/// Opens the store at `path` under its exclusive lock, grows it, and lets go
/// of the lock, so a `Mind::open` can follow.
pub fn synth_history_at(path: &Path, instance: &Slug, factor: u32, spread: bool) -> Result<usize, MindRefusal> {
    let store = crate::store::OwnedRedbMessagePackBackingStore::new(path).map_err(|error| fault(format!("{error:#}")))?;
    synth_history(&store, instance, factor, spread)
}
