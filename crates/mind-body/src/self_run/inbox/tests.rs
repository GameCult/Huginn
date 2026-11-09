//! The inbox over a mind in memory: what is a brief, what is told and what the
//! backlog lists.

use eureka_pipeline::{PipelineDocument, PipelineKind as K, PipelineRef, RunOperator, RunTurn};

use super::*;
use crate::persona::AGENT as PERSONA;
use crate::self_run::AGENT as SELF_RUN;
use crate::testkit::{TestMind, close as closing, follow_up, id, recorded, reference, run, run_ref, spec};

fn brief_doc(label: &str, owner: &str, source: &PipelineRef) -> PipelineDocument {
    let PipelineDocument::FollowUp(mut document) = follow_up(label, source) else { unreachable!() };
    document.owner = owner.into();
    document.item = format!("Do {label}.").as_str().into();
    PipelineDocument::FollowUp(document)
}

/// The mind gains these documents under `agent`'s receipt.
fn admitted_by(mind: &TestMind, agent: &str, documents: Vec<PipelineDocument>) {
    let outcome = mind.admit(agent, "session-a", documents).unwrap();
    assert!(matches!(outcome, huginn_mind::PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
}

fn brief_ref(label: &str) -> PipelineRef {
    reference(K::FollowUp, &id("follow_up", label))
}

/// A mind holding a Persona turn's run `persona`, a Self run `selfrun` and a spec.
fn mind() -> TestMind {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x")]);
    mind.committed(vec![run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[]), run("selfrun", RunTurn::SelfRun, RunOperator::Mind, &[])]);
    // Only one Self run is live at a time: hers has ended, and stays a Self run.
    mind.committed(vec![closing(run_ref("selfrun"), recorded())]);
    mind
}

fn ids(briefs: Vec<Brief>) -> Vec<String> {
    briefs.into_iter().map(|brief| brief.id).collect()
}

#[test]
fn a_brief_is_an_in_force_self_run_follow_up_her_persona_turn_admitted_whatever_source_it_names() {
    let mind = mind();
    let persona = run_ref("persona");
    admitted_by(
        &mind,
        PERSONA,
        vec![
            brief_doc("from-persona", OWNER, &persona),
            brief_doc("persona-names-a-spec", OWNER, &crate::testkit::spec_ref("x")),
            brief_doc("someone-elses", "Hands", &persona),
            brief_doc("answered", OWNER, &persona),
        ],
    );
    mind.committed(vec![closing(brief_ref("answered"), recorded())]);
    // Her Self names the Persona turn's run as the source, and so does a hand: the receipt says who admitted them.
    admitted_by(&mind, SELF_RUN, vec![brief_doc("self-cites-persona", OWNER, &persona)]);
    mind.committed(vec![brief_doc("hand-cites-persona", OWNER, &persona)]);
    assert_eq!(
        ids(derive(&mind, &HashSet::new()).unwrap()),
        vec![brief_ref("from-persona").id.0, brief_ref("persona-names-a-spec").id.0],
        "the admitting agent decides, never the source the writer chose"
    );
}

#[test]
fn derive_leaves_out_what_the_session_was_told_and_orders_by_admission() {
    let mind = mind();
    let persona = run_ref("persona");
    admitted_by(&mind, PERSONA, vec![brief_doc("first", OWNER, &persona)]);
    admitted_by(&mind, PERSONA, vec![brief_doc("second", OWNER, &persona)]);
    admitted_by(&mind, PERSONA, vec![brief_doc("third", OWNER, &persona)]);
    let told: HashSet<String> = [brief_ref("second").id.0].into();
    assert_eq!(ids(derive(&mind, &told).unwrap()), vec![brief_ref("first").id.0, brief_ref("third").id.0]);
}

#[test]
fn a_brief_a_closed_run_claimed_is_still_a_brief_and_no_standing_rule_closes_it() {
    let mind = mind();
    admitted_by(&mind, PERSONA, vec![brief_doc("old", OWNER, &run_ref("persona"))]);
    mind.committed(vec![run("old-self", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("old")])]);
    mind.committed(vec![closing(run_ref("old-self"), recorded())]);
    assert_eq!(ids(derive(&mind, &HashSet::new()).unwrap()), vec![brief_ref("old").id.0], "derive does not filter by past claims");
}

#[test]
fn the_backlog_lists_in_force_persona_briefs_claimed_by_closed_runs_and_admits_nothing() {
    let mind = mind();
    let persona = run_ref("persona");
    admitted_by(&mind, PERSONA, vec![brief_doc("stale", OWNER, &persona), brief_doc("fresh", OWNER, &persona), brief_doc("claimed-live", OWNER, &persona)]);
    admitted_by(&mind, SELF_RUN, vec![brief_doc("hers", OWNER, &run_ref("selfrun"))]);
    mind.committed(vec![run("closed-self", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("stale"), brief_ref("hers")])]);
    mind.committed(vec![closing(run_ref("closed-self"), recorded())]);
    mind.committed(vec![run("live-self", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("claimed-live")])]);
    let before = mind.ids(K::Resolution).len();
    let listed = backlog(&mind).unwrap();
    assert_eq!(listed.iter().map(|(brief, run)| (brief.id.clone(), run.clone())).collect::<Vec<_>>(), vec![(brief_ref("stale").id.0, run_ref("closed-self").id.0)], "only a Persona brief claimed by a closed run");
    assert_eq!(mind.ids(K::Resolution).len(), before, "listing admits nothing");
}

#[test]
fn a_tell_names_each_brief_the_plan_and_how_she_resolves_it() {
    let brief = Brief { id: "eureka-body:follow_up:b1".into(), campaign: "eureka-body".into(), repo: "GameCult/Huginn".into(), item: "Wire the thing.".into(), ordinal: 1, conversation: persona::Conversation::Public };
    let text = render(&[brief], "Plan: 5h at 40%.");
    assert!(text.contains("eureka-body:follow_up:b1") && text.contains("Wire the thing."), "{text}");
    let reminder = resolving_sentence("eureka-body:follow_up:b1");
    assert!(text.contains(&reminder), "{text}");
    for word in ["eureka-body:follow_up:b1", "Recorded", "Fixed", "default branch", "Superseded", "Withdrawn", "An open PR, a pushed branch or a local commit is not done"] {
        assert!(reminder.contains(word), "{word}: {reminder}");
    }
    assert!(!text.contains(CLOSING_RULE), "the rule is told once, at start: {text}");
    assert!(text.ends_with("Plan: 5h at 40%."), "{text}");
    assert!(!render(&[], "").contains("brief"), "no brief, no plan, nothing");
}

#[test]
fn imported_briefs_are_not_her_briefs() {
    // A campaign moved to her: the giver's Persona admitted this brief, and her Self run imports it.
    let mind = TestMind::without_campaigns();
    let persona = run_ref("persona");
    let source = huginn_mind::ImportSource {
        instance: "eureka".into(),
        receipt_id: "mind-commit-giver".into(),
        ordinal: 7,
        committed_at: "2026-11-08T16:00:00Z".into(),
        faculty: huginn_mind::Faculty::SelfFaculty,
        agent: PERSONA.into(),
        session: "giver-persona-session".into(),
        tool: "huginn".into(),
        external: vec![persona.clone()],
    };
    let batch = huginn_mind::PipelineAdmissionBatch {
        instance: crate::testkit::INSTANCE.into(),
        provenance: huginn_mind::PipelineProvenance {
            faculty: huginn_mind::Faculty::SelfFaculty,
            agent: SELF_RUN.into(),
            session: "import-1".into(),
            tool: "huginn".into(),
            imported_from: Some(source),
        },
        documents: vec![crate::testkit::campaign_named(crate::testkit::CAMPAIGN, "Eureka body"), brief_doc("moved", OWNER, &persona)],
    };
    let huginn_mind::HuginnMindResponse::Admit(outcome) = mind.answer(huginn_mind::HuginnMindRequest::Admit(batch)) else { panic!("an admit answers an admit") };
    assert!(matches!(outcome, huginn_mind::PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");

    let in_force_ids: Vec<String> = headers(&mind, &in_force(of_kind(K::FollowUp))).unwrap().into_iter().map(|header| header.id.id.0).collect();
    assert_eq!(in_force_ids, vec![brief_ref("moved").id.0.clone()], "the imported follow-up stands in her mind");
    assert!(derive(&mind, &HashSet::new()).unwrap().is_empty(), "the giver's Persona's brief is not her Persona's brief");

    // The same document admitted by her own Persona turn is a brief.
    mind.committed(vec![run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    admitted_by(&mind, PERSONA, vec![brief_doc("hers", OWNER, &persona)]);
    assert_eq!(ids(derive(&mind, &HashSet::new()).unwrap()), vec![brief_ref("hers").id.0]);
}
