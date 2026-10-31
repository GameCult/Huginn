//! The inbox over a mind in memory: what is a brief, what is told, what the
//! backlog lists, and what the idle-end close resolves.

use eureka_pipeline::{PipelineDocument, PipelineKind as K, RunOperator, RunTurn};

use super::*;
use crate::testkit::{TestMind, at, close as closing, follow_up, id, recorded, reference, run, run_ref, spec};

fn brief_doc(label: &str, owner: &str, source: &PipelineRef) -> PipelineDocument {
    let PipelineDocument::FollowUp(mut document) = follow_up(label, source) else { unreachable!() };
    document.owner = owner.into();
    document.item = format!("Do {label}.").as_str().into();
    PipelineDocument::FollowUp(document)
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
fn a_brief_is_an_in_force_self_run_follow_up_sourced_from_a_persona_turn() {
    let mind = mind();
    let persona = run_ref("persona");
    mind.committed(vec![
        brief_doc("from-persona", OWNER, &persona),
        brief_doc("from-her-self", OWNER, &run_ref("selfrun")),
        brief_doc("from-no-run", OWNER, &crate::testkit::spec_ref("x")),
        brief_doc("someone-elses", "Hands", &persona),
        brief_doc("answered", OWNER, &persona),
    ]);
    mind.committed(vec![closing(brief_ref("answered"), recorded())]);
    assert_eq!(ids(derive(&mind, &HashSet::new()).unwrap()), vec![brief_ref("from-persona").id.0], "a follow-up her Self files for herself is never a brief");
}

#[test]
fn derive_leaves_out_what_the_session_was_told_and_orders_by_admission() {
    let mind = mind();
    let persona = run_ref("persona");
    mind.committed(vec![brief_doc("first", OWNER, &persona)]);
    mind.committed(vec![brief_doc("second", OWNER, &persona)]);
    mind.committed(vec![brief_doc("third", OWNER, &persona)]);
    let told: HashSet<String> = [brief_ref("second").id.0].into();
    assert_eq!(ids(derive(&mind, &told).unwrap()), vec![brief_ref("first").id.0, brief_ref("third").id.0]);
}

#[test]
fn a_brief_a_closed_run_claimed_is_still_a_brief_and_no_standing_rule_closes_it() {
    let mind = mind();
    mind.committed(vec![brief_doc("old", OWNER, &run_ref("persona"))]);
    mind.committed(vec![run("old-self", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("old")])]);
    mind.committed(vec![closing(run_ref("old-self"), recorded())]);
    assert_eq!(ids(derive(&mind, &HashSet::new()).unwrap()), vec![brief_ref("old").id.0], "derive does not filter by past claims");
}

#[test]
fn the_backlog_lists_in_force_persona_briefs_claimed_by_closed_runs_and_admits_nothing() {
    let mind = mind();
    let persona = run_ref("persona");
    mind.committed(vec![brief_doc("stale", OWNER, &persona), brief_doc("fresh", OWNER, &persona), brief_doc("claimed-live", OWNER, &persona), brief_doc("hers", OWNER, &run_ref("selfrun"))]);
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
    let brief = Brief { id: "eureka-body:follow_up:b1".into(), campaign: "eureka-body".into(), repo: "GameCult/Huginn".into(), item: "Wire the thing.".into(), ordinal: 1 };
    let text = render(&[brief], "Plan: 5h at 40%.");
    assert!(text.contains("eureka-body:follow_up:b1") && text.contains("Wire the thing."), "{text}");
    assert!(text.contains("resolve follow-up eureka-body:follow_up:b1 Recorded") && text.contains("Withdrawn"), "she is told to resolve it: {text}");
    assert!(text.ends_with("Plan: 5h at 40%."), "{text}");
    assert!(!render(&[], "").contains("brief"), "no brief, no plan, nothing");
}

#[test]
fn closing_resolves_only_the_briefs_still_in_force_in_one_admission() {
    let mind = mind();
    mind.committed(vec![brief_doc("a", OWNER, &run_ref("persona")), brief_doc("b", OWNER, &run_ref("persona"))]);
    mind.committed(vec![closing(brief_ref("b"), recorded())]);
    let entry = |label: &str, n: u64| Brief { id: brief_ref(label).id.0, campaign: "eureka-body".into(), repo: "GameCult/Huginn".into(), item: format!("Item {n}."), ordinal: n };
    let briefs = [(entry("a", 1), "The answer.".to_string()), (entry("b", 2), "Ignored.".to_string())];
    let now = at(0);
    assert_eq!(close(&mind, now, "selfrun-a", &briefs, "answered").unwrap(), 1);
    let rationales: Vec<String> = mind
        .ids(K::Resolution)
        .iter()
        .filter_map(|resolution| match mind.view(K::Resolution, resolution)?.document {
            PipelineDocument::Resolution(document) if document.subject == brief_ref("a") => Some(document.rationale.0),
            _ => None,
        })
        .collect();
    assert_eq!(rationales, vec!["The answer.".to_string()]);
    assert_eq!(close(&mind, now, "selfrun-a", &briefs[..1], "answered").unwrap(), 0, "already closed");
}
