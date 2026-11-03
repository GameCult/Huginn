//! The held image against the oracle. The oracle derives every answer from the store alone, per
//! call, by the linear derivation the read side had before it held an image; these tests set the
//! two side by side over a history with withdrawal chains, a re-resolution, hand-offs, a run and
//! its closure, at every snapshot, across a restart, and across the commit paths that do not
//! land. They fail when a held answer differs from a fresh one, not when a particular structure
//! changes.

use std::collections::BTreeSet;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use anyhow::{Result, anyhow};
use cultcache_rs::{CacheBackingStore, CultCacheEnvelope, DatabaseEntry, PushAllOptions};
use cultnet_rs::{Citation, FieldPredicate, Incoming, RecordRef, Row, Selection};
use eureka_pipeline::{ClaimOutcome, FindingConfidence, PipelineDocument as D, PipelineKind as K, RunTurn, pipeline_key};

use crate::docs::{CitationRole, kind_of_id};
use crate::fixtures::*;
use crate::image::Image;
use crate::mind::{Mind, schema_cache};
use crate::oracle::{oracle_ids, oracle_page, oracle_view};
use crate::query::{AdmissionIndex, PipelineSelectionPage, Reader};
use crate::receipt::{self, Faculty, HuginnCommitReceipt};
use crate::refusal::MindRefusal;
use crate::rows::{Alias, STATUS_ALIASES, SelectionRow};
use crate::store::MindStore;
use crate::store::test_stores::{MemoryStore, RefusingStore, SwapCommand};

fn answering(label: &str, question: &str) -> D {
    let mut ruling = ruling(label);
    ruling.answers = Some(s(question));
    ruling.choice = Some(l("A"));
    D::Ruling(ruling)
}

fn key_of(document: &D) -> String {
    pipeline_key(document).unwrap()
}

/// The batches of a history, each with the faculty that admits it.
fn history() -> Vec<(Faculty, Vec<D>)> {
    let spec = |cut: &str, revision: u32| id("cut_spec", &format!("cut-{cut}.r{revision}"));
    let run_a = mind_run("a", RunTurn::SelfRun, &[r(K::CutSpec, &spec("1", 1))]);
    let run_key = key_of(&run_a);
    let mut back = match hand_off(OTHER_INSTANCE, INSTANCE, REPO, &[]) {
        D::HandOff(back) => back,
        _ => unreachable!(),
    };
    back.handed_on = eureka_pipeline::Date("2026-09-17".into());
    vec![
        (Faculty::Hands, vec![
            question("Q1", &["A", "B"], "A"),
            D::Ruling(ruling("R1")),
            D::CutSpec(cut_spec("1", 1)),
            D::CutSpec(cut_spec("9", 1)),
            D::CutSpec(cut_spec("10", 1)),
            follow_up("FU-1", r(K::Question, &id("question", "Q1"))),
        ]),
        (Faculty::Hands, vec![D::CutReport(cut_report("9", 1)), D::CutReport(cut_report("10", 1))]),
        (Faculty::Hands, vec![
            verdict("9", 1, vec![claim(ClaimOutcome::Falsified, &[&id("finding", "cut-9.s1.F1")], Some("P1"), &["M1"])]),
            D::Finding(finding("9", 1, "F1", FindingConfidence::Confirmed)),
        ]),
        // A resolution that sorts before its subject's superseder in the batch it arrives in.
        (Faculty::Hands, vec![
            D::CutSpec(cut_spec("9", 2)),
            resolution(r(K::CutSpec, &spec("9", 1)), superseded(&[r(K::CutSpec, &spec("9", 2))])),
        ]),
        // The withdrawal of a resolution, and its withdrawal: the spec closes and reopens.
        (Faculty::Hands, vec![resolution(r(K::CutSpec, &spec("10", 1)), withdrawn())]),
        (Faculty::Hands, vec![resolution(r(K::Resolution, &id("resolution", "cut_spec.cut-10.r1.n1")), withdrawn())]),
        (Faculty::Soul, vec![D::Ruling(ruling("R2"))]),
        (Faculty::Hands, vec![answering("R3", &id("question", "Q1"))]),
        // A hand-off away and back: a stewardship closes, its closure is withdrawn, a new one opens.
        (Faculty::Hands, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]),
        (Faculty::Hands, vec![D::HandOff(back)]),
        // A run that claims a spec, then is withdrawn unrun.
        (Faculty::Hands, vec![run_a]),
        (Faculty::Hands, vec![resolution(r(K::Run, &run_key), withdrawn())]),
        (Faculty::Life, vec![question_n(2)]),
    ]
}

fn apply<S: MindStore>(mind: &mut Mind<S>, faculty: Faculty, documents: Vec<D>) {
    committed(admit_as(mind, faculty, documents));
}

fn history_mind() -> Mind<MemoryStore> {
    let mut mind = seeded();
    for (faculty, documents) in history() {
        apply(&mut mind, faculty, documents);
    }
    mind
}

fn held_equals_fresh<S: MindStore>(mind: &Mind<S>) {
    let held = mind.image().unwrap();
    let fresh = mind.fresh_image().unwrap();
    assert!(*held == fresh, "the held image differs from a fresh fold at head {}", held.head);
}

fn of_kinds(kinds: &[K]) -> Selection {
    Selection { schemas: Some(kinds.iter().map(|kind| kind.type_id().to_string()).collect()), ..Selection::default() }
}

fn any_of(index: &str, values: &[&str]) -> FieldPredicate {
    FieldPredicate { index: index.into(), op: "any_of".into(), values: Some(values.iter().map(|v| v.to_string()).collect()), number: None }
}

fn with(mut selection: Selection, predicate: FieldPredicate) -> Selection {
    selection.fields.get_or_insert_with(Vec::new).push(predicate);
    selection
}

fn all_rows<S: MindStore>(mind: &Mind<S>, as_of: u64) -> Vec<SelectionRow> {
    let mut reader = Reader::at(mind, as_of).unwrap();
    let ids = reader.universe(&Selection::default());
    reader.rows(&ids).into_iter().cloned().collect()
}

/// Selections that between them touch every kind, every alias value the history emits, keys, both
/// hops on every role with both `exists`, combinations, the other order and the smallest page.
fn corpus<S: MindStore>(mind: &Mind<S>) -> Vec<Selection> {
    let head = mind.image().unwrap().head;
    let rows = all_rows(mind, head);
    let mut out = vec![Selection::default()];
    for kind in K::ALL.iter().copied() {
        out.push(of_kinds(&[kind]));
        out.push(Selection { descending: true, limit: Some(1), ..of_kinds(&[kind]) });
        for alias in ["in_force", "root", "faculty"] {
            for value in ["true", "false", CAMPAIGN, "Hands"] {
                out.push(with(of_kinds(&[kind]), any_of(alias, &[value])));
            }
        }
    }
    let (mut pairs, mut edges) = (BTreeSet::new(), BTreeSet::new());
    for row in &rows {
        for alias in Alias::ALL {
            for value in Row::values(row, alias.name()) {
                pairs.insert((alias.name(), value));
            }
        }
        for (role, target, _) in Row::references(row) {
            edges.insert((role, target));
        }
        out.push(Selection { keys: Some(vec![row.key.clone()]), projection: "document".into(), ..Selection::default() });
    }
    out.push(Selection { keys: Some(vec!["nobody:question:nothing".into()]), ..Selection::default() });
    for (alias, value) in &pairs {
        out.push(with(Selection::default(), any_of(alias, &[value])));
    }
    for (role, target) in &edges {
        let schema = kind_of_id(target).map_or("epiphany.pipeline.question.v1".to_string(), |kind| kind.type_id().to_string());
        for role_filter in [Some(role.clone()), None] {
            let citation = Citation { target: RecordRef::new(schema.clone(), target.clone()), role: role_filter };
            out.push(Selection { cites: Some(citation.clone()), ..Selection::default() });
            out.push(with(Selection { cites: Some(citation), ..Selection::default() }, any_of("in_force", &["true"])));
        }
    }
    for role in CitationRole::ALL {
        for exists in [true, false] {
            let incoming = Incoming { role: role.name().into(), exists };
            out.push(Selection { cited: Some(incoming.clone()), ..Selection::default() });
            out.push(with(Selection { cited: Some(incoming.clone()), ..Selection::default() }, any_of("in_force", &["true"])));
            out.push(Selection { cited: Some(incoming), descending: true, limit: Some(1), ..of_kinds(&[K::CutSpec, K::Question]) });
        }
    }
    if let Some((role, target)) = edges.iter().next() {
        let citation = Citation { target: RecordRef::new(K::Question.type_id(), target.clone()), role: Some(role.clone()) };
        out.push(Selection { cites: Some(citation), cited: Some(Incoming { role: "subject".into(), exists: false }), ..Selection::default() });
    }
    out
}

/// One page, both ways, whole page or whole refusal.
fn agree<S: MindStore>(mind: &Mind<S>, selection: &Selection, as_of: u64) -> Result<PipelineSelectionPage, MindRefusal> {
    let held = mind.query_at(selection, as_of);
    let fresh = oracle_page(mind, selection, as_of);
    assert_eq!(held, fresh, "as_of {as_of}: {selection:?}");
    held
}

fn walk<S: MindStore>(mind: &Mind<S>, selection: &Selection, as_of: u64) {
    let mut selection = Selection { limit: Some(1), ..selection.clone() };
    let mut at = as_of;
    for _ in 0..80 {
        let Ok(page) = agree(mind, &selection, at) else { return };
        let Some(next) = page.next else { return };
        at = page.as_of;
        selection.cursor = Some(next);
    }
    panic!("a limit-1 walk did not end");
}

#[test]
fn held_image_equals_a_fresh_fold_after_every_commit() {
    let mut mind = seeded();
    held_equals_fresh(&mind);
    for (faculty, documents) in history() {
        apply(&mut mind, faculty, documents);
        held_equals_fresh(&mind);
    }
    let image = mind.image().unwrap();
    assert!(image.status_changed.iter().any(|at| *at > 1), "the history changes a status after the row was written");
    assert_eq!(image.head, history().len() as u64 + 1);
}

#[test]
fn every_read_agrees_with_the_oracle_at_every_snapshot() {
    let mind = history_mind();
    let head = mind.image().unwrap().head;
    let corpus = corpus(&mind);
    assert!(corpus.len() > 300, "the corpus is {}", corpus.len());
    let mut differs_from_head = 0;
    for as_of in 1..=head {
        for selection in &corpus {
            let _ = agree(&mind, selection, as_of);
        }
        let open = Selection { fields: Some(vec![any_of("in_force", &["true"])]), ..Selection::default() };
        if mind.query_at(&open, as_of).unwrap().matched != mind.query_at(&open, head).unwrap().matched {
            differs_from_head += 1;
        }
    }
    assert!(differs_from_head > 0, "a status reversed by a later withdrawal shows at an old snapshot");
    for kind in K::ALL.iter().copied() {
        walk(&mind, &of_kinds(&[kind]), head);
        walk(&mind, &of_kinds(&[kind]), head - 3);
    }
    walk(&mind, &Selection::default(), head);
    walk(&mind, &Selection { descending: true, ..Selection::default() }, 4);
}

#[test]
fn views_and_status_agree_with_the_oracle_at_every_snapshot() {
    let mind = history_mind();
    let head = mind.image().unwrap().head;
    let every = oracle_ids(&mind, head).unwrap();
    assert_eq!(every.len(), mind.image().unwrap().rows.len(), "the oracle names every document, or the loop below proves nothing");
    for as_of in 1..=head {
        let reader = Reader::at(&mind, as_of).unwrap();
        for id in &every {
            assert_eq!(reader.view(id).unwrap(), oracle_view(&mind, id, as_of).unwrap(), "{} at {as_of}", id.id.0);
        }
    }
    let spec = r(K::CutSpec, &id("cut_spec", "cut-10.r1"));
    assert_eq!(mind.view(&spec).unwrap().unwrap().status, crate::query::PipelineStatus::InForce, "the reopened spec stands at the head");
    let closed = Reader::at(&mind, 6).unwrap().view(&spec).unwrap().unwrap();
    assert!(matches!(closed.status, crate::query::PipelineStatus::Resolved { .. }), "and is closed while its withdrawal is the head");
    let reopened = Mind::open_with(mind.store().clone(), &slug(INSTANCE)).unwrap();
    assert_eq!(reopened.status(crate::wire::IndexStatus::Current), mind.status(crate::wire::IndexStatus::Current));
    assert_eq!(reopened.persona().unwrap(), mind.persona().unwrap());
}

#[test]
fn a_restart_answers_what_the_held_image_answered() {
    let dir = tempfile::tempdir().unwrap();
    let instance = slug(INSTANCE);
    let mut mind = Mind::open(dir.path(), &instance).unwrap();
    seed(&mut mind);
    for (faculty, documents) in history() {
        apply(&mut mind, faculty, documents);
    }
    let head = mind.image().unwrap().head;
    let corpus = corpus(&mind);
    // A cursor is minted under the process's own key, so the answers are compared without it.
    let plain = |page: Result<PipelineSelectionPage, MindRefusal>| page.map(|page| PipelineSelectionPage { next: None, ..page });
    let answers: Vec<_> = corpus.iter().map(|selection| plain(mind.query_at(selection, head))).collect();
    let cursor = mind.query(&Selection { limit: Some(1), ..of_kinds(&[K::Question]) }).unwrap().next.unwrap();
    let held = mind.image().unwrap().clone();
    drop(mind);
    let reopened = Mind::open(dir.path(), &instance).unwrap();
    assert!(*reopened.image().unwrap() == held, "the image a restart folds is the image the holder held");
    for (selection, before) in corpus.iter().zip(&answers) {
        assert_eq!(&plain(reopened.query_at(selection, head)), before, "{selection:?}");
    }
    let stale = Selection { limit: Some(1), cursor: Some(cursor), ..of_kinds(&[K::Question]) };
    assert!(matches!(reopened.query(&stale), Err(MindRefusal::CursorInvalid { .. })), "a cursor does not survive a restart");
}

#[test]
fn a_lost_swap_rebuilds_from_what_the_store_holds() {
    let store = MemoryStore::new();
    let mut holder = opened(store.clone(), INSTANCE);
    seed(&mut holder);
    let mut other = Mind::open_with(store.clone(), &slug(INSTANCE)).unwrap();
    committed(admit(&mut other, vec![question_n(7)]));
    let planted = r(K::Question, &id("question", "Q7"));
    assert!(holder.view(&planted).unwrap().is_none(), "without a lost swap the plant stays unseen until restart: the stated contract");
    assert_eq!(Mind::open_with(store.clone(), &slug(INSTANCE)).unwrap().view(&planted).unwrap().map(|v| v.id), Some(planted.clone()));
    let outcome = admit(&mut holder, vec![question_n(7)]);
    assert!(matches!(outcome, crate::admission::PipelineAdmissionOutcome::Conflict { .. }), "{outcome:?}");
    held_equals_fresh(&holder);
    assert!(holder.view(&planted).unwrap().is_some(), "the lost swap looked at the store");
    committed(admit(&mut holder, vec![question_n(8)]));
    held_equals_fresh(&holder);
}

/// A store that lands the swap and then reports an error, so the holder cannot know.
#[derive(Clone)]
struct LandsThenFails {
    inner: MemoryStore,
    armed: Arc<AtomicBool>,
}

impl CacheBackingStore for LandsThenFails {
    fn pull_all(&self) -> Result<Vec<CultCacheEnvelope>> {
        self.inner.pull_all()
    }
    fn push(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
        self.inner.push(entry)
    }
    fn delete(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
        self.inner.delete(entry)
    }
    fn push_all(&mut self, entries: &[CultCacheEnvelope], options: PushAllOptions) -> Result<()> {
        self.inner.push_all(entries, options)
    }
}

impl MindStore for LandsThenFails {
    fn compare_and_swap_batch(&self, expected: &[CultCacheEnvelope], replacements: Vec<CultCacheEnvelope>) -> Result<bool> {
        let landed = self.inner.compare_and_swap_batch(expected, replacements)?;
        if self.armed.swap(false, Ordering::SeqCst) {
            return Err(anyhow!("the store failed after landing"));
        }
        Ok(landed)
    }
}

#[test]
fn a_store_error_during_commit_rebuilds() {
    let inner = MemoryStore::new();
    let armed = Arc::new(AtomicBool::new(false));
    let mut mind = opened(LandsThenFails { inner: inner.clone(), armed: armed.clone() }, INSTANCE);
    seed(&mut mind);
    armed.store(true, Ordering::SeqCst);
    let outcome = admit(&mut mind, vec![question_n(1)]);
    assert!(matches!(outcome, crate::admission::PipelineAdmissionOutcome::Refused(MindRefusal::Unavailable { .. })), "{outcome:?}");
    held_equals_fresh(&mind);
    assert!(mind.view(&r(K::Question, &id("question", "Q1"))).unwrap().is_some(), "the batch that landed is held");
    committed(admit(&mut mind, vec![question_n(2)]));
    held_equals_fresh(&mind);

    let refusing = RefusingStore::new();
    let mut mind = opened(refusing.clone(), INSTANCE);
    seed(&mut mind);
    refusing.command(SwapCommand::Fail);
    let before = mind.image().unwrap().clone();
    assert!(matches!(admit(&mut mind, vec![question_n(1)]), crate::admission::PipelineAdmissionOutcome::Refused(_)));
    held_equals_fresh(&mind);
    assert!(*mind.image().unwrap() == before, "a store that did not land leaves the image as it was");
    refusing.command(SwapCommand::Delegate);
    committed(admit(&mut mind, vec![question_n(1)]));
    held_equals_fresh(&mind);
}

/// The rows of a healthy two-step mind, to be broken one way at a time.
fn healthy_rows() -> Vec<CultCacheEnvelope> {
    let mut mind = seeded();
    committed(admit(&mut mind, vec![question_n(1)]));
    mind.store().rows()
}

fn receipt_of(rows: &[CultCacheEnvelope], ordinal: u64) -> HuginnCommitReceipt {
    rows.iter()
        .filter(|row| row.r#type == HuginnCommitReceipt::TYPE)
        .map(|row| rmp_serde::from_slice::<HuginnCommitReceipt>(&row.payload).unwrap())
        .find(|receipt| receipt.ordinal == ordinal)
        .unwrap()
}

fn faulted_mind(rows: Vec<CultCacheEnvelope>) -> (Mind<MemoryStore>, MemoryStore) {
    let store = MemoryStore::new();
    rows.into_iter().for_each(|row| store.plant(row));
    (Mind::open_with(store.clone(), &slug(INSTANCE)).unwrap(), store)
}

#[test]
fn a_faulted_store_refuses_alike_at_every_snapshot() {
    let cache = schema_cache().unwrap();
    let mut cases: Vec<(&str, Vec<CultCacheEnvelope>)> = Vec::new();

    let mut rows = healthy_rows();
    let at = rows.iter().position(|row| row.key == id("question", "Q1")).unwrap();
    rows[at].payload = vec![0xc1];
    cases.push(("an undecodable document", rows));

    let mut rows = healthy_rows();
    let at = rows.iter().position(|row| row.r#type == HuginnCommitReceipt::TYPE).unwrap();
    rows[at].payload = vec![0xc1];
    cases.push(("an undecodable receipt", rows));

    let mut rows = healthy_rows();
    let gone = receipt_of(&rows, 1).receipt_id;
    rows.retain(|row| row.key != gone);
    cases.push(("an undense chain", rows));

    let mut rows = healthy_rows();
    let orphan = prepare(&question_n(5));
    rows.push(orphan);
    cases.push(("an orphan", rows));

    let mut rows = healthy_rows();
    let mut twice = receipt_of(&rows, 2);
    twice.ordinal = 3;
    twice.receipt_id = "mind-commit-twice".into();
    rows.push(cache.prepare_entry_named(&twice.receipt_id, &twice).unwrap().0);
    cases.push(("a document written by two receipts", rows));

    for (name, rows) in cases {
        let (mind, store) = faulted_mind(rows);
        let before = store.rows();
        let probe = of_kinds(&[K::Question]);
        let seen = mind.query_at(&probe, 1);
        let fault = seen.clone().expect_err(name);
        for as_of in [1, 2, 5] {
            assert_eq!(mind.query_at(&probe, as_of), seen, "{name} at {as_of}");
            assert_eq!(oracle_page(&mind, &probe, as_of).expect_err(name), fault, "{name}: the oracle names the same fault");
        }
        assert_eq!(mind.query(&probe), seen, "{name}: query answers the held fault");
        assert_eq!(mind.view(&r(K::Question, &id("question", "Q1"))), Err(fault.clone()), "{name}");
        let outcome = admit(&mut Mind::open_with(store.clone(), &slug(INSTANCE)).unwrap(), vec![question_n(9)]);
        assert!(matches!(outcome, crate::admission::PipelineAdmissionOutcome::Refused(ref refusal) if *refusal == fault), "{name}: admit refuses with the fault, {outcome:?}");
        assert_eq!(store.rows(), before, "{name}: a refused admission writes nothing");
        let status = mind.status(crate::wire::IndexStatus::Current);
        assert_eq!(status.persona, crate::wire::PersonaStatus::Absent, "{name}: status stays total");
        assert_eq!(mind.persona().unwrap(), None, "{name}");
    }
}

#[test]
fn an_edge_outside_its_target_falls_back_to_the_whole_universe() {
    let mut rows = healthy_rows();
    let cache = schema_cache().unwrap();
    // A run claims a question: the role claims cut specs, findings and follow-ups. Admission would
    // refuse it, so it is planted with its receipt.
    let planted = mind_run("planted", RunTurn::SelfRun, &[r(K::Question, &id("question", "Q1"))]);
    let envelope = planted.prepare(&cache).unwrap();
    let mut receipt = receipt::candidate(&slug(INSTANCE), provenance(Faculty::Hands), &[], std::slice::from_ref(&envelope), 3, now()).unwrap();
    receipt.receipt_id = receipt.digest().unwrap();
    rows.push(cache.prepare_entry_named(&receipt.receipt_id, &receipt).unwrap().0);
    rows.push(envelope);
    let (mind, _) = faulted_mind(rows);
    assert!(!mind.image().unwrap().edges_in_target, "the planted edge is outside its role's kinds");
    let healthy = seeded();
    assert!(healthy.image().unwrap().edges_in_target);
    let refused = MindRefusal::from(cultnet_rs::SelectionRefusal::ReferenceOutsideTarget {
        from_schema_id: K::Run.type_id().into(),
        from_key: key_of(&planted),
        role: "claims".into(),
        to_schema_id: K::Question.type_id().into(),
        to_key: id("question", "Q1"),
    });
    let elsewhere = Citation { target: RecordRef::new(K::CutSpec.type_id(), id("cut_spec", "cut-1.r1")), role: Some("claims".into()) };
    for selection in [
        Selection { cited: Some(Incoming { role: "claims".into(), exists: true }), ..Selection::default() },
        Selection { cites: Some(elsewhere), ..Selection::default() },
    ] {
        assert_eq!(agree(&mind, &selection, 3), Err(refused.clone()), "the substrate refuses the edge exactly as it would over the whole mind");
    }
}

#[test]
fn status_aliases_are_the_declared_ones() {
    let mut mind = seeded();
    let mut before = all_rows(&mind, 1);
    let mut status_changes = 0;
    for (faculty, documents) in history() {
        apply(&mut mind, faculty, documents);
        let after = all_rows(&mind, mind.image().unwrap().head);
        for (old, new) in before.iter().zip(&after) {
            if old.view.status != new.view.status {
                status_changes += 1;
            }
            for alias in Alias::ALL {
                if !STATUS_ALIASES.contains(&alias) {
                    assert_eq!(Row::values(old, alias.name()), Row::values(new, alias.name()), "{} changed under a status change", alias.name());
                }
            }
        }
        before = after;
    }
    assert!(status_changes >= 3, "the history changes statuses ({status_changes})");
}

#[test]
fn read_back_refuses_a_held_image_that_differs_from_a_fresh_fold() {
    let store = MemoryStore::new();
    let mut holder = opened(store.clone(), INSTANCE);
    seed(&mut holder);
    assert_eq!(holder.read_back().unwrap(), 4, "a clean store counts every document");
    let mut other = Mind::open_with(store, &slug(INSTANCE)).unwrap();
    committed(admit(&mut other, vec![question_n(1)]));
    let refused = holder.read_back();
    assert!(matches!(&refused, Err(MindRefusal::Unavailable { detail }) if detail.contains("held image differs from a fresh fold")), "{refused:?}");
    assert_eq!(other.read_back().unwrap(), 5);
}

#[test]
fn the_admission_index_names_what_receipts_wrote() {
    let mind = history_mind();
    let index = AdmissionIndex::build(&mind.receipts().unwrap()).unwrap();
    let image: &Image = mind.image().unwrap();
    for row in &image.rows {
        assert!(index.contains(row.kind.type_id(), &row.key));
    }
}

/// A store whose swap can land then err, be lost, and whose next pull can fail: a look that fails
/// after a store fault.
#[derive(Clone)]
struct FailsTheLook {
    inner: MemoryStore,
    land_then_err: Arc<AtomicBool>,
    lose: Arc<AtomicBool>,
    pull_err: Arc<AtomicBool>,
}

impl CacheBackingStore for FailsTheLook {
    fn pull_all(&self) -> Result<Vec<CultCacheEnvelope>> {
        if self.pull_err.swap(false, Ordering::SeqCst) {
            return Err(anyhow!("the look failed"));
        }
        self.inner.pull_all()
    }
    fn push(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
        self.inner.push(entry)
    }
    fn delete(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
        self.inner.delete(entry)
    }
    fn push_all(&mut self, entries: &[CultCacheEnvelope], options: PushAllOptions) -> Result<()> {
        self.inner.push_all(entries, options)
    }
}

impl MindStore for FailsTheLook {
    fn compare_and_swap_batch(&self, expected: &[CultCacheEnvelope], replacements: Vec<CultCacheEnvelope>) -> Result<bool> {
        if self.lose.swap(false, Ordering::SeqCst) {
            return Ok(false);
        }
        let landed = self.inner.compare_and_swap_batch(expected, replacements)?;
        if self.land_then_err.swap(false, Ordering::SeqCst) {
            return Err(anyhow!("the store failed after landing"));
        }
        Ok(landed)
    }
}

fn fails_the_look() -> (Mind<FailsTheLook>, FailsTheLook) {
    let store = FailsTheLook { inner: MemoryStore::new(), land_then_err: Default::default(), lose: Default::default(), pull_err: Default::default() };
    let mut mind = opened(store.clone(), INSTANCE);
    seed(&mut mind);
    (mind, store)
}

/// One rule for a failed look: the fault is held, reads and admission refuse with it, nothing is
/// written, and a restart holds what the store holds and admits at the next ordinal.
fn assert_fault_is_held(mind: &mut Mind<FailsTheLook>, store: &FailsTheLook, landed: &[&str]) {
    assert!(matches!(mind.image(), Err(MindRefusal::Unavailable { .. })));
    assert!(matches!(mind.query(&of_kinds(&[K::Question])), Err(MindRefusal::Unavailable { .. })));
    let rows = store.inner.rows();
    let next = admit(mind, vec![question_n(50)]);
    assert!(matches!(next, crate::admission::PipelineAdmissionOutcome::Refused(MindRefusal::Unavailable { .. })), "{next:?}");
    assert_eq!(store.inner.rows(), rows, "a held fault writes nothing");
    let mut reopened = opened(store.clone(), INSTANCE);
    held_equals_fresh(&reopened);
    assert_eq!(reopened.query(&of_kinds(&[K::Question])).unwrap().matched, landed.len() as u32);
    committed(admit(&mut reopened, vec![question_n(51)]));
    held_equals_fresh(&reopened);
    reopened.read_back().unwrap();
}

#[test]
fn a_failed_look_after_a_landed_error_holds_the_fault() {
    let (mut mind, store) = fails_the_look();
    store.land_then_err.store(true, Ordering::SeqCst);
    store.pull_err.store(true, Ordering::SeqCst);
    let first = admit(&mut mind, vec![question_n(1)]);
    assert!(matches!(first, crate::admission::PipelineAdmissionOutcome::Refused(MindRefusal::Unavailable { .. })), "{first:?}");
    assert_fault_is_held(&mut mind, &store, &["Q1"]);
}

#[test]
fn a_failed_look_after_a_lost_swap_holds_the_fault() {
    let (mut mind, store) = fails_the_look();
    let mut other = opened(store.clone(), INSTANCE);
    committed(admit(&mut other, vec![question_n(7)]));
    store.lose.store(true, Ordering::SeqCst);
    store.pull_err.store(true, Ordering::SeqCst);
    let lost = admit(&mut mind, vec![question_n(8)]);
    assert!(matches!(lost, crate::admission::PipelineAdmissionOutcome::Refused(MindRefusal::Unavailable { .. })), "{lost:?}");
    assert_fault_is_held(&mut mind, &store, &["Q7"]);
}

// ---- the floor: what a read and a landed commit may cost, counted ----

/// Reads and a landed admission never pull the store: a pull is a decode of the whole mind.
#[test]
fn reads_and_landed_admissions_pull_the_store_zero_times() {
    let mut mind = history_mind();
    let pulls = mind.store().pull_count();
    let head = mind.image().unwrap().head;
    mind.query(&Selection::default()).unwrap();
    mind.query_at(&of_kinds(&[K::Question]), head / 2).unwrap();
    mind.view(&r(K::Question, &id("question", "Q1"))).unwrap();
    let _ = mind.status(crate::wire::IndexStatus::Current);
    let _ = mind.persona();
    assert_eq!(mind.store().pull_count(), pulls, "a read pulled the store");
    committed(admit(&mut mind, vec![question_n(99)]));
    committed(admit(&mut mind, vec![resolution(r(K::Question, &id("question", "Q99")), withdrawn())]));
    assert_eq!(mind.store().pull_count(), pulls, "a landed swap pulled the store");
    held_equals_fresh(&mind);
}

/// A lost swap and a store error each look at the store exactly once.
#[test]
fn a_lost_swap_and_a_store_error_pull_the_store_once_each() {
    let (mut mind, store) = fails_the_look();
    let mut other = opened(store.clone(), INSTANCE);
    committed(admit(&mut other, vec![question_n(7)]));
    let pulls = store.inner.pull_count();
    store.lose.store(true, Ordering::SeqCst);
    admit(&mut mind, vec![question_n(8)]);
    assert_eq!(store.inner.pull_count(), pulls + 1, "a lost swap looks once");
    store.land_then_err.store(true, Ordering::SeqCst);
    admit(&mut mind, vec![question_n(9)]);
    assert_eq!(store.inner.pull_count(), pulls + 2, "a store error looks once");
}

/// A history of many questions, one of them the only Soul's, one follow-up citing Q1.
fn many_rows_mind() -> Mind<MemoryStore> {
    let mut mind = seeded();
    for chunk in (1..=40).collect::<Vec<u32>>().chunks(10) {
        committed(admit(&mut mind, chunk.iter().map(|n| question_n(*n)).collect()));
    }
    committed(admit_as(&mut mind, Faculty::Soul, vec![question_n(41)]));
    committed(admit(&mut mind, vec![follow_up("FU-1", r(K::Question, &id("question", "Q1")))]));
    mind
}

/// The universe is the narrowing: a selection that names a key, a rare field value or a cited
/// target hands the match only its candidates, never the mind.
#[test]
fn a_selection_reads_its_candidates_and_not_the_mind() {
    let mind = many_rows_mind();
    let image = mind.image().unwrap();
    let (head, total) = (image.head, image.rows.len());
    assert!(total > 40);
    assert_eq!(image.universe(&Selection::default(), head).len(), total, "the unfiltered selection is the whole mind");
    let keyed = Selection { keys: Some(vec![id("question", "Q3")]), ..Selection::default() };
    assert_eq!(image.universe(&keyed, head).len(), 1, "keys");
    let rare = with(of_kinds(&[K::Question]), any_of("faculty", &["Soul"]));
    assert_eq!(image.universe(&rare, head).len(), 1, "a rare field value");
    assert_eq!(image.universe(&of_kinds(&[K::Question]), head).len(), 41, "a kind is its own rows");
    let citation = Citation { target: RecordRef::new(K::Question.type_id(), id("question", "Q1")), role: None };
    let citers = Selection { cites: Some(citation), ..Selection::default() };
    let universe = image.universe(&citers, head);
    assert!(universe.len() <= 3 && !universe.is_empty(), "a hop starts from the target's citers: {}", universe.len());
    assert!(total > 10 * universe.len());
    assert!(mind.query(&citers).unwrap().matched >= 1);
}

// ---- the narrowing lines the oracle corpus must reach ----

/// Both hops with the citer's schema excluding the target's: the target row is handed to the
/// match only by the line that adds the cites target to a both-hop universe.
#[test]
fn both_hops_with_a_schema_excluding_the_target_agree() {
    let mind = history_mind();
    let head = mind.image().unwrap().head;
    let rows = all_rows(&mind, head);
    let mut edges = BTreeSet::new();
    for row in &rows {
        for (role, target, _) in Row::references(row) {
            edges.insert((row.kind, role, target));
        }
    }
    let (mut pages, mut hits) = (0, 0);
    for (carrier, role, target) in &edges {
        let schema = kind_of_id(target).map_or(K::Question.type_id().to_string(), |kind| kind.type_id().to_string());
        let citation = Citation { target: RecordRef::new(schema, target.clone()), role: Some(role.clone()) };
        for exists in [true, false] {
            let incoming = Incoming { role: "subject".into(), exists };
            let plain = Selection { cites: Some(citation.clone()), cited: Some(incoming.clone()), ..of_kinds(&[*carrier]) };
            let narrowed = with(Selection { projection: "document".into(), ..plain.clone() }, any_of("in_force", &["true"]));
            for as_of in [head, head / 2] {
                for selection in [&plain, &narrowed] {
                    if let Ok(page) = agree(&mind, selection, as_of) {
                        pages += 1;
                        hits += usize::from(page.matched > 0);
                    }
                }
            }
        }
    }
    assert!(pages > 0 && hits > 0, "no both-hop selection with a schema matched anything ({pages} pages)");
}

/// A resolution withdrawn by another: the Resolution schema excludes the cut spec it cites.
#[test]
fn a_both_hop_selection_finds_a_citer_whose_target_the_schema_excludes() {
    let mind = history_mind();
    let selection = Selection {
        cites: Some(Citation { target: RecordRef::new(K::CutSpec.type_id(), id("cut_spec", "cut-10.r1")), role: None }),
        cited: Some(Incoming { role: "subject".into(), exists: true }),
        ..of_kinds(&[K::Resolution])
    };
    let head = mind.image().unwrap().head;
    let page = agree(&mind, &selection, head).unwrap();
    assert_eq!(page.matched, 1, "{page:?}");
}

fn planted_rows(rows: &mut Vec<CultCacheEnvelope>, ordinal: u64, document: &D) {
    let cache = schema_cache().unwrap();
    let envelope = document.prepare(&cache).unwrap();
    let mut receipt = receipt::candidate(&slug(INSTANCE), provenance(Faculty::Hands), &[], std::slice::from_ref(&envelope), ordinal, now()).unwrap();
    receipt.receipt_id = receipt.digest().unwrap();
    rows.push(cache.prepare_entry_named(&receipt.receipt_id, &receipt).unwrap().0);
    rows.push(envelope);
}

/// Two closures in force over one subject, the later sequence arriving first. Admission refuses
/// this; a store that holds it must still answer as the oracle does, closers in key order.
#[test]
fn planted_double_closers_keep_key_order() {
    let q = r(K::Question, &id("question", "Q1"));
    let mut rows = healthy_rows();
    planted_rows(&mut rows, 3, &resolution_n(q.clone(), 2, withdrawn()));
    planted_rows(&mut rows, 4, &resolution_n(q.clone(), 1, withdrawn()));
    let (mind, _) = faulted_mind(rows);
    for as_of in 1..=4 {
        assert_eq!(Reader::at(&mind, as_of).unwrap().view(&q).unwrap(), oracle_view(&mind, &q, as_of).unwrap(), "at {as_of}");
    }
}

// ---- Hands' own gaps ----

/// A lost swap names the identities that moved beneath it: a written key already stored, a read
/// document that changed, and not a read document that is the same.
#[test]
fn a_conflict_names_the_identities_that_changed() {
    let (mut mind, store) = fails_the_look();
    let mut other = opened(store.clone(), INSTANCE);
    committed(admit(&mut other, vec![question_n(7)]));
    store.lose.store(true, Ordering::SeqCst);
    let outcome = admit(&mut mind, vec![question_n(7)]);
    let crate::admission::PipelineAdmissionOutcome::Conflict { identities } = outcome else { panic!("{outcome:?}") };
    assert_eq!(identities, vec![r(K::Question, &id("question", "Q7"))], "the written key that was already stored");

    let cited = |mind: &mut Mind<FailsTheLook>| {
        let mut spec = cut_spec("1", 1);
        spec.rulings = vec![s(&id("ruling", "R1"))];
        admit(mind, vec![D::CutSpec(spec)])
    };
    committed(admit(&mut mind, vec![D::Ruling(ruling("R1"))]));
    store.lose.store(true, Ordering::SeqCst);
    let outcome = cited(&mut mind);
    let crate::admission::PipelineAdmissionOutcome::Conflict { identities } = outcome else { panic!("{outcome:?}") };
    assert!(identities.is_empty(), "a read document that did not change is not named: {identities:?}");

    let mut bent = ruling("R1");
    bent.ruling = "A ruling that moved beneath the holder.".into();
    store.inner.plant(prepare(&D::Ruling(bent)));
    store.lose.store(true, Ordering::SeqCst);
    let outcome = cited(&mut mind);
    let crate::admission::PipelineAdmissionOutcome::Conflict { identities } = outcome else { panic!("{outcome:?}") };
    assert_eq!(identities, vec![r(K::Ruling, &id("ruling", "R1"))], "the read document that changed");
}
