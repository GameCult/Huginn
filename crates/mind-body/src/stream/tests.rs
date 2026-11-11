use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{MemorySort, PipelineDocument, PipelineKind, PipelineReply, PipelineThought, RunOperator, RunTurn, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelineSelectionPage, RepoActivity, RepoChange, RepoEvent};
use serde_json::{Value, json};

use super::*;
use crate::control::{Grant, HeldReason};
use crate::testkit::{INSTANCE, TestMind, at, memory, reference, run, run_ref};

fn t(secs: i64) -> DateTime<Utc> {
    at(secs * 1000)
}

/// The label of a thought ticked at test-clock `secs`: the Unix seconds of that instant.
fn ticked(secs: i64) -> String {
    format!("t{}", t(secs).timestamp())
}

fn released() -> Effective {
    Effective::Released(Grant::for_test(Slug(INSTANCE.into())))
}

fn state() -> Value {
    json!({
        "publicName": "Yggdrasil",
        "publicDescription": "A Mind of the swarm.",
        "presentation": { "pronouns": "she/her", "voiceSummary": "Plain." },
        "values": [{ "id": "v1", "label": "Coherence", "priority": 0.9, "summary": "Coherence over velocity." }],
        "agencyPressure": { "pressures": [{ "summary": "The shaders are late." }] },
        "affect": { "socialBonds": [{
            "object": { "kind": "person", "id": "@libby:chat", "label": "Libby" },
            "relationshipKind": "collaborator", "summary": "Libby holds the sounds."
        }] },
    })
}

struct Persona(PersonaRead);

impl PersonaSource for Persona {
    fn persona(&self) -> Result<PersonaRead> {
        Ok(PersonaRead { state: self.0.state.clone(), receipt_id: self.0.receipt_id.clone() })
    }
}

fn persona() -> Persona {
    Persona(PersonaRead { state: state(), receipt_id: "mind-commit-1".into() })
}

struct Fake {
    projection: String,
    candidates: Vec<String>,
    fault: bool,
    projected: RefCell<Vec<(String, String)>>,
    continued: RefCell<Vec<(String, Sampling, usize)>>,
}

impl Fake {
    fn saying(projection: &str, candidates: &[&str]) -> Self {
        Self { projection: projection.into(), candidates: candidates.iter().map(|text| (*text).into()).collect(), fault: false, projected: RefCell::default(), continued: RefCell::default() }
    }

    fn calls(&self) -> usize {
        self.projected.borrow().len() + self.continued.borrow().len()
    }
}

impl Projector for Fake {
    fn model(&self) -> &str {
        "projector:test"
    }

    fn project(&self, request_id: &str, prompt: &str) -> Result<String> {
        self.projected.borrow_mut().push((request_id.into(), prompt.into()));
        if self.fault { Err(anyhow!("the connector is down")) } else { Ok(self.projection.clone()) }
    }
}

impl Continuer for Fake {
    fn continue_raw(&self, prompt: &str, sampling: &Sampling, n: usize) -> Result<Vec<String>> {
        self.continued.borrow_mut().push((prompt.into(), *sampling, n));
        Ok(self.candidates.iter().take(n).cloned().collect())
    }
}

/// A real mind that records what is asked and admitted, and scores semantic asks as told.
struct Wrap<'a> {
    inner: &'a TestMind,
    scores: RefCell<VecDeque<f32>>,
    selections: RefCell<Vec<Selection>>,
    admitted: RefCell<Vec<Vec<PipelineDocument>>>,
    forge: Option<RepoActivity>,
    forge_reads: std::cell::Cell<usize>,
}

impl<'a> Wrap<'a> {
    fn over(inner: &'a TestMind, scores: &[f32]) -> Self {
        Self { inner, scores: RefCell::new(scores.iter().copied().collect()), selections: RefCell::default(), admitted: RefCell::default(), forge: None, forge_reads: std::cell::Cell::new(0) }
    }

    /// The same mind with a forge that answers `events`, whatever the asker's cursor.
    fn with_forge(mut self, events: Vec<RepoEvent>) -> Self {
        self.forge = Some(RepoActivity { head: 1, events, capped: vec![], open_prs: vec![], unseen: vec![], moved: vec![] });
        self
    }
}

impl MindPort for Wrap<'_> {
    fn instance(&self) -> &Slug {
        self.inner.instance()
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.selections.borrow_mut().push(selection.clone());
        self.inner.query(selection)
    }

    fn query_near(&self, selection: &Selection, _text: &str, _top_k: u32) -> Result<PipelineSelectionPage> {
        self.selections.borrow_mut().push(selection.clone());
        let mut page = self.inner.query(selection)?;
        page.scores = Some(vec![self.scores.borrow_mut().pop_front().ok_or_else(|| anyhow!("no score was scripted"))?]);
        Ok(page)
    }

    fn roots_touched(&self) -> Result<Vec<huginn_mind::RootTouched>> {
        self.inner.roots_touched()
    }

    fn stewarded_activity(&self, repos: &[huginn_mind::eureka_pipeline::OrgRepo], after: Option<u64>) -> Result<RepoActivity> {
        self.forge_reads.set(self.forge_reads.get() + 1);
        match &self.forge {
            Some(activity) => Ok(activity.clone()),
            None => self.inner.stewarded_activity(repos, after),
        }
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.admitted.borrow_mut().push(documents.clone());
        self.inner.admit(agent, session, documents)
    }
}

/// The seed as the tick builds it: the cheap reads first, then the rest.
fn seed_of(mind: &dyn MindPort, read: &PersonaRead, since: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Result<Seed> {
    let responsibilities = render_responsibilities(mind, mind.instance(), &BTreeMap::new())?;
    let mut fresh = mind_news(mind, mind.instance(), since)?;
    fresh.extend(repo_news(mind, since, &responsibilities, now));
    build_seed(mind, read, since, now, &responsibilities, fresh)
}

/// A push by someone else to her repo at `secs`.
fn push(id: u64, secs: i64) -> RepoEvent {
    RepoEvent { id, at: u64::try_from(t(secs).timestamp_millis()).unwrap(), repo: "GameCult/Huginn".into(), actor: "libby".into(), change: RepoChange::Pushed { commits: 1 } }
}

/// Counts how often her persona is read, which only a tick that may run does.
struct CountedPersona(Persona, std::cell::Cell<usize>);

impl PersonaSource for CountedPersona {
    fn persona(&self) -> Result<PersonaRead> {
        self.1.set(self.1.get() + 1);
        self.0.persona()
    }
}

fn thought_doc(label: &str, text: &str) -> PipelineDocument {
    PipelineDocument::Thought(PipelineThought {
        instance: Slug(INSTANCE.into()),
        label: label.into(),
        text: text.into(),
        reads: vec![],
        persona_receipt: "mind-commit-0".into(),
        identity: "I".into(),
        context: "".into(),
        stimulus: "".into(),
        projector_model: "projector".into(),
        projection: "A projection.".into(),
        base_model: "base".into(),
        temperature_permille: 1000,
        min_p_permille: 50,
        max_tokens: 160,
        candidates: vec![text.into()],
        nearest_permille: vec![1000],
        chosen: 0,
        projector_ms: 0,
        continuer_ms: 0,
        prompt_version: "test".into(),
    })
}

/// A mind with an old memory about Libby and one thought at `thought_at`.
fn mind_with_thought(thought_at: i64) -> TestMind {
    let mind = TestMind::seeded();
    mind.admit_at(t(10));
    mind.committed(vec![memory("m1", MemorySort::Learned, "Libby wants the sounds before the shaders", &["@libby:chat"])]);
    mind.admit_at(t(thought_at));
    mind.committed(vec![thought_doc(&ticked(thought_at), "An earlier drift")]);
    mind
}

fn organs<'a>(mind: &'a dyn MindPort, persona: &'a Persona, model: &'a Fake) -> Organs<'a> {
    Organs { mind, persona, projector: model, continuer: model, base_model: "base:test" }
}

fn thoughts(mind: &TestMind) -> Vec<String> {
    mind.ids(PipelineKind::Thought)
}

#[test]
fn budgets_leave_room_for_every_section() {
    let sum: usize = BUDGETS.iter().map(|(_, budget)| budget).sum();
    assert!(sum + BUDGETS.len() <= SECTION_MAX_BYTES, "{sum}");
}

#[test]
fn seed_reads_no_thought() {
    let without = TestMind::seeded();
    without.committed(vec![memory("m1", MemorySort::Learned, "Libby wants the sounds first", &["@libby:chat"])]);
    let with = TestMind::seeded();
    with.committed(vec![memory("m1", MemorySort::Learned, "Libby wants the sounds first", &["@libby:chat"])]);
    with.committed(vec![thought_doc("t1", "A drift that must stay out of the seed")]);
    let (plain, thoughtful) = (Wrap::over(&without, &[]), Wrap::over(&with, &[]));
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let one = seed_of(&plain, &read, None, t(100_000)).unwrap();
    let two = seed_of(&thoughtful, &read, None, t(100_000)).unwrap();
    assert_eq!(one.projector_prompt(), two.projector_prompt(), "a mind holding thoughts yields the prompt of the same mind without them");
    assert_eq!(one.reads, two.reads);
    assert!(!two.projector_prompt().contains("must stay out"));
    let thought_schema = PipelineKind::Thought.type_id();
    for selection in thoughtful.selections.borrow().iter() {
        assert!(!selection.schemas.iter().flatten().any(|schema| schema == thought_schema), "a seed selection named thoughts: {selection:?}");
    }
}

#[test]
fn seed_carries_relationships() {
    let mind = TestMind::seeded();
    mind.committed(vec![
        memory("m1", MemorySort::Learned, "Libby wants the sounds before the shaders", &["@libby:chat"]),
        memory("m2", MemorySort::Rapport, "Noor distrusts the new scheduler", &["@noor:chat"]),
        run("w1", RunTurn::SelfRun, RunOperator::Mind, &[]),
    ]);
    mind.committed(vec![PipelineDocument::Reply(PipelineReply {
        instance: Slug(INSTANCE.into()),
        label: "w1-r1".into(),
        run: run_ref("w1"),
        answers: vec![],
        text: "The shader pass is done; the sounds are Libby's.".into(),
    })]);
    let seed = seed_of(&Wrap::over(&mind, &[]), &PersonaRead { state: state(), receipt_id: "r".into() }, None, t(100_000)).unwrap();
    for wanted in ["Libby holds the sounds", "collaborator", "Libby wants the sounds before the shaders", "Noor distrusts the new scheduler", "The shader pass is done", "Coherence over velocity", "The shaders are late"] {
        assert!(seed.context.contains(wanted), "{wanted:?} missing from {}", seed.context);
    }
    assert!(seed.identity.contains("Yggdrasil") && seed.identity.contains("she/her") && seed.identity.contains("Plain."));
    assert!(seed.reads.iter().any(|read| read.kind == PipelineKind::Reply));
    assert!(seed.reads.iter().any(|read| read.kind == PipelineKind::Memory));
}

#[test]
fn seed_sections_keep_budgets() {
    let huge = "x".repeat(3000);
    let mut crowded = state();
    crowded["values"] = json!((0..40).map(|at| json!({ "id": format!("v{at}"), "label": format!("Value {at}"), "priority": 0.5, "summary": huge })).collect::<Vec<_>>());
    crowded["agencyPressure"]["pressures"] = json!((0..40).map(|_| json!({ "summary": huge })).collect::<Vec<_>>());
    let mind = TestMind::seeded();
    mind.committed(vec![memory("m1", MemorySort::Learned, "Libby wants the sounds first", &["@libby:chat"])]);
    let seed = seed_of(&Wrap::over(&mind, &[]), &PersonaRead { state: crowded, receipt_id: "r".into() }, None, t(100_000)).unwrap();
    assert!(seed.context.len() <= SECTION_MAX_BYTES, "{}", seed.context.len());
    assert!(seed.context.contains("Libby holds the sounds"), "oversized values and pressures crowded out the bonds");
    assert!(seed.context.contains("Libby wants the sounds first"), "oversized values and pressures crowded out the recall");
    let values = seed.context.split("What presses on her:").next().unwrap();
    assert!(values.len() <= BUDGETS[0].1 + 1, "values took {}", values.len());
}

#[test]
fn stimulus_is_what_is_new() {
    let mind = TestMind::seeded();
    mind.admit_at(t(100));
    mind.committed(vec![memory("m1", MemorySort::Learned, "Before the thought, the kettle", &[])]);
    mind.admit_at(t(200));
    mind.committed(vec![thought_doc(&ticked(200), "A drift")]);
    mind.admit_at(t(300));
    mind.committed(vec![memory("m2", MemorySort::Decided, "After the thought, the window", &[]), run("w9", RunTurn::SelfRun, RunOperator::Mind, &[])]);
    mind.admit_at(t(350));
    mind.committed(vec![crate::testkit::close(run_ref("w9"), crate::testkit::recorded())]);
    let wrap = Wrap::over(&mind, &[]);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let seed = seed_of(&wrap, &read, Some(t(200)), t(400)).unwrap();
    assert!(seed.news);
    assert!(seed.stimulus.contains("After the thought, the window"), "{}", seed.stimulus);
    assert!(seed.stimulus.contains("w9 closed"), "a run closure is news: {}", seed.stimulus);
    assert!(!seed.stimulus.contains("Before the thought, the kettle"), "{}", seed.stimulus);
    assert!(seed.stimulus.find("closed").unwrap() < seed.stimulus.find("After the thought").unwrap(), "newest first");
    assert!(seed.reads.contains(&reference(PipelineKind::Memory, &format!("{INSTANCE}:memory:m2"))));
}

#[test]
fn stimulus_without_news_is_an_unbidden_memory_chosen_by_the_tick_label_else_the_nothing_line() {
    let mind = TestMind::seeded();
    mind.admit_at(t(100));
    mind.committed(vec![memory("m1", MemorySort::Learned, "The first old thing", &[]), memory("m2", MemorySort::Learned, "The second old thing", &[])]);
    let long_after = 100 * 86_400;
    mind.admit_at(t(long_after - 100));
    mind.committed(vec![memory("m3", MemorySort::Learned, "A recent thing", &[])]);
    let wrap = Wrap::over(&mind, &[]);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let picks: Vec<String> = (0..4)
        .map(|offset| {
            let seed = seed_of(&wrap, &read, Some(t(long_after - 10)), t(long_after + offset)).unwrap();
            assert!(!seed.news);
            seed.stimulus
        })
        .collect();
    assert!(picks.iter().all(|stimulus| stimulus.starts_with("Out of nowhere you recall")), "{picks:?}");
    assert!(picks.iter().all(|stimulus| !stimulus.contains("recent thing")), "a memory under 7 days old is not unbidden");
    assert!(picks.iter().any(|stimulus| stimulus.contains("first old")) && picks.iter().any(|stimulus| stimulus.contains("second old")), "the label seeds a uniform pick: {picks:?}");
    assert_eq!(picks[0], picks[2], "the same label seed picks the same memory");

    let bare = TestMind::seeded();
    let seed = seed_of(&Wrap::over(&bare, &[]), &read, Some(t(1000)), t(5000)).unwrap();
    assert!(!seed.news);
    assert!(seed.stimulus.starts_with("Nothing has happened since"), "{}", seed.stimulus);
}

#[test]
fn cadence_from_newest_thought() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    let organs = organs(&wrap, &persona, &model);
    let tick_at = |secs: i64| tick(&organs, &|| released(), Mode::Live, t(secs)).unwrap();
    assert_eq!(tick_at(1000 + STREAM_MIN_GAP_S - 1), Outcome::NotDue, "no tick before the minimum gap");
    assert_eq!(tick_at(1000 + STREAM_MIN_GAP_S), Outcome::NotDue, "past the minimum gap but nothing is new");
    assert_eq!(model.calls(), 0);
    assert_eq!(tick_at(1000 + STREAM_IDLE_GAP_S - 1), Outcome::NotDue);
    assert_eq!(thoughts(&mind).len(), 1);
    // Something new, past the minimum gap but short of the idle gap.
    mind.admit_at(t(1000 + STREAM_MIN_GAP_S));
    mind.committed(vec![memory("m9", MemorySort::Decided, "Something happened", &[])]);
    let Outcome::Thought { admitted: true, .. } = tick_at(1000 + STREAM_MIN_GAP_S + 1) else { panic!("news past the minimum gap makes a thought") };
    assert_eq!(thoughts(&mind).len(), 2);
}

#[test]
fn the_idle_gap_alone_makes_a_tick_due() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    let Outcome::Thought { admitted: true, thought } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(1000 + STREAM_IDLE_GAP_S)).unwrap() else { panic!("idle gap") };
    assert_eq!(thought.label.0, format!("t{}", t(1000 + STREAM_IDLE_GAP_S).timestamp()));
    assert!(due(None, t(0), false) && !due(Some(t(0)), t(STREAM_MIN_GAP_S), false) && due(Some(t(0)), t(STREAM_MIN_GAP_S), true));
}

#[test]
fn held_brake_ticks_nothing() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    for reason in [HeldReason::BrakeHeld, HeldReason::BrakeAbsent, HeldReason::Undecodable] {
        let held = tick(&organs(&wrap, &persona, &model), &move || Effective::Held { reason }, Mode::Live, t(100_000)).unwrap();
        assert_eq!(held, Outcome::Held(reason));
        let dry = tick(&organs(&wrap, &persona, &model), &move || Effective::Held { reason }, Mode::DryRun, t(100_000)).unwrap();
        assert_eq!(dry, Outcome::Held(reason), "a dry run is held too");
    }
    assert_eq!(model.calls(), 0, "no model call");
    assert!(wrap.admitted.borrow().is_empty() && wrap.selections.borrow().is_empty(), "no admission, and the mind was not read");
}

#[test]
fn least_near_candidate_chosen() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["one", "two", "three", "four"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.9, 0.2, 0.2, 0.5]);
    let Outcome::Thought { thought, admitted: true } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(1000 + STREAM_IDLE_GAP_S)).unwrap() else { panic!("a thought") };
    assert_eq!(thought.nearest_permille, vec![900, 200, 200, 500]);
    assert_eq!(thought.chosen, 1, "the least near, the first on a tie");
    assert_eq!(thought.text.0, "two");
    assert_eq!(thought.candidates.iter().map(|line| line.0.as_str()).collect::<Vec<_>>(), ["one", "two", "three", "four"], "every candidate is kept in the trace");
    assert_eq!(least_near(&[500, 100, 100]), 1);
    assert_eq!(least_near(&[]), 0);
}

#[test]
fn no_earlier_thought_is_nearness_1000() {
    let mind = TestMind::seeded();
    let model = Fake::saying("I turn it over.", &["one", "two"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[]);
    let Outcome::Thought { thought, .. } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(5000)).unwrap() else { panic!("a thought") };
    assert_eq!(thought.nearest_permille, vec![1000, 1000]);
    assert_eq!(thought.chosen, 0);
    assert_eq!((thought.projector_model.0.as_str(), thought.base_model.0.as_str(), thought.persona_receipt.0.as_str()), ("projector:test", "base:test", "mind-commit-1"));
    assert_eq!((thought.temperature_permille, thought.min_p_permille, thought.max_tokens), (1000, 50, 160));
}

#[test]
fn frame_holds_projection_verbatim() {
    assert_eq!(continuer_frame("Yggdrasil", "9 October 2026", "I turn it over."), "Yggdrasil's notebook\n\n9 October 2026\n\nI turn it over.\n\n");
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I keep turning over the sounds.", &["a", "b", "c", "d", "e"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.3, 0.3, 0.3, 0.3]);
    tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(1000 + STREAM_IDLE_GAP_S)).unwrap();
    let continued = model.continued.borrow();
    let (prompt, sampling, n) = &continued[0];
    assert_eq!(prompt, &continuer_frame("Yggdrasil", &t(1000 + STREAM_IDLE_GAP_S).format(FRAME_DATE_FORMAT).to_string(), "I keep turning over the sounds."));
    assert_eq!((*n, *sampling), (4, Sampling::STREAM));
    for foreign in ["Libby", "Coherence", "Plain.", "she/her", "private Projector"] {
        assert!(!prompt.contains(foreign), "{foreign} reached the continuer");
    }
    let projected = model.projected.borrow();
    assert!(projected[0].1.contains("private Projector") && projected[0].1.contains(STREAM_GUIDANCE), "the Projector prompt is Ghostlight's builder over her seed");
    assert!(projected[0].1.contains("Libby holds the sounds"));
}

#[test]
fn blank_and_faulty_ticks_admit_nothing() {
    let mind = TestMind::seeded();
    let persona = persona();
    let blank = Fake::saying("A projection.", &["", "  \n", "\t"]);
    let wrap = Wrap::over(&mind, &[]);
    assert_eq!(tick(&organs(&wrap, &persona, &blank), &|| released(), Mode::Live, t(5000)).unwrap(), Outcome::Blank);
    let silent_projector = Fake::saying("   ", &["x"]);
    assert_eq!(tick(&organs(&wrap, &persona, &silent_projector), &|| released(), Mode::Live, t(5000)).unwrap(), Outcome::Blank);
    let mut down = Fake::saying("x", &["x"]);
    down.fault = true;
    assert!(tick(&organs(&wrap, &persona, &down), &|| released(), Mode::Live, t(5000)).is_err());
    assert!(wrap.admitted.borrow().is_empty() && thoughts(&mind).is_empty());
}

#[test]
fn dry_run_admits_nothing() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift", "Another"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.4, 0.6]);
    let Outcome::Thought { thought, admitted } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::DryRun, t(1001)).unwrap() else { panic!("a dry run prints a thought") };
    assert!(!admitted);
    assert_eq!(thought.text.0, "A new drift");
    assert!(wrap.admitted.borrow().is_empty());
    assert_eq!(thoughts(&mind).len(), 1);
    assert!(serde_json::to_string(&*thought).unwrap().contains("\"candidates\""));
}

#[test]
fn stream_admits_only_thoughts() {
    let mind = TestMind::seeded();
    mind.committed(vec![memory("m1", MemorySort::Learned, "Libby wants the sounds first", &["@libby:chat"])]);
    let model = Fake::saying("I turn it over.", &["A drift", "Another"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[]);
    let Outcome::Thought { thought, admitted: true } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(100_000)).unwrap() else { panic!("a thought") };
    let batches = wrap.admitted.borrow();
    assert_eq!(batches.len(), 1);
    assert!(matches!(batches[0].as_slice(), [PipelineDocument::Thought(_)]), "exactly one thought and nothing else");
    let stored = mind.view(PipelineKind::Thought, &format!("{INSTANCE}:thought:t{}", t(100_000).timestamp())).expect("admitted under its instance-rooted key");
    assert_eq!(stored.document, PipelineDocument::Thought(*thought.clone()));
    assert!(thought.reads.iter().any(|read| read.kind == PipelineKind::Memory), "the thought cites what the seed read");
    assert_eq!(mind.ids(PipelineKind::Memory).len(), 1, "no memory was written");
}

// ---------------------------------------------------------------------------------------------
// The Projector against a real kit server.

use model_connector::{CallerAdmission, ProviderBackend, ProviderCall, ServeLimits, StopReason, TransportEvent, TransportResult, Usage, serve as serve_kit};

const KIT_KEY: &str = "stream-projector-test-key";
const KIT_MODEL: &str = "claude-haiku-5-5";

/// Answers every call as scripted and records the caller and the request the provider would have been shown.
struct Scripted {
    outcome: TransportOutcome,
    words: Vec<&'static str>,
    seen: Mutex<Vec<(String, ProviderRequest)>>,
}

impl Scripted {
    fn ending(stop_reason: StopReason, words: &[&'static str]) -> Arc<Self> {
        let usage = Usage { input_tokens: 1, cache_read_tokens: 0, cache_write_tokens: 0, output_tokens: 1, reasoning_tokens: None };
        Arc::new(Self { outcome: TransportOutcome::Completed { provider_response_id: Some("msg-1".into()), stop_reason, usage }, words: words.to_vec(), seen: Mutex::default() })
    }
}

impl ProviderBackend for Scripted {
    fn ready(&self) -> Result<(), &'static str> {
        Ok(())
    }

    fn provider(&self) -> &str {
        "anthropic-messages"
    }

    fn execute(&self, call: &ProviderCall) -> TransportResult {
        self.seen.lock().unwrap().push((call.caller_runtime_id().to_owned(), call.request().clone()));
        let events = self.words.iter().enumerate().map(|(at, text)| TransportEvent { sequence: at as u64, payload: TransportEventPayload::TextDelta { text: (*text).into() } }).collect();
        call.transported(events, self.outcome.clone())
    }
}

/// A running kit whose caller row for `instance` allows `allowed`, and a Projector opened on it.
fn kit(backend: Arc<Scripted>, allowed: &str) -> (KitProjector, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("replay.cc");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let admission = CallerAdmission::new(INSTANCE, KIT_KEY, 1, [allowed.to_owned()], 2, 64 * 1024, 4096).unwrap();
    let limits = ServeLimits { max_frame_bytes: 512 * 1024, max_connections: 8, socket_timeout_ms: 5000, max_expiry_skew_ms: 300_000 };
    std::thread::spawn(move || serve_kit(listener, limits, vec![admission], &store, backend));
    (KitProjector::open(endpoint, KIT_KEY.into(), INSTANCE, KIT_MODEL, Duration::from_secs(5)).unwrap(), root)
}

#[test]
fn projector_is_one_kit_turn() {
    let backend = Scripted::ending(StopReason::EndTurn, &["I turn ", "it over."]);
    let (projector, _root) = kit(Arc::clone(&backend), KIT_MODEL);
    let id = "mind-stream:eureka:t100";
    assert_eq!(projector.project(id, "The projector prompt.").unwrap(), "I turn it over.");
    let seen = backend.seen.lock().unwrap();
    let (caller, request) = &seen[0];
    assert_eq!(caller, INSTANCE, "her instance is the caller");
    assert_eq!((request.request_id.as_str(), request.conversation_id.as_str(), request.model.as_str()), (id, id, KIT_MODEL));
    assert_eq!(request.input, [InputItem::UserText { text: "The projector prompt.".into() }]);
    assert_eq!(request.instructions, PROJECTOR_INSTRUCTIONS);
    assert_eq!(request.reasoning_effort, Some(ReasoningEffort::Low));
    assert!(request.tools.is_empty() && request.output_schema_json.is_none());
    assert_eq!(request.max_output_tokens, PROJECTOR_MAX_OUTPUT_TOKENS);
}

#[test]
fn each_projection_is_its_own_conversation() {
    let backend = Scripted::ending(StopReason::EndTurn, &["One."]);
    let (projector, _root) = kit(Arc::clone(&backend), KIT_MODEL);
    projector.project("mind-stream:eureka:t100", "first").unwrap();
    projector.project("mind-stream:eureka:t200", "second").unwrap();
    let seen = backend.seen.lock().unwrap();
    assert_ne!(seen[0].1.request_id, seen[1].1.request_id);
    assert_ne!(seen[0].1.conversation_id, seen[1].1.conversation_id);
    assert_eq!(seen[1].1.input, [InputItem::UserText { text: "second".into() }], "no transcript is carried");
}

#[test]
fn a_kit_refusal_is_a_tick_fault() {
    let backend = Scripted::ending(StopReason::EndTurn, &["Never said."]);
    let (projector, _root) = kit(Arc::clone(&backend), "some-other-model");
    let mind = mind_with_thought(1000);
    let continuer = Fake::saying("unused", &["unused"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[]);
    let organs = Organs { mind: &wrap, persona: &persona, projector: &projector, continuer: &continuer, base_model: "base:test" };
    let fault = format!("{:#}", tick(&organs, &|| released(), Mode::Live, t(1000 + STREAM_IDLE_GAP_S)).unwrap_err());
    assert!(fault.contains("ModelNotAllowed"), "{fault}");
    assert!(backend.seen.lock().unwrap().is_empty(), "the provider was never reached");
    assert_eq!(continuer.calls(), 0, "the continuer is not called");
    assert!(wrap.admitted.borrow().is_empty());
}

#[test]
fn a_thought_names_the_model_the_kit_ran() {
    let backend = Scripted::ending(StopReason::EndTurn, &["I turn it over."]);
    let (projector, _root) = kit(Arc::clone(&backend), KIT_MODEL);
    let mind = mind_with_thought(1000);
    let continuer = Fake::saying("unused", &["A drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    let organs = Organs { mind: &wrap, persona: &persona, projector: &projector, continuer: &continuer, base_model: "base:test" };
    let Outcome::Thought { thought, .. } = tick(&organs, &|| released(), Mode::Live, t(1000 + STREAM_IDLE_GAP_S)).unwrap() else { panic!("a thought") };
    let ran = backend.seen.lock().unwrap()[0].1.model.clone();
    assert_eq!(ran, KIT_MODEL);
    assert_eq!(thought.projector_model.0, ran, "the recorded model is the one the kit was asked for");
}

#[test]
fn the_instructions_restate_no_membrane_text() {
    let membrane = build_projector_prompt(&ProjectorPrompt { identity: "", typed_context: "", visible_stimulus: "", domain_guidance: STREAM_GUIDANCE, word_budget: PROJECTOR_WORDS }).to_lowercase();
    let words: Vec<&str> = PROJECTOR_INSTRUCTIONS.split_whitespace().collect();
    assert!(!PROJECTOR_INSTRUCTIONS.contains('\n') && words.len() <= 20, "one short line");
    for run in words.windows(4) {
        let run = run.join(" ").to_lowercase();
        assert!(!membrane.contains(&run), "Ghostlight's membrane text reached the instructions: {run}");
    }
}

#[test]
fn the_connector_key_is_read_trimmed_and_its_absence_leaks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("connector-key"), "canary-key-7f3a91\n").unwrap();
    assert_eq!(connector_key(dir.path()).unwrap(), "canary-key-7f3a91");
    let empty = tempfile::tempdir().unwrap();
    let fault = format!("{:#}", connector_key(empty.path()).unwrap_err());
    assert!(fault.contains("connector-key") && !fault.contains(empty.path().to_str().unwrap()), "{fault}");
    let bad = tempfile::tempdir().unwrap();
    std::fs::write(bad.path().join("connector-key"), b"canary-key-7f3a91\xff\xfe").unwrap();
    let fault = format!("{:#}", connector_key(bad.path()).unwrap_err());
    assert!(!fault.contains("canary"), "no byte of the key reaches an error: {fault}");
}

#[test]
fn a_projection_cut_short_is_a_fault() {
    for stop in [StopReason::MaxOutputTokens, StopReason::Refusal, StopReason::ToolUse, StopReason::ContextWindow] {
        let (projector, _root) = kit(Scripted::ending(stop, &["Half a thou"]), KIT_MODEL);
        let fault = format!("{:#}", projector.project("mind-stream:eureka:t1", "p").unwrap_err());
        assert!(fault.contains("ended early") && !fault.contains("Half a thou"), "{stop:?}: {fault}");
    }
    let failed = Arc::new(Scripted { outcome: TransportOutcome::Failed { kind: model_connector::FailureKind::RateLimited, message: "slow down".into(), usage: None }, words: vec![], seen: Mutex::default() });
    let (projector, _root) = kit(failed, KIT_MODEL);
    assert!(format!("{:#}", projector.project("mind-stream:eureka:t1", "p").unwrap_err()).contains("RateLimited"));
}

#[test]
fn non_loopback_connector_refused() {
    for refused in ["10.77.0.1:4103", "192.0.2.1:4103", "[2001:db8::1]:4103"] {
        assert!(KitProjector::open(refused.parse().unwrap(), KIT_KEY.into(), INSTANCE, KIT_MODEL, Duration::from_secs(1)).is_err(), "{refused} must be refused");
    }
    for opened in ["127.0.0.1:4103", "[::1]:4103"] {
        assert!(KitProjector::open(opened.parse().unwrap(), KIT_KEY.into(), INSTANCE, KIT_MODEL, Duration::from_secs(1)).is_ok(), "{opened} is loopback");
    }
}

#[test]
fn request_ids_name_the_tick_and_a_dry_run_never_takes_the_live_one() {
    let mind = mind_with_thought(1000);
    let persona = persona();
    let at = t(1000 + STREAM_IDLE_GAP_S);
    let run = |mode, now: DateTime<Utc>| {
        let model = Fake::saying("I turn it over.", &["A drift"]);
        let wrap = Wrap::over(&mind, &[0.1]);
        tick(&organs(&wrap, &persona, &model), &|| released(), mode, now).unwrap();
        model.projected.into_inner().remove(0).0
    };
    let live = run(Mode::Live, at);
    assert_eq!(live, format!("mind-stream:{INSTANCE}:t{}", at.timestamp()));
    let dry = run(Mode::DryRun, at);
    let later_dry = run(Mode::DryRun, at + Span::milliseconds(5));
    assert_ne!(dry, live, "the kit's replay store would answer the live id byte-identically");
    assert!(dry.starts_with(&format!("{live}:dry")), "{dry}");
    assert_ne!(dry, later_dry, "two dry runs of one label do not collide");
    let next = run(Mode::Live, at + Span::seconds(STREAM_IDLE_GAP_S));
    assert_ne!(next, live, "each tick is its own conversation");
}

// ---------------------------------------------------------------------------------------------
// The continuer against a fake HTTP server.

type Seen = Arc<Mutex<Vec<(String, Value)>>>;

fn serve(answers: Vec<String>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let record = Arc::clone(&seen);
    std::thread::spawn(move || {
        for answer in answers {
            let (mut socket, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut buffer = [0u8; 4096];
            let (head, length) = loop {
                let read = socket.read(&mut buffer).unwrap();
                raw.extend_from_slice(&buffer[..read]);
                if let Some(end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&raw[..end]).to_string();
                    let length = head.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|value| value.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if raw.len() >= end + 4 + length {
                        break (head, (end + 4, length));
                    }
                }
                assert!(read > 0, "the client closed early");
            };
            let body: Value = serde_json::from_slice(&raw[length.0..length.0 + length.1]).unwrap();
            let path = head.lines().next().unwrap().split_whitespace().nth(1).unwrap().to_owned();
            record.lock().unwrap().push((path, body));
            write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}", answer.len()).unwrap();
        }
    });
    (endpoint, seen)
}

#[test]
fn raw_generate_is_raw() {
    let (endpoint, seen) = serve(vec![json!({ "response": "one" }).to_string(), json!({ "response": "two" }).to_string()]);
    let model = OllamaContinuer::open(&endpoint, "base:y").unwrap();
    let out = model.continue_raw("The frame\n\n", &Sampling::STREAM, 2).unwrap();
    assert_eq!(out, ["one", "two"]);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "one call per continuation");
    for (path, body) in seen.iter() {
        assert_eq!(path, "/api/generate");
        assert_eq!(body["raw"], json!(true));
        assert_eq!(body["stream"], json!(false));
        assert_eq!(body["model"], json!("base:y"));
        assert_eq!(body["prompt"], json!("The frame\n\n"));
        assert_eq!(body["options"], json!({ "temperature": 1.0, "min_p": 0.05, "num_predict": 160, "stop": ["\n\n"], "num_thread": 4 }));
    }
}

#[test]
fn a_model_host_that_answers_badly_is_a_fault() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 4096];
        let _ = socket.read(&mut buffer);
        let _ = socket.write_all(b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 4\r\nconnection: close\r\n\r\nboom");
    });
    let model = OllamaContinuer::open(&endpoint, "b").unwrap();
    let fault = model.continue_raw("x", &Sampling::STREAM, 1).unwrap_err().to_string();
    assert!(fault.contains("500"), "{fault}");
}

#[test]
fn non_loopback_endpoint_refused() {
    for refused in [
        "http://localhost:11434@example.com",
        "http://localhost@example.com",
        "http://127.0.0.1:11434@example.com:80",
        "http://user:secret@127.0.0.1:11434",
        "http://127.0.0.1:11434@localhost",
        "http://[::1]:11434@example.com",
        "http://localhost:11434\\@example.com",
        "http://example.com#@127.0.0.1",
        "http://example.com?@localhost",
        "http://127.0.0.1:11434?x=1",
        "http://localhost.example.com:11434",
        "http://",
        "",
    ] {
        assert!(OllamaContinuer::open(refused, "b").is_err(), "{refused} must be refused");
    }
    for refused in ["http://10.77.0.1:11434", "http://example.com:11434", "http://0.0.0.0:11434", "https://127.0.0.1:11434", "127.0.0.1:11434", "http://127.0.0.1:11434/api", "http://[2001:db8::1]:11434"] {
        assert!(OllamaContinuer::open(refused, "b").is_err(), "{refused} must be refused");
    }
    for opened in ["http://127.0.0.1:11434", "http://localhost:11434", "http://[::1]:11434", "http://127.0.0.2:1/", "http://LOCALHOST:11434/"] {
        assert!(OllamaContinuer::open(opened, "b").is_ok(), "{opened} is loopback");
    }
}

#[test]
fn cap_cuts_at_a_line_else_at_a_character() {
    assert_eq!(cap("short", 10), "short");
    assert_eq!(cap("ab\ncd", 4), "ab", "whole lines only");
    assert_eq!(cap("abcdef", 4), "abcd", "no newline to cut at");
    assert_eq!(cap("\nabc", 3), "\nab", "a newline at the very start is no cut point");
    assert_eq!(cap("aé\nb", 2), "a", "never inside a character");
    assert_eq!(cap("ééé", 5), "éé");
}

#[test]
fn bonds_name_the_label_else_the_id_else_someone() {
    let line = |bond: Value| bond_line(&bond);
    assert_eq!(line(json!({ "object": { "label": "Libby", "id": "@libby:chat" }, "relationshipKind": "friend", "summary": "s" })), "- Libby (friend): s");
    assert_eq!(line(json!({ "object": { "label": "", "id": "@libby:chat" }, "relationshipKind": "friend", "summary": "s" })), "- @libby:chat (friend): s");
    assert_eq!(line(json!({ "object": {}, "relationshipKind": "friend", "summary": "s" })), "- someone (friend): s");
}

#[test]
fn only_person_bonds_with_ids_are_people_she_asks_her_memories_about() {
    let state = json!({ "affect": { "socialBonds": [
        { "object": { "kind": "person", "id": "@libby:chat" } },
        { "object": { "kind": "repo", "id": "GameCult/Huginn" } },
        { "object": { "kind": "person", "id": "" } },
        { "object": { "kind": "person", "id": "@noor:chat" } },
    ] } });
    assert_eq!(bonded_people(&state), ["@libby:chat", "@noor:chat"]);
    assert!(bonded_people(&json!({})).is_empty());
}

#[test]
fn news_is_strictly_after_her_newest_thought() {
    let mind = TestMind::seeded();
    mind.admit_at(t(500));
    mind.committed(vec![memory("m1", MemorySort::Learned, "Admitted at the very instant of the thought", &[])]);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let seed = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(500)), t(600)).unwrap();
    assert!(!seed.news, "{}", seed.stimulus);
    let seed = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(499)), t(600)).unwrap();
    assert!(seed.news);
}

#[test]
fn an_unbidden_memory_is_older_than_seven_days_not_exactly_seven() {
    let mind = TestMind::seeded();
    mind.admit_at(t(1000));
    mind.committed(vec![memory("m1", MemorySort::Learned, "The week-old thing", &[])]);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let week = 7 * 86_400;
    let exactly = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(1000)), t(1000 + week)).unwrap();
    assert!(exactly.stimulus.starts_with("Nothing has happened"), "{}", exactly.stimulus);
    let over = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(1000)), t(1001 + week)).unwrap();
    assert!(over.stimulus.contains("The week-old thing"), "{}", over.stimulus);
}

#[test]
fn a_flood_of_news_fills_the_stimulus_with_whole_lines_newest_first() {
    let mind = TestMind::seeded();
    let flood: Vec<PipelineDocument> = (0..60).map(|at| memory(&format!("f{at:02}"), MemorySort::Learned, &format!("{at:02} {}", "w".repeat(150)), &[])).collect();
    mind.admit_at(t(100));
    mind.committed(flood);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let seed = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(50)), t(200)).unwrap();
    let lines: Vec<&str> = seed.stimulus.lines().skip(1).collect();
    assert!(seed.stimulus.len() <= SECTION_MAX_BYTES && seed.stimulus.len() > SECTION_MAX_BYTES - 250, "{}", seed.stimulus.len());
    assert!(lines.iter().all(|line| line.starts_with("- You noted") && line.ends_with('"')), "a line was cut: {lines:?}");
    assert!(lines.len() > 10 && lines.len() < 30, "{}", lines.len());
    let cited = seed.reads.iter().filter(|read| read.kind == PipelineKind::Memory).count();
    assert!(cited >= lines.len() && cited <= lines.len() + 12, "the thought cites what its stimulus showed: {cited} reads for {} lines", lines.len());
    let newest = lines[0];
    assert!(newest.contains("\"29 ") || newest.contains("\"59 "), "newest first: {newest}");
}

#[test]
fn the_newest_thought_sets_the_cadence_not_the_oldest() {
    let mind = TestMind::seeded();
    mind.admit_at(t(100));
    mind.committed(vec![thought_doc(&ticked(100), "An old drift")]);
    mind.admit_at(t(10_000));
    mind.committed(vec![thought_doc(&ticked(10_000), "A recent drift")]);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    let tick_at = |secs: i64| tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(secs)).unwrap();
    assert_eq!(tick_at(10_000 + STREAM_MIN_GAP_S - 1), Outcome::NotDue, "a recent thought holds the stream back whatever the oldest says");
    assert_eq!(model.calls(), 0);
}

#[test]
fn news_at_exactly_the_minimum_gap_is_due() {
    let mind = mind_with_thought(1000);
    mind.admit_at(t(1500));
    mind.committed(vec![memory("m9", MemorySort::Decided, "Something happened", &[])]);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    let Outcome::Thought { admitted: true, .. } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(1000 + STREAM_MIN_GAP_S)).unwrap() else { panic!("due at the minimum gap") };
}

#[test]
fn repo_activity_is_news_only_after_her_newest_thought_and_is_stamped_when_it_happened() {
    let mind = mind_with_thought(1000);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let old = Wrap::over(&mind, &[]).with_forge(vec![push(1, 500)]);
    let seed = seed_of(&old, &read, Some(t(1000)), t(1901)).unwrap();
    assert!(!seed.news && !seed.stimulus.contains("Changes in your repos"), "an event older than her newest thought is not news: {}", seed.stimulus);
    let edge = Wrap::over(&mind, &[]).with_forge(vec![push(1, 1000)]);
    assert!(!seed_of(&edge, &read, Some(t(1000)), t(1901)).unwrap().news, "an event at her newest thought's time is not after it");
    let mixed = Wrap::over(&mind, &[]).with_forge(vec![push(2, 1500), push(1, 500)]);
    let seed = seed_of(&mixed, &read, Some(t(1000)), t(1901)).unwrap();
    assert!(seed.news && seed.stimulus.contains("Changes in your repos: GameCult/Huginn: 1 commit pushed by libby"), "{}", seed.stimulus);
    let stamp = t(1500).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    assert!(seed.stimulus.contains(&format!("at={stamp} ")), "the notice carries the event's time, not the tick's: {}", seed.stimulus);
    let first = Wrap::over(&mind, &[]).with_forge(vec![push(1, 500)]);
    assert!(seed_of(&first, &read, None, t(1901)).unwrap().news, "with no thought yet, what the forge holds is news");
}

#[test]
fn a_capped_repo_is_news_only_beside_a_forge_event_after_her_newest_thoughts_label() {
    let mind = mind_with_thought(1000);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let mut quiet = Wrap::over(&mind, &[]).with_forge(vec![push(1, 500)]);
    quiet.forge.as_mut().unwrap().capped.push("GameCult/Huginn".into());
    assert!(!seed_of(&quiet, &read, Some(t(1000)), t(1901)).unwrap().news);
    let mut busy = Wrap::over(&mind, &[]).with_forge(vec![push(2, 1500)]);
    busy.forge.as_mut().unwrap().capped.push("GameCult/Huginn".into());
    let seed = seed_of(&busy, &read, Some(t(1000)), t(1901)).unwrap();
    assert!(seed.news && seed.stimulus.to_lowercase().contains("page limit"), "{}", seed.stimulus);
}

#[test]
fn old_repo_activity_does_not_make_a_tick_due() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = CountedPersona(persona(), std::cell::Cell::new(0));
    let wrap = Wrap::over(&mind, &[0.1]).with_forge(vec![push(1, 500)]);
    let organs = Organs { mind: &wrap, persona: &persona, projector: &model, continuer: &model, base_model: "base:test" };
    assert_eq!(tick(&organs, &|| released(), Mode::Live, t(1901)).unwrap(), Outcome::NotDue);
    assert_eq!((model.calls(), persona.1.get(), thoughts(&mind).len()), (0, 0, 1), "no model call, no seed, no thought");
}

#[test]
fn new_repo_activity_makes_a_tick_due_and_the_hourly_memory_comes_for_an_active_steward() {
    let week = 8 * 86_400;
    let mind = TestMind::seeded();
    mind.admit_at(t(100));
    mind.committed(vec![memory("m1", MemorySort::Learned, "An old thing she holds", &[])]);
    mind.admit_at(t(week));
    mind.committed(vec![thought_doc(&ticked(week), "An earlier drift")]);
    let model = Fake::saying("I turn it over.", &["A new drift", "Another"]);
    let persona = persona();
    let fresh = Wrap::over(&mind, &[0.1, 0.2]).with_forge(vec![push(2, week + 1000)]);
    let Outcome::Thought { thought, admitted: true } = tick(&organs(&fresh, &persona, &model), &|| released(), Mode::Live, t(week + 2 * STREAM_MIN_GAP_S)).unwrap() else { panic!("new activity at the next gap mark") };
    assert!(thought.stimulus.0.contains("Changes in your repos"), "{}", thought.stimulus.0);

    let model = Fake::saying("I turn it over.", &["Yet another"]);
    let quiet = Wrap::over(&mind, &[0.3]).with_forge(vec![push(1, week - 5000)]);
    let at = week + 1000 + STREAM_MIN_GAP_S + STREAM_IDLE_GAP_S + 10;
    let Outcome::Thought { thought, admitted: true } = tick(&organs(&quiet, &persona, &model), &|| released(), Mode::Live, t(at)).unwrap() else { panic!("the idle gap") };
    assert!(thought.stimulus.0.starts_with("Out of nowhere you recall"), "old activity is not news, so the hourly memory comes: {}", thought.stimulus.0);
}

#[test]
fn reads_cite_only_what_the_context_showed() {
    let mind = TestMind::seeded();
    let memories: Vec<PipelineDocument> = (0..12).map(|at| memory(&format!("m{at:02}"), MemorySort::Learned, &format!("zz{at:02} {}", "x".repeat(290)), &["@libby:chat"])).collect();
    mind.admit_at(t(100));
    mind.committed(memories);
    mind.admit_at(t(200));
    mind.committed(vec![run("w1", RunTurn::SelfRun, RunOperator::Mind, &[])]);
    mind.committed(
        (1..=3)
            .map(|at| PipelineDocument::Reply(PipelineReply { instance: Slug(INSTANCE.into()), label: format!("w1-r{at}").as_str().into(), run: run_ref("w1"), answers: vec![], text: format!("rp{at} {}", "y".repeat(400)).as_str().into() }))
            .collect(),
    );
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let seed = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(300)), t(400)).unwrap();
    assert!(!seed.news);
    let cited: Vec<String> = seed.reads.iter().filter(|read| read.kind == PipelineKind::Memory).map(|read| read.id.0.rsplit(':').next().unwrap().to_owned()).collect();
    assert!(!cited.is_empty() && cited.len() < 12, "{cited:?}");
    for label in &cited {
        assert!(seed.context.contains(&format!("zz{}", &label[1..])), "{label} is cited but not shown");
    }
    let shown = (0..12).filter(|at| seed.context.contains(&format!("zz{at:02}"))).count();
    assert_eq!(shown, cited.len(), "every memory the context shows is cited");
    let replies_cited = seed.reads.iter().filter(|read| read.kind == PipelineKind::Reply).count();
    let replies_shown = (1..=3).filter(|at| seed.context.contains(&format!("rp{at} "))).count();
    assert!((1..3).contains(&replies_shown), "the replies section is bounded: {replies_shown}");
    assert_eq!(replies_cited, replies_shown, "replies cited are the replies shown");
}

#[test]
fn every_thought_records_the_fixed_parts_of_its_prompts() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]);
    let Outcome::Thought { thought, .. } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(1000 + STREAM_IDLE_GAP_S)).unwrap() else { panic!("idle gap") };
    let version = &thought.prompt_version.0;
    assert_eq!(version, &prompt_version());
    let guidance = format!("{STREAM_GUIDANCE:?}");
    for part in [guidance.as_str(), GHOSTLIGHT_REV, "words=180", "<name>'s notebook", "<projection>", "<date %-d %B %Y>"] {
        assert!(version.contains(part), "{part} missing from {version}");
    }
    assert!(version.len() <= LINE_MAX_BYTES, "{}", version.len());
    assert!(include_str!("../../Cargo.toml").contains(GHOSTLIGHT_REV), "the recorded revision is the manifest's pin");
    let rebuilt = build_projector_prompt(&ProjectorPrompt { identity: &thought.identity.0, typed_context: &thought.context.0, visible_stimulus: &thought.stimulus.0, domain_guidance: STREAM_GUIDANCE, word_budget: PROJECTOR_WORDS });
    assert_eq!(model.projected.borrow()[0].1, rebuilt, "the trace rebuilds the Projector's exact prompt");
}


/// A thought ticked at 900 s and admitted at 1000 s: what landed at 950 s while the models ran is news to the next one.
#[test]
fn what_lands_while_she_thinks_is_news_to_the_next_thought() {
    let mind = TestMind::seeded();
    mind.admit_at(t(950));
    mind.committed(vec![memory("m9", MemorySort::Decided, "Landed while the Projector ran", &[])]);
    mind.admit_at(t(1000));
    mind.committed(vec![thought_doc(&ticked(900), "A drift that took a hundred seconds")]);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]).with_forge(vec![push(2, 1000), push(1, 950), push(0, 899)]);
    let Outcome::Thought { thought, admitted: true } = tick(&organs(&wrap, &persona, &model), &|| released(), Mode::Live, t(1901)).unwrap() else { panic!("news past the gap") };
    assert!(thought.stimulus.0.contains("Landed while the Projector ran"), "{}", thought.stimulus.0);
    assert!(thought.stimulus.0.contains("2 commits pushed by libby"), "the pushes at 950 s and 1000 s, not the one at 899 s: {}", thought.stimulus.0);
}

#[test]
fn a_capped_repo_is_judged_by_its_own_events_and_disclosed_when_they_do_not_decide_it() {
    let mind = mind_with_thought(1000);
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let cultlib = |id, secs| RepoEvent { repo: "GameCult/CultLib".into(), ..push(id, secs) };
    let mut two = Wrap::over(&mind, &[]).with_forge(vec![push(2, 1500), cultlib(1, 500)]);
    two.forge.as_mut().unwrap().capped = vec!["GameCult/Huginn".into(), "GameCult/CultLib".into()];
    let seed = seed_of(&two, &read, Some(t(1000)), t(1901)).unwrap();
    assert!(seed.stimulus.to_lowercase().contains("page limit") && seed.stimulus.contains("GameCult/Huginn"), "the repo no event decides stays disclosed: {}", seed.stimulus);
    assert!(!seed.stimulus.contains("GameCult/CultLib"), "an event before her thought decides CultLib's cap: {}", seed.stimulus);
    let mut flooded = Wrap::over(&mind, &[]).with_forge(vec![]);
    flooded.forge.as_mut().unwrap().capped = vec!["GameCult/Huginn".into()];
    let seed = seed_of(&flooded, &read, Some(t(1000)), t(1901)).unwrap();
    assert!(seed.news && seed.stimulus.to_lowercase().contains("page limit"), "a cap with no event at all is disclosed: {}", seed.stimulus);
}

#[test]
fn a_quiet_hour_reads_the_forge_at_each_gap_mark_only() {
    let mind = mind_with_thought(1000);
    let model = Fake::saying("I turn it over.", &["A new drift"]);
    let persona = persona();
    let wrap = Wrap::over(&mind, &[0.1]).with_forge(vec![push(1, 500)]);
    let organs = organs(&wrap, &persona, &model);
    let mut secs = 1000 + STREAM_POLL_S;
    while secs < 1000 + STREAM_IDLE_GAP_S {
        assert_eq!(tick(&organs, &|| released(), Mode::Live, t(secs)).unwrap(), Outcome::NotDue, "at {secs}");
        secs += STREAM_POLL_S;
    }
    assert_eq!(wrap.forge_reads.get(), 3, "one read for each mark crossed at 900, 1800 and 2700 s");
    assert_eq!(model.calls(), 0);
}

#[test]
fn identical_memory_lines_are_each_cited_once() {
    let mind = TestMind::seeded();
    mind.admit_at(t(100));
    mind.committed((0..12).map(|at| memory(&format!("m{at:02}"), MemorySort::Learned, &format!("the same thing {}", "x".repeat(290)), &["@libby:chat"])).collect());
    let read = PersonaRead { state: state(), receipt_id: "r".into() };
    let seed = seed_of(&Wrap::over(&mind, &[]), &read, Some(t(300)), t(400)).unwrap();
    let shown = seed.context.lines().filter(|line| line.contains("the same thing")).count();
    let cited = seed.reads.iter().filter(|read| read.kind == PipelineKind::Memory).count();
    assert!(shown >= 1 && cited == shown, "{shown} lines shown, {cited} cited");
}
