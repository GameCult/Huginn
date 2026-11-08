//! A campaign moving between minds: the giver's Moved resolution freezes its root and closes
//! everything under it, and the receiver admits the history as an import that names its source.
//! Each test fails when the rule it names breaks, and reads behaviour through the doors a caller
//! uses: admission, `view`, `query`.

use cultnet_rs::{FieldPredicate, Selection};
use eureka_pipeline::{
    ClaimOutcome, FindingConfidence, PipelineDocument as D, PipelineKind as K, PipelineRef, ResolutionOutcome, pipeline_key,
};

use crate::admission::{PipelineAdmissionBatch, PipelineAdmissionOutcome};
use crate::fixtures::*;
use crate::mind::Mind;
use crate::query::{PipelinePageItems, PipelineStatus, Reader};
use crate::receipt::{Faculty, HuginnCommitReceipt, IMPORT_EXTERNAL_MAX, ImportSource, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::store::test_stores::MemoryStore;

const RECEIVER: &str = "heimdall";
const OTHER_ROOT: &str = "other-campaign";

fn campaign_ref() -> PipelineRef {
    r(K::Campaign, &id("campaign", "self"))
}

fn moved_to(mind: &str) -> D {
    resolution(campaign_ref(), ResolutionOutcome::Moved { to: slug(mind) })
}

fn moved_key(mind: &str) -> String {
    pipeline_key(&moved_to(mind)).unwrap()
}

fn answering(campaign: &str, label: &str, question: &str) -> D {
    let mut ruling = ruling(label);
    ruling.campaign = slug(campaign);
    ruling.answers = Some(s(question));
    ruling.choice = Some(l("A"));
    D::Ruling(ruling)
}

/// A question of another campaign, which this mind holds without holding the campaign.
fn other_question(label: &str) -> D {
    let mut question = question(label, &["A", "B"], "A");
    let D::Question(value) = &mut question else { unreachable!() };
    value.campaign = slug(OTHER_ROOT);
    question
}

fn other_follow_up(label: &str, question: &str) -> D {
    let mut follow_up = follow_up(label, r(K::Question, &format!("{OTHER_ROOT}:question:{question}")));
    let D::FollowUp(value) = &mut follow_up else { unreachable!() };
    value.campaign = slug(OTHER_ROOT);
    follow_up
}

/// The campaign of `OTHER_ROOT`, which a ruling of that root belongs to.
fn other_campaign() -> D {
    let D::Campaign(mut value) = campaign(&[REPO]) else { unreachable!() };
    value.slug = slug(OTHER_ROOT);
    D::Campaign(value)
}

fn view_of(mind: &Mind<MemoryStore>, kind: K, key: &str) -> crate::query::PipelineDocumentView {
    mind.view(&r(kind, key)).unwrap().unwrap_or_else(|| panic!("{key} is not held"))
}

fn head(mind: &Mind<MemoryStore>) -> u64 {
    mind.image().unwrap().head
}

fn moved_by(status: &PipelineStatus, moved: &str) -> bool {
    matches!(status, PipelineStatus::Resolved { resolution, record }
        if resolution.id.0 == moved && matches!(record.outcome, ResolutionOutcome::Moved { .. }))
}

fn open_follow_ups(mind: &Mind<MemoryStore>) -> Vec<String> {
    let in_force = FieldPredicate { index: "in_force".into(), op: "any_of".into(), values: Some(vec!["true".into()]), number: None };
    let selection = Selection {
        schemas: Some(vec![K::FollowUp.type_id().to_string()]),
        fields: Some(vec![in_force]),
        projection: "document".into(),
        ..Selection::default()
    };
    match mind.query(&selection).unwrap().items {
        PipelinePageItems::Documents(found) => found.into_iter().map(|view| view.id.id.0).collect(),
        PipelinePageItems::Headers(_) => panic!("the document projection answers documents"),
    }
}

fn receiver() -> Mind<MemoryStore> {
    let mut mind = opened(MemoryStore::new(), RECEIVER);
    committed(admit(&mut mind, vec![instance(RECEIVER), stewardship(RECEIVER, REPO)]));
    mind
}

fn source_of(instance: &str, external: Vec<PipelineRef>) -> ImportSource {
    ImportSource {
        instance: slug(instance),
        receipt_id: s("mind-commit-source"),
        ordinal: 5,
        committed_at: s("2026-11-08T16:20:05Z"),
        faculty: Faculty::Imagination,
        agent: s("imagination"),
        session: s("source-session"),
        tool: s("huginn"),
        external,
    }
}

fn import_as(mind: &mut Mind<MemoryStore>, source: ImportSource, documents: Vec<D>) -> PipelineAdmissionOutcome {
    let provenance = PipelineProvenance {
        faculty: Faculty::SelfFaculty,
        agent: s("heimdall-self"),
        session: s("import-1"),
        tool: s("huginn"),
        imported_from: Some(source),
    };
    let batch = PipelineAdmissionBatch { instance: mind.instance().clone(), provenance, documents };
    mind.admit(batch, now())
}

fn import(mind: &mut Mind<MemoryStore>, documents: Vec<D>, external: Vec<PipelineRef>) -> PipelineAdmissionOutcome {
    import_as(mind, source_of(INSTANCE, external), documents)
}

/// The giver of a whole campaign: a question and the ruling that answers it, a spec, its report, a
/// falsifying verdict with its finding, the finding fixed, and two follow-ups one of which is
/// recorded. Returns the mind and the ordinal of its last receipt before any move.
fn giver() -> (Mind<MemoryStore>, u64) {
    let mut mind = seeded();
    let q1 = id("question", "Q1");
    let finding = id("finding", "cut-1.s1.F1");
    committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), question("Q2", &["A", "B"], "A"), follow_up("FU-1", r(K::Question, &q1))]));
    committed(admit(&mut mind, vec![answering(CAMPAIGN, "R1", &q1)]));
    committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1))]));
    committed(admit(&mut mind, vec![D::CutReport(cut_report("1", 1))]));
    committed(admit(
        &mut mind,
        vec![
            verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&finding], Some("P1"), &["M1"])]),
            D::Finding(finding_doc()),
        ],
    ));
    committed(admit(&mut mind, vec![resolution(r(K::Finding, &finding), ResolutionOutcome::Fixed { commit: sha(), by: None })]));
    committed(admit(&mut mind, vec![follow_up("FU-2", r(K::Question, &q1))]));
    committed(admit(&mut mind, vec![resolution(r(K::FollowUp, &id("follow_up", "FU-2")), ResolutionOutcome::Recorded { reason: "noted".into() })]));
    let head = head(&mind);
    (mind, head)
}

fn finding_doc() -> eureka_pipeline::PipelineFinding {
    finding("1", 1, "F1", FindingConfidence::Confirmed)
}

fn root_of(key: &str) -> &str {
    key.split(':').next().unwrap()
}

/// Replays the giver's receipts up to `through`, in ordinal order, each filtered to the campaign's
/// root, into the receiver as imports that name their source receipt.
fn replay(giver: &Mind<MemoryStore>, receiver: &mut Mind<MemoryStore>, through: u64) -> Vec<PipelineAdmissionOutcome> {
    let mut receipts: Vec<HuginnCommitReceipt> = giver.receipts().unwrap();
    receipts.sort_by_key(|receipt| receipt.ordinal);
    let mut outcomes = Vec::new();
    for receipt in receipts.into_iter().filter(|receipt| receipt.ordinal <= through) {
        let documents: Vec<D> = receipt
            .writes
            .iter()
            .filter(|write| crate::docs::kind_of_type(&write.document_type).is_some() && root_of(&write.document_key) == CAMPAIGN)
            .map(|write| giver.get(crate::docs::kind_of_type(&write.document_type).unwrap(), &write.document_key).unwrap().unwrap())
            .collect();
        if documents.is_empty() {
            continue;
        }
        let source = ImportSource {
            instance: giver.instance().clone(),
            receipt_id: s(&receipt.receipt_id),
            ordinal: receipt.ordinal,
            committed_at: s(&receipt.committed_at),
            faculty: receipt.provenance.faculty,
            agent: receipt.provenance.agent.clone(),
            session: receipt.provenance.session.clone(),
            tool: receipt.provenance.tool.clone(),
            external: vec![],
        };
        outcomes.push(import_as(receiver, source, documents));
    }
    outcomes
}

#[test]
fn moved_campaign_refuses_every_later_write_under_its_root() {
    let mut mind = seeded();
    let q1 = id("question", "Q1");
    let q0 = id("question", "Q0");
    committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), question("Q0", &["A", "B"], "A")]));
    // A ruling of another root answers a question of this one: the derived resolution is rooted here.
    let (_, writes) = committed(admit(&mut mind, vec![answering(OTHER_ROOT, "R0", &q0)]));
    assert_eq!(writes.len(), 2, "before the move, the ruling and its derived Answered land: {writes:?}");

    committed(admit(&mut mind, vec![moved_to(RECEIVER)]));
    let frozen_at = head(&mind);
    let frozen = |outcome: PipelineAdmissionOutcome| {
        assert_eq!(refusal(outcome), MindRefusal::CampaignMoved { campaign: CAMPAIGN.into(), to: RECEIVER.into() });
    };
    frozen(admit(&mut mind, vec![follow_up("FU-1", r(K::Question, &q1))]));
    frozen(admit(&mut mind, vec![resolution(r(K::Question, &q1), withdrawn())]));
    frozen(admit(&mut mind, vec![answering(OTHER_ROOT, "R1", &q1)]));
    frozen(admit(&mut mind, vec![moved_to(RECEIVER)]));
    frozen(admit(&mut mind, vec![moved_to("kiko")]));
    assert_eq!(head(&mind), frozen_at, "a refused write leaves nothing behind");

    committed(admit(&mut mind, vec![other_question("Q5")]));
    assert_eq!(head(&mind), frozen_at + 1, "a write under another root lands");
}

#[test]
fn moved_rows_read_resolved_by_the_move_and_keep_their_own_closer() {
    let (mut mind, _) = giver();
    let moved = moved_key(RECEIVER);
    let finding = id("finding", "cut-1.s1.F1");
    let fixed = id("resolution", "finding.cut-1.s1.F1.n1");
    committed(admit(&mut mind, vec![other_question("Q5")]));
    committed(admit(&mut mind, vec![moved_to(RECEIVER)]));

    for (kind, key) in [
        (K::Question, id("question", "Q2")),
        (K::FollowUp, id("follow_up", "FU-1")),
        (K::Campaign, id("campaign", "self")),
        (K::Ruling, id("ruling", "R1")),
    ] {
        let status = view_of(&mind, kind, &key).status;
        assert!(moved_by(&status, &moved), "{key} reads {status:?}");
    }
    let answered = view_of(&mind, K::Question, &id("question", "Q1")).status;
    assert!(matches!(&answered, PipelineStatus::Resolved { record, .. } if matches!(record.outcome, ResolutionOutcome::Answered { .. })), "{answered:?}");
    let PipelineStatus::Resolved { resolution, record } = view_of(&mind, K::Finding, &finding).status else {
        panic!("the fixed finding stays resolved")
    };
    assert_eq!(resolution.id.0, fixed, "its own closer, not the move");
    assert!(matches!(record.outcome, ResolutionOutcome::Fixed { .. }));
    let recorded = view_of(&mind, K::FollowUp, &id("follow_up", "FU-2")).status;
    assert!(matches!(&recorded, PipelineStatus::Resolved { record, .. } if matches!(record.outcome, ResolutionOutcome::Recorded { .. })), "{recorded:?}");

    assert_eq!(view_of(&mind, K::Resolution, &moved).status, PipelineStatus::InForce, "the Moved resolution stands");
    assert_eq!(view_of(&mind, K::Question, &format!("{OTHER_ROOT}:question:Q5")).status, PipelineStatus::InForce);
    assert_eq!(view_of(&mind, K::Instance, &format!("{INSTANCE}:instance:self")).status, PipelineStatus::InForce);
}

#[test]
fn a_read_before_the_move_still_sees_in_force() {
    let (mut mind, _) = giver();
    let moved = moved_key(RECEIVER);
    let fixed = r(K::Finding, &id("finding", "cut-1.s1.F1"));
    committed(admit(&mut mind, vec![moved_to(RECEIVER)]));
    let landed = head(&mind);

    let before = Reader::at(&mind, landed - 1).unwrap();
    let follow_up = r(K::FollowUp, &id("follow_up", "FU-1"));
    assert_eq!(before.view(&follow_up).unwrap().unwrap().status, PipelineStatus::InForce);
    assert_eq!(before.view(&r(K::Campaign, &id("campaign", "self"))).unwrap().unwrap().status, PipelineStatus::InForce);
    assert!(before.view(&r(K::Resolution, &moved)).unwrap().is_none(), "the Moved resolution did not exist yet");
    assert!(matches!(before.view(&fixed).unwrap().unwrap().status, PipelineStatus::Resolved { .. }), "a closer of its own reads as it did");
    assert!(moved_by(&Reader::at(&mind, landed).unwrap().view(&follow_up).unwrap().unwrap().status, &moved));
    assert!(moved_by(&mind.view(&follow_up).unwrap().unwrap().status, &moved));
}

#[test]
fn in_force_follow_ups_leave_a_moved_campaign() {
    let mut mind = seeded();
    committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), follow_up("FU-1", r(K::Question, &id("question", "Q1")))]));
    committed(admit(&mut mind, vec![other_question("Q5"), other_follow_up("FU-9", "Q5")]));
    let mut both = open_follow_ups(&mind);
    both.sort();
    assert_eq!(both, vec![id("follow_up", "FU-1"), format!("{OTHER_ROOT}:follow_up:FU-9")]);

    committed(admit(&mut mind, vec![moved_to(RECEIVER)]));
    assert_eq!(open_follow_ups(&mind), vec![format!("{OTHER_ROOT}:follow_up:FU-9")], "the moved root's leads left, the other root's stayed");
}

#[test]
fn moved_only_closes_a_campaign_is_final_and_never_to_self() {
    let mut mind = seeded();
    let q1 = id("question", "Q1");
    committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), follow_up("FU-1", r(K::Question, &q1))]));
    let to = |mind: &str| ResolutionOutcome::Moved { to: slug(mind) };
    for (kind, key) in [(K::FollowUp, id("follow_up", "FU-1")), (K::Question, q1.clone())] {
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(r(kind, &key), to(RECEIVER))])),
            MindRefusal::IncompatibleResolution { subject_kind: kind, outcome: "Moved".into() }
        );
    }
    assert_eq!(refusal(admit(&mut mind, vec![resolution(campaign_ref(), to(INSTANCE))])), MindRefusal::MoveToSelf);
    assert!(mind.view(&r(K::Resolution, &moved_key(INSTANCE))).unwrap().is_none(), "nothing was written");

    committed(admit(&mut mind, vec![moved_to(RECEIVER)]));
    let withdrawal = resolution(r(K::Resolution, &moved_key(RECEIVER)), withdrawn());
    assert_eq!(refusal(admit(&mut mind, vec![withdrawal])), MindRefusal::MoveIsFinal);
    assert!(moved_by(&view_of(&mind, K::Question, &q1).status, &moved_key(RECEIVER)), "the move still closes the campaign");
}

#[test]
fn import_replays_a_receipt_without_derivation() {
    let mut mind = receiver();
    // A ruling of another root that answered this question in the giver: the derived Answered is
    // rooted at the question's campaign, so a replay of the ruling's root does not carry it. The
    // receiver does not derive it back.
    let q1 = id("question", "Q1");
    let ruling = answering(OTHER_ROOT, "R1", &q1);
    committed(import(&mut mind, vec![other_campaign()], vec![]));
    let before = head(&mind);
    let (_, writes) = committed(import(&mut mind, vec![ruling.clone()], vec![r(K::Question, &q1)]));
    assert_eq!(writes, vec![r(K::Ruling, &pipeline_key(&ruling).unwrap())], "only what the source wrote");
    assert_eq!(head(&mind), before + 1);
    assert!(mind.view(&r(K::Resolution, &id("resolution", "question.Q1.n1"))).unwrap().is_none(), "no second resolution");

    // The same shape through admission derives it, which is what import mode forgoes.
    let mut plain = receiver();
    committed(admit(&mut plain, vec![question("Q1", &["A", "B"], "A")]));
    let (_, writes) = committed(admit(&mut plain, vec![ruling]));
    assert_eq!(writes.len(), 2, "an ordinary admission derives the Answered: {writes:?}");
}

#[test]
fn import_admits_a_declared_external_and_refuses_an_undeclared_one() {
    let mut mind = receiver();
    committed(import(&mut mind, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A"), follow_up("FU-1", r(K::Question, &id("question", "Q1")))], vec![]));
    let report = r(K::CutReport, &format!("{OTHER_ROOT}:cut_report:cut-3.h1"));
    let fixed = resolution(
        r(K::FollowUp, &id("follow_up", "FU-1")),
        ResolutionOutcome::Fixed { commit: sha(), by: Some(report.clone()) },
    );
    assert_eq!(
        refusal(import(&mut mind, vec![fixed.clone()], vec![])),
        MindRefusal::MissingReference { kind: K::CutReport, id: report.id.0.clone() }
    );
    let (_, writes) = committed(import(&mut mind, vec![fixed.clone()], vec![report.clone()]));
    assert_eq!(writes.len(), 1);
    assert!(matches!(view_of(&mind, K::FollowUp, &id("follow_up", "FU-1")).status, PipelineStatus::Resolved { .. }));
    assert!(mind.view(&report).unwrap().is_none(), "an external is stored nowhere");

    // A listing names the referent it stands in for and no other of its kind.
    let mut elsewhere = receiver();
    committed(import(&mut elsewhere, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A"), follow_up("FU-1", r(K::Question, &id("question", "Q1")))], vec![]));
    let other = r(K::CutReport, &format!("{OTHER_ROOT}:cut_report:cut-4.h1"));
    assert_eq!(
        refusal(import(&mut elsewhere, vec![fixed.clone()], vec![other])),
        MindRefusal::MissingReference { kind: K::CutReport, id: report.id.0.clone() }
    );

    // Outside an import, the same document is refused: externals are an import's alone.
    let mut plain = receiver();
    committed(admit(&mut plain, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A"), follow_up("FU-1", r(K::Question, &id("question", "Q1")))]));
    assert!(matches!(refusal(admit(&mut plain, vec![fixed])), MindRefusal::MissingReference { .. }));
}

#[test]
fn import_refuses_a_present_external_a_moved_outcome_and_its_own_instance() {
    let mut mind = receiver();
    committed(import(&mut mind, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A")], vec![]));
    let held = r(K::Question, &id("question", "Q1"));
    let stray = follow_up("FU-1", held.clone());
    // Held in the image.
    assert_eq!(
        refusal(import(&mut mind, vec![stray.clone()], vec![held.clone()])),
        MindRefusal::ExternalPresent { id: held.id.0.clone() }
    );
    // Carried by the batch.
    let carried = r(K::FollowUp, &id("follow_up", "FU-1"));
    assert_eq!(
        refusal(import(&mut mind, vec![stray], vec![carried.clone()])),
        MindRefusal::ExternalPresent { id: carried.id.0.clone() }
    );
    // The giver's resolution is never replayed.
    assert_eq!(
        refusal(import(&mut mind, vec![moved_to("kiko")], vec![])),
        MindRefusal::IncompatibleResolution { subject_kind: K::Campaign, outcome: "Moved".into() }
    );
    // A source that is this mind.
    let own = import_as(&mut mind, source_of(RECEIVER, vec![]), vec![follow_up("FU-1", held)]);
    assert_eq!(refusal(own), MindRefusal::ImportFromSelf);
}

#[test]
fn replaying_a_whole_campaign_reproduces_it() {
    let (mut giver, last) = giver();
    committed(admit(&mut giver, vec![moved_to(RECEIVER)]));
    let moved_at = head(&giver);
    assert_eq!(moved_at, last + 1);

    let mut mind = receiver();
    let outcomes = replay(&giver, &mut mind, last);
    assert!(outcomes.len() >= 8, "{} receipts replayed", outcomes.len());
    for outcome in &outcomes {
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }

    let before_the_move = Reader::at(&giver, last).unwrap();
    let mut compared = 0;
    for kind in K::ALL {
        for (key, _) in giver.image().unwrap().of_kind(*kind) {
            if root_of(key) != CAMPAIGN || *kind == K::Resolution && key == moved_key(RECEIVER) {
                continue;
            }
            let at = before_the_move.view(&r(*kind, key)).unwrap().unwrap();
            let there = view_of(&mind, *kind, key);
            assert_eq!(there.document, at.document, "{key}");
            assert_eq!(there.status, at.status, "{key} reads as it did before the move");
            let source = there.admission.provenance.imported_from.expect("every replayed document names its source");
            assert_eq!(source.instance.0, INSTANCE);
            assert_eq!(source.ordinal, at.admission.ordinal, "{key} names the giver's receipt");
            assert_eq!(source.receipt_id.0, at.admission.receipt_id);
            compared += 1;
        }
    }
    let held: usize = K::ALL.iter().map(|kind| mind.image().unwrap().of_kind(*kind).filter(|(key, _)| root_of(key) == CAMPAIGN).count()).sum();
    assert_eq!(compared, held, "the receiver holds exactly the documents compared, and the giver's all of them but the move");
    assert!(compared >= 14, "{compared} documents compared");
    assert!(
        mind.image().unwrap().of_kind(K::Resolution).all(|(key, _)| key != moved_key(RECEIVER)),
        "the move itself is not replayed"
    );

    // The receiver now keeps the campaign: its leads are hers and the giver's are gone.
    assert_eq!(open_follow_ups(&mind), vec![id("follow_up", "FU-1")]);
    assert!(open_follow_ups(&giver).is_empty());

    // The source survives a restart: it is on the stored receipt.
    let reopened = Mind::open_with(mind.store().clone(), &slug(RECEIVER)).unwrap();
    let key = id("question", "Q1");
    let source = reopened.view(&r(K::Question, &key)).unwrap().unwrap().admission.provenance.imported_from;
    assert_eq!(source.map(|source| source.instance.0), Some(INSTANCE.to_string()));
}

#[test]
fn an_import_replayed_twice_is_already_admitted() {
    let mut mind = receiver();
    let documents = vec![campaign(&[REPO]), target(1, &[INVARIANT])];
    let (receipt_id, _) = committed(import(&mut mind, documents.clone(), vec![]));
    let at = head(&mind);
    assert_eq!(import(&mut mind, documents, vec![]), PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id });
    assert_eq!(head(&mind), at, "the replay wrote nothing");
}

#[test]
fn receipts_without_imported_from_keep_their_bytes_and_digest() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Before {
        faculty: Faculty,
        agent: eureka_pipeline::Short,
        session: eureka_pipeline::Short,
        tool: eureka_pipeline::Short,
    }
    let before = Before { faculty: Faculty::Hands, agent: s("claude"), session: s("session-1"), tool: s("admit") };
    let bytes = rmp_serde::to_vec_named(&before).unwrap();
    assert_eq!(rmp_serde::to_vec_named(&provenance(Faculty::Hands)).unwrap(), bytes, "a non-import provenance serialises as it did");
    let read: PipelineProvenance = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(read, provenance(Faculty::Hands), "and a stored one reads back with no source");

    // Who wrote it never enters the digest: the same batch imported and plain is one receipt.
    let mut plain = receiver();
    let mut imported = receiver();
    let documents = vec![campaign(&[REPO])];
    let (plain_id, _) = committed(admit(&mut plain, documents.clone()));
    let (imported_id, _) = committed(import(&mut imported, documents, vec![]));
    assert_eq!(plain_id, imported_id);
}

#[test]
fn an_imports_source_is_bounded_like_the_rest_of_provenance() {
    let mut mind = receiver();
    let documents = vec![campaign(&[REPO])];
    let field = |outcome: PipelineAdmissionOutcome| match refusal(outcome) {
        MindRefusal::Document(eureka_pipeline::PipelineRefusal::InvalidFormat { field, .. })
        | MindRefusal::Document(eureka_pipeline::PipelineRefusal::FieldBound { field, .. }) => field,
        other => panic!("{other:?}"),
    };
    let with = |edit: &dyn Fn(&mut ImportSource)| {
        let mut source = source_of(INSTANCE, vec![]);
        edit(&mut source);
        source
    };
    let bad = [
        ("provenance.imported_from.instance", with(&|source| source.instance = slug("Not A Slug!"))),
        ("provenance.imported_from.agent", with(&|source| source.agent = s("  "))),
        ("provenance.imported_from.session", with(&|source| source.session = s("two\nlines"))),
        ("provenance.imported_from.receipt_id", with(&|source| source.receipt_id = s(&"x".repeat(201)))),
        ("provenance.imported_from.external", with(&|source| source.external = vec![r(K::Question, "not an id")])),
        (
            "provenance.imported_from.external",
            with(&|source| source.external = (0..=IMPORT_EXTERNAL_MAX).map(|n| r(K::Question, &id("question", &format!("Q{n}")))).collect()),
        ),
    ];
    for (named, source) in bad {
        assert_eq!(field(import_as(&mut mind, source, documents.clone())), named);
        assert_eq!(head(&mind), 1, "{named}: a refused import wrote");
    }
    let exact = with(&|source| source.external = (0..IMPORT_EXTERNAL_MAX).map(|n| r(K::Question, &id("question", &format!("Q{n}")))).collect());
    committed(import_as(&mut mind, exact, documents));
}

#[test]
fn a_moved_resolution_lands_alone_under_its_root() {
    let frozen = |outcome: PipelineAdmissionOutcome| {
        assert_eq!(refusal(outcome), MindRefusal::CampaignMoved { campaign: CAMPAIGN.into(), to: RECEIVER.into() });
    };
    let mut mind = seeded();
    let head_before = head(&mind);
    let sibling = || follow_up("FU-C", campaign_ref());
    // The giver refuses a Moved with a sibling under its root, whichever order they come in.
    frozen(admit(&mut mind, vec![moved_to(RECEIVER), sibling()]));
    frozen(admit(&mut mind, vec![sibling(), moved_to(RECEIVER)]));
    assert_eq!(head(&mind), head_before, "nothing landed");
    // A second Moved is a sibling of the first.
    let second = resolution_n(campaign_ref(), 2, ResolutionOutcome::Moved { to: slug(RECEIVER) });
    frozen(admit(&mut mind, vec![moved_to(RECEIVER), second]));
    // A document of another root rides beside it.
    committed(admit(&mut mind, vec![moved_to(RECEIVER), other_question("QO")]));

    // An import refuses the pairing too: its own sibling never lands beside a Moved.
    let mut recv = receiver();
    committed(import(&mut recv, vec![campaign(&[REPO])], vec![]));
    for documents in [vec![moved_to("kiko"), sibling()], vec![sibling(), moved_to("kiko")]] {
        let refused = refusal(import(&mut recv, documents, vec![]));
        assert!(matches!(refused, MindRefusal::CampaignMoved { .. } | MindRefusal::IncompatibleResolution { .. }), "{refused:?}");
    }
    assert!(recv.view(&r(K::FollowUp, &id("follow_up", "FU-C"))).unwrap().is_none());
}

fn import_from_kiko(mind: &mut Mind<MemoryStore>, documents: Vec<D>) -> PipelineAdmissionOutcome {
    import_as(mind, source_of("kiko", vec![]), documents)
}

fn stewardship_status(mind: &Mind<MemoryStore>, instance: &str) -> PipelineStatus {
    view_of(mind, K::Stewardship, &pipeline_key(&stewardship(instance, REPO)).unwrap()).status
}

#[test]
fn an_import_refuses_instance_rooted_records() {
    // A hand-off from this mind, imported, would leave her stewardship in force beside its own hand-off:
    // import mode derives nothing, so the call that resolves a stewardship is not one it may carry.
    let mut mind = seeded();
    let before = head(&mind);
    let handing = hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[]);
    assert_eq!(refusal(import_from_kiko(&mut mind, vec![handing.clone()])), MindRefusal::ImportOutsideCampaign { root: INSTANCE.into() });
    assert_eq!(
        refusal(import_from_kiko(&mut mind, vec![thought("T1", "an import", &[])])),
        MindRefusal::ImportOutsideCampaign { root: INSTANCE.into() }
    );
    assert_eq!(head(&mind), before, "nothing landed");
    assert!(matches!(stewardship_status(&mind, INSTANCE), PipelineStatus::InForce));
    // The same hand-off, admitted natively, resolves the stewardship.
    committed(admit(&mut mind, vec![handing]));
    assert!(matches!(stewardship_status(&mind, INSTANCE), PipelineStatus::Resolved { .. }));
}

#[test]
fn an_import_lands_only_under_a_root_that_only_imports_have_written() {
    // This mind wrote Q1 and the campaign itself: a ruling imported beside them would answer Q1
    // without the Answered a native ruling derives, and a native ruling could then answer it again.
    let mut mind = seeded();
    committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
    let before = head(&mind);
    let q1 = id("question", "Q1");
    let first = answering(CAMPAIGN, "R1", &q1);
    assert_eq!(refusal(import_from_kiko(&mut mind, vec![first])), MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() });
    assert_eq!(head(&mind), before);
    assert!(matches!(view_of(&mind, K::Question, &q1).status, PipelineStatus::InForce));

    // A root only imports have written takes more imports, and a retry of one is no second write.
    let mut recv = receiver();
    let history = vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A")];
    committed(import_from_kiko(&mut recv, history.clone()));
    committed(import_from_kiko(&mut recv, vec![question("Q2", &["A", "B"], "A")]));
    assert!(matches!(import_from_kiko(&mut recv, history), PipelineAdmissionOutcome::AlreadyAdmitted { .. }));

    // A root that names no campaign, held or carried, is not a campaign's history.
    let mut bare = receiver();
    assert_eq!(
        refusal(import_from_kiko(&mut bare, vec![question("Q1", &["A", "B"], "A")])),
        MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() }
    );
}

#[test]
fn a_replay_under_another_source_is_no_replay() {
    let mut recv = receiver();
    let history = vec![campaign(&[REPO]), question("Q7", &["A", "B"], "A")];
    let q7 = id("question", "Q7");
    committed(import_from_kiko(&mut recv, history.clone()));
    let recorded = |mind: &Mind<MemoryStore>| view_of(mind, K::Question, &q7).admission.provenance.imported_from.map(|source| source.instance.0);
    assert_eq!(recorded(&recv).as_deref(), Some("kiko"));
    let before = head(&recv);

    // The native writer of the same batch is not told it was already admitted, and the record
    // keeps naming the source that wrote it.
    assert!(matches!(refusal(admit(&mut recv, history.clone())), MindRefusal::IdentityCollision { .. }));
    // Nor does another source take the first one's place.
    let other = import_as(&mut recv, source_of("nibu", vec![]), history);
    assert!(matches!(refusal(other), MindRefusal::IdentityCollision { .. }));
    assert_eq!(recorded(&recv).as_deref(), Some("kiko"));
    assert_eq!(head(&recv), before);
}

#[test]
fn an_import_naming_a_case_variant_of_this_mind_names_another_mind() {
    // Two minds whose names differ only in case are two minds (fixtures::CASED_INSTANCE).
    let mut mind = opened(MemoryStore::new(), INSTANCE);
    committed(admit(&mut mind, vec![instance(INSTANCE)]));
    committed(import_as(&mut mind, source_of(CASED_INSTANCE, vec![]), vec![campaign(&[REPO])]));
    assert_eq!(
        refusal(import_as(&mut mind, source_of(INSTANCE, vec![]), vec![question("Q1", &["A", "B"], "A")])),
        MindRefusal::ImportFromSelf
    );
}

#[test]
fn a_campaign_cannot_be_moved_out_of_its_minds_own_instance_root() {
    // A campaign slugged like this mind shares a root with the instance, its stewardships and its
    // thoughts: the Moved would freeze all of them.
    let mut mind = seeded();
    let D::Campaign(mut named_for_the_mind) = campaign(&[REPO]) else { unreachable!() };
    named_for_the_mind.slug = slug(INSTANCE);
    committed(admit(&mut mind, vec![D::Campaign(named_for_the_mind)]));
    let before = head(&mind);
    let moving = resolution(r(K::Campaign, &format!("{INSTANCE}:campaign:self")), ResolutionOutcome::Moved { to: slug(RECEIVER) });
    assert_eq!(refusal(admit(&mut mind, vec![moving])), MindRefusal::MoveOfInstanceRoot { root: INSTANCE.into() });
    assert_eq!(head(&mind), before);
    committed(admit(&mut mind, vec![thought("T1", "still writable", &[])]));
    assert!(matches!(stewardship_status(&mind, INSTANCE), PipelineStatus::InForce));

    // The same campaign cannot arrive by import either: the instance's records are native writes under its root.
    let mut recv = receiver();
    let D::Campaign(mut named_for_recv) = campaign(&[REPO]) else { unreachable!() };
    named_for_recv.slug = slug(RECEIVER);
    assert_eq!(
        refusal(import_from_kiko(&mut recv, vec![D::Campaign(named_for_recv)])),
        MindRefusal::ImportOutsideCampaign { root: RECEIVER.into() }
    );
}
