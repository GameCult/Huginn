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
use crate::receipt::{Faculty, IMPORT_EXTERNAL_MAX, ImportSource, PipelineProvenance};
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

/// Every document the giver holds under the campaign's root, as the verb reads them: one cursor
/// walk of the document projection.
fn giver_views(giver: &Mind<MemoryStore>) -> Vec<crate::query::PipelineDocumentView> {
    let mut selection = Selection {
        fields: Some(vec![FieldPredicate { index: "root".into(), op: "any_of".into(), values: Some(vec![CAMPAIGN.into()]), number: None }]),
        projection: "document".into(),
        limit: Some(cultnet_rs::LIMIT_MAX),
        ..Selection::default()
    };
    let mut views = Vec::new();
    loop {
        let page = giver.query(&selection).unwrap();
        let PipelinePageItems::Documents(found) = page.items else { panic!("the document projection answers documents") };
        views.extend(found);
        match page.next {
            Some(next) => selection.cursor = Some(next),
            None => return views,
        }
    }
}

/// Replays the giver's campaign into the receiver as `import::plan` lays it out: one import per
/// source receipt, each naming its source.
fn replay(giver: &Mind<MemoryStore>, receiver: &mut Mind<MemoryStore>) -> Vec<PipelineAdmissionOutcome> {
    let plan = crate::import::plan(&slug(CAMPAIGN), giver.instance(), receiver.instance(), giver_views(giver)).unwrap();
    plan.batches
        .into_iter()
        .map(|batch| import_as(receiver, batch.source, batch.documents.into_iter().map(|(_, document)| document).collect()))
        .collect()
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
    let outcomes = replay(&giver, &mut mind);
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
fn an_import_resumes_past_a_follow_up_or_idea_the_receiver_wrote_after_it_began() {
    let q1 = id("question", "Q1");
    let begun = || {
        let mut recv = receiver();
        committed(import_from_kiko(&mut recv, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A")]));
        recv
    };
    for native in [follow_up("FU-N", r(K::Question, &q1)), idea("p", "note", "a note")] {
        let mut recv = begun();
        committed(admit(&mut recv, vec![native]));
        committed(import_from_kiko(&mut recv, vec![question("Q2", &["A", "B"], "A")]));
        committed(import_from_kiko(&mut recv, vec![answering(CAMPAIGN, "R1", &q1)]));
    }

    // Before the first imported document the same record is the receiver's own, as ever.
    let mut early = receiver();
    committed(admit(&mut early, vec![idea("p", "note", "a note")]));
    assert_eq!(
        refusal(import_from_kiko(&mut early, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A")])),
        MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() }
    );

    // A kind that could answer, supersede or resolve a replayed document still refuses, after the import began.
    for native in [question("Q9", &["A", "B"], "A"), resolution(r(K::Question, &q1), ResolutionOutcome::Withdrawn { reason: "native".into() })] {
        let mut recv = begun();
        committed(admit(&mut recv, vec![native]));
        assert_eq!(refusal(import_from_kiko(&mut recv, vec![question("Q2", &["A", "B"], "A")])), MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() });
    }
}

#[test]
fn an_import_resumes_past_the_resolution_of_a_follow_up_the_receiver_wrote_after_it_began() {
    let q1 = id("question", "Q1");
    let begun = || {
        let mut recv = receiver();
        committed(import_from_kiko(&mut recv, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A")]));
        recv
    };
    // Her own follow-up, closed natively: nothing the replay reaches.
    let mut recv = begun();
    let native = follow_up("FU-N", r(K::Question, &q1));
    let native_ref = r(K::FollowUp, &pipeline_key(&native).unwrap());
    committed(admit(&mut recv, vec![native]));
    committed(admit(&mut recv, vec![resolution(native_ref, ResolutionOutcome::Recorded { reason: "done".into() })]));
    committed(import_from_kiko(&mut recv, vec![question("Q2", &["A", "B"], "A")]));

    // A native closing of a replayed follow-up is the receiver reaching into the plan: still refused.
    let mut recv = begun();
    let imported = follow_up("FU-I", r(K::Question, &q1));
    let imported_ref = r(K::FollowUp, &pipeline_key(&imported).unwrap());
    committed(import_from_kiko(&mut recv, vec![imported]));
    committed(admit(&mut recv, vec![resolution(imported_ref, ResolutionOutcome::Recorded { reason: "done".into() })]));
    assert_eq!(refusal(import_from_kiko(&mut recv, vec![question("Q2", &["A", "B"], "A")])), MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() });
}

#[test]
fn an_import_never_cites_or_resolves_a_follow_up_the_receiver_wrote() {
    let q1 = id("question", "Q1");
    let mut recv = receiver();
    committed(import_from_kiko(&mut recv, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A")]));
    let native = follow_up("FU-N", r(K::Question, &q1));
    let native_ref = r(K::FollowUp, &pipeline_key(&native).unwrap());
    committed(admit(&mut recv, vec![native]));
    let before = head(&recv);
    // An imported record that withdraws the receiver's own follow-up mixes a replay into her work.
    assert_eq!(
        refusal(import_from_kiko(&mut recv, vec![resolution(native_ref.clone(), ResolutionOutcome::Withdrawn { reason: "x".into() })])),
        MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() }
    );
    // Nor may one cite it.
    assert_eq!(
        refusal(import_from_kiko(&mut recv, vec![follow_up("FU-I", native_ref.clone())])),
        MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() }
    );
    // Nor may a closing of its own point at it: an imported follow-up superseded by her native one.
    let own_fu = follow_up("FU-I2", r(K::Question, &q1));
    let own_fu_ref = r(K::FollowUp, &pipeline_key(&own_fu).unwrap());
    committed(import_from_kiko(&mut recv, vec![own_fu]));
    let before = head(&recv);
    assert_eq!(
        refusal(import_from_kiko(&mut recv, vec![resolution(own_fu_ref, ResolutionOutcome::Superseded { by: vec![native_ref.clone()] })])),
        MindRefusal::ImportOutsideCampaign { root: CAMPAIGN.into() }
    );
    assert_eq!(head(&recv), before);
    // Her follow-up still closes natively.
    committed(admit(&mut recv, vec![resolution(native_ref, ResolutionOutcome::Withdrawn { reason: "x".into() })]));
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

fn fu(root: &str, label: &str) -> PipelineRef {
    r(K::FollowUp, &format!("{root}:follow_up:{label}"))
}

/// The receiver after campaign A (`CAMPAIGN`) came in whole: its follow-up FU-1 is open there, and
/// FU-A is recorded.
fn receiver_holding_a_resolved_follow_up() -> Mind<MemoryStore> {
    let mut mind = receiver();
    let q1 = r(K::Question, &id("question", "Q1"));
    committed(import(&mut mind, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A"), follow_up("FU-A", q1)], vec![]));
    committed(import(&mut mind, vec![resolution(fu(CAMPAIGN, "FU-A"), ResolutionOutcome::Recorded { reason: "noted".into() })], vec![]));
    mind
}

/// Campaign B's receipt, admitted by its giver while FU-A was still open: FU-B superseded by FU-A.
fn b_supersedes_a() -> Vec<D> {
    vec![resolution(fu(OTHER_ROOT, "FU-B"), ResolutionOutcome::Superseded { by: vec![fu(CAMPAIGN, "FU-A")] })]
}

fn b_campaign() -> Vec<D> {
    vec![other_campaign(), other_question("Q9"), other_follow_up("FU-B", "Q9")]
}

#[test]
fn an_import_may_cite_a_document_the_receiver_resolved_after_the_source_ordinal() {
    let mut mind = receiver_holding_a_resolved_follow_up();
    committed(import(&mut mind, b_campaign(), vec![]));
    let (_, writes) = committed(import(&mut mind, b_supersedes_a(), vec![]));
    assert_eq!(writes.len(), 1);
    let view = view_of(&mind, K::FollowUp, &fu(OTHER_ROOT, "FU-B").id.0);
    assert!(matches!(view.status, PipelineStatus::Resolved { .. }), "{:?}", view.status);
}

#[test]
fn a_native_admission_citing_a_resolved_document_is_still_refused() {
    let mut mind = receiver();
    let q1 = r(K::Question, &id("question", "Q1"));
    committed(admit(&mut mind, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A"), follow_up("FU-A", q1.clone()), follow_up("FU-B", q1)]));
    committed(admit(&mut mind, vec![resolution(fu(CAMPAIGN, "FU-A"), ResolutionOutcome::Recorded { reason: "noted".into() })]));
    let superseded = resolution(fu(CAMPAIGN, "FU-B"), ResolutionOutcome::Superseded { by: vec![fu(CAMPAIGN, "FU-A")] });
    assert_eq!(refusal(admit(&mut mind, vec![superseded])), MindRefusal::CitesResolvedDocument { kind: K::FollowUp, id: fu(CAMPAIGN, "FU-A").id.0 });
}

#[test]
fn a_campaign_imported_after_the_one_it_cites_resumes_and_matches_its_giver() {
    let (mut giver, _) = giver();
    committed(admit(&mut giver, b_campaign()));
    // B's receipt cites FU-1 while it is open; A resolves it afterwards.
    committed(admit(&mut giver, vec![resolution(fu(OTHER_ROOT, "FU-B"), ResolutionOutcome::Superseded { by: vec![fu(CAMPAIGN, "FU-1")] })]));
    committed(admit(&mut giver, vec![resolution(fu(CAMPAIGN, "FU-1"), ResolutionOutcome::Recorded { reason: "noted".into() })]));
    committed(admit(&mut giver, vec![moved_to(RECEIVER)]));
    committed(admit(&mut giver, vec![resolution(r(K::Campaign, &format!("{OTHER_ROOT}:campaign:self")), ResolutionOutcome::Moved { to: slug(RECEIVER) })]));

    let mut mind = receiver();
    for outcome in replay_root(&giver, &mut mind, CAMPAIGN) {
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }
    let outcomes = replay_root(&giver, &mut mind, OTHER_ROOT);
    assert_eq!(outcomes.len(), 2, "B lands as its own opening and the citing receipt");
    for outcome in &outcomes {
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }
    let key = fu(OTHER_ROOT, "FU-B").id.0;
    assert_eq!(view_of(&mind, K::FollowUp, &key).status, Reader::at(&giver, head(&giver) - 2).unwrap().view(&r(K::FollowUp, &key)).unwrap().unwrap().status);

    // A run stopped partway runs again from the top: what landed answers AlreadyAdmitted.
    for outcome in replay_root(&giver, &mut mind, OTHER_ROOT) {
        assert!(matches!(outcome, PipelineAdmissionOutcome::AlreadyAdmitted { .. }), "{outcome:?}");
    }
}

/// `replay` for any root, declaring external only what the receiver does not hold, as the verb does.
fn replay_root(giver: &Mind<MemoryStore>, receiver: &mut Mind<MemoryStore>, root: &str) -> Vec<PipelineAdmissionOutcome> {
    let mut selection = Selection {
        fields: Some(vec![FieldPredicate { index: "root".into(), op: "any_of".into(), values: Some(vec![root.into()]), number: None }]),
        projection: "document".into(),
        limit: Some(cultnet_rs::LIMIT_MAX),
        ..Selection::default()
    };
    let mut views = Vec::new();
    loop {
        let page = giver.query(&selection).unwrap();
        let PipelinePageItems::Documents(found) = page.items else { panic!("the document projection answers documents") };
        views.extend(found);
        match page.next {
            Some(next) => selection.cursor = Some(next),
            None => break,
        }
    }
    let plan = crate::import::plan(&slug(root), giver.instance(), receiver.instance(), views).unwrap();
    plan.batches
        .into_iter()
        .map(|mut batch| {
            batch.source.external.retain(|referent| receiver.view(referent).unwrap().is_none());
            import_as(receiver, batch.source, batch.documents.into_iter().map(|(_, document)| document).collect())
        })
        .collect()
}

fn ruling_of(root: &str, label: &str) -> D {
    let mut value = ruling(label);
    value.campaign = slug(root);
    D::Ruling(value)
}

fn ruling_ref(root: &str, label: &str) -> PipelineRef {
    r(K::Ruling, &format!("{root}:ruling:{label}"))
}

fn spec_ref(root: &str, revision: u32) -> PipelineRef {
    r(K::CutSpec, &format!("{root}:cut_spec:cut-1.r{revision}"))
}

fn spec_of(root: &str, revision: u32) -> D {
    let mut value = cut_spec("1", revision);
    value.campaign = slug(root);
    D::CutSpec(value)
}

fn report_citing(spec: &PipelineRef) -> D {
    let mut value = cut_report("1", 1);
    value.cut_spec = spec.id.clone();
    D::CutReport(value)
}

fn spec_citing(ruling: &PipelineRef) -> D {
    let mut value = cut_spec("2", 1);
    value.rulings = vec![ruling.id.clone()];
    D::CutSpec(value)
}

/// A receiver holding `root` whole, in which `old` is superseded by `new` (both of that root).
fn receiver_with_superseded(root: &str, opening: Vec<D>, closing: Vec<D>, old: PipelineRef, new: PipelineRef) -> Mind<MemoryStore> {
    let mut mind = receiver();
    let mut first = vec![if root == CAMPAIGN { campaign(&[REPO]) } else { other_campaign() }];
    first.extend(opening);
    committed(import(&mut mind, first, vec![]));
    let mut second = closing;
    second.push(resolution(old, superseded(&[new])));
    committed(import(&mut mind, second, vec![]));
    mind
}

#[test]
fn an_imported_report_citing_its_own_campaigns_superseded_spec_is_refused() {
    let (old, new) = (spec_ref(CAMPAIGN, 1), spec_ref(CAMPAIGN, 2));
    let mut mind = receiver_with_superseded(CAMPAIGN, vec![spec_of(CAMPAIGN, 1)], vec![spec_of(CAMPAIGN, 2)], old.clone(), new);
    assert_eq!(
        refusal(import(&mut mind, vec![report_citing(&old)], vec![])),
        MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: old.id.0 }
    );
}

#[test]
fn an_imported_spec_may_cite_another_campaigns_ruling_superseded_since() {
    let (old, new) = (ruling_ref(OTHER_ROOT, "R1"), ruling_ref(OTHER_ROOT, "R2"));
    let mut mind = receiver_with_superseded(OTHER_ROOT, vec![ruling_of(OTHER_ROOT, "R1"), ruling_of(OTHER_ROOT, "R2")], vec![], old.clone(), new);
    committed(import(&mut mind, vec![campaign(&[REPO])], vec![]));
    let (_, writes) = committed(import(&mut mind, vec![spec_citing(&old)], vec![]));
    assert_eq!(writes.len(), 1);
}

#[test]
fn an_imported_spec_citing_its_own_campaigns_superseded_ruling_is_refused() {
    let (old, new) = (ruling_ref(CAMPAIGN, "R1"), ruling_ref(CAMPAIGN, "R2"));
    let mut mind = receiver_with_superseded(CAMPAIGN, vec![ruling_of(CAMPAIGN, "R1"), ruling_of(CAMPAIGN, "R2")], vec![], old.clone(), new);
    assert_eq!(
        refusal(import(&mut mind, vec![spec_citing(&old)], vec![])),
        MindRefusal::CitesResolvedDocument { kind: K::Ruling, id: old.id.0 }
    );
}

#[test]
fn an_imported_follow_up_may_not_be_superseded_by_its_own_campaigns_closed_follow_up() {
    let q1 = r(K::Question, &id("question", "Q1"));
    let mut mind = receiver();
    committed(import(&mut mind, vec![campaign(&[REPO]), question("Q1", &["A", "B"], "A"), follow_up("FU-A", q1.clone()), follow_up("FU-B", q1)], vec![]));
    committed(import(&mut mind, vec![resolution(fu(CAMPAIGN, "FU-A"), ResolutionOutcome::Recorded { reason: "noted".into() })], vec![]));
    assert_eq!(
        refusal(import(&mut mind, vec![resolution(fu(CAMPAIGN, "FU-B"), superseded(&[fu(CAMPAIGN, "FU-A")]))], vec![])),
        MindRefusal::CitesResolvedDocument { kind: K::FollowUp, id: fu(CAMPAIGN, "FU-A").id.0 }
    );
}

/// The real cases (thing2 ordinal 1815, aetheria-release ordinal 2247): a Finding of the imported
/// root deferred to another root's follow-up that the receiver holds already closed.
#[test]
fn import_replays_a_finding_deferred_to_another_roots_closed_follow_up() {
    let mut mind = receiver();
    committed(admit(&mut mind, vec![other_question("Q5"), other_follow_up("FU-9", "Q5")]));
    let foreign = r(K::FollowUp, &format!("{OTHER_ROOT}:follow_up:FU-9"));
    committed(admit(&mut mind, vec![resolution(foreign.clone(), ResolutionOutcome::Fixed { commit: sha(), by: None })]));
    let q1 = id("question", "Q1");
    let finding = id("finding", "cut-1.s1.F1");
    committed(import(&mut mind, vec![campaign(&[REPO]), target(1, &[INVARIANT]), question("Q1", &["A", "B"], "A"), answering(CAMPAIGN, "R1", &q1)], vec![]));
    committed(import(&mut mind, vec![D::CutSpec(cut_spec("1", 1))], vec![]));
    committed(import(&mut mind, vec![D::CutReport(cut_report("1", 1))], vec![]));
    committed(import(
        &mut mind,
        vec![verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&finding], Some("P1"), &["M1"])]), D::Finding(finding_doc())],
        vec![],
    ));
    let (_, writes) = committed(import(&mut mind, vec![resolution(r(K::Finding, &finding), ResolutionOutcome::Deferred { to: foreign })], vec![]));
    assert_eq!(writes.len(), 1);
    assert!(matches!(view_of(&mind, K::Finding, &finding).status, PipelineStatus::Resolved { .. }));
}

/// The replay does not ask a cited referent of another root to be open, whatever its kind or how
/// it closed: an idea superseded by another root's idea that is already recorded.
#[test]
fn import_replays_a_cite_of_another_roots_closed_document_of_any_kind() {
    let mut mind = receiver();
    let mut other = idea("p", "other", "x");
    let D::Idea(value) = &mut other else { unreachable!() };
    value.campaign = slug(OTHER_ROOT);
    let other_ref = r(K::Idea, &pipeline_key(&other).unwrap());
    committed(admit(&mut mind, vec![other]));
    committed(admit(&mut mind, vec![resolution(other_ref.clone(), ResolutionOutcome::Recorded { reason: "x".into() })]));
    let note = idea("p", "note", "an idea");
    let note_ref = r(K::Idea, &pipeline_key(&note).unwrap());
    committed(import(&mut mind, vec![campaign(&[REPO]), note], vec![]));
    committed(import(&mut mind, vec![resolution(note_ref.clone(), ResolutionOutcome::Superseded { by: vec![other_ref] })], vec![]));
    assert!(matches!(view_of(&mind, K::Idea, &note_ref.id.0).status, PipelineStatus::Resolved { .. }));
}
