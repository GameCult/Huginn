//! The night's ledger rules, one behaviour each: what the decoder refuses and tells back, what an operation
//! admits, how operations pack into batches, and which memories a night is shown. The whole night through a
//! scripted child is in `persona::tests`.

use eureka_pipeline::{MemoryOrigin, MemorySort, PipelineDocument, ResolutionOutcome, RunOperator, RunTurn};
use serde_json::json;

use super::*;
use crate::testkit::{INSTANCE, TestMind, at, memory_from, run, run_ref};

const T0: i64 = 1_000_000_000;

fn dm(room: &str, with: &[&str]) -> MemoryOrigin {
    MemoryOrigin::Direct { room: room.into(), with: with.iter().map(|who| (*who).into()).collect() }
}

fn wake() -> MemoryOrigin {
    MemoryOrigin::Wake
}

fn id(label: &str) -> String {
    format!("{INSTANCE}:memory:{label}")
}

fn held(label: &str, sort: MemorySort, about: &[&str], origin: MemoryOrigin, ordinal: u64) -> Held {
    let PipelineDocument::Memory(memory) = memory_from(label, sort, "Text.", about, origin, 500) else { unreachable!() };
    Held { id: id(label), memory, at: at(T0 + i64::try_from(ordinal).unwrap()), ordinal }
}

/// A night over four memories: two in her day, one open ask, three elsewhere (public, and two DMs).
fn ledger() -> Context {
    let day = vec![held("d1", MemorySort::Learned, &["@kiko:chat"], wake(), 4), held("d2", MemorySort::Decided, &[], wake(), 5)];
    let open = vec![held("ask", MemorySort::Asked, &["@op:chat"], wake(), 1)];
    let others = vec![held("x1", MemorySort::Learned, &[], wake(), 2), held("dmA", MemorySort::Learned, &[], dm("!a:chat", &["@a:chat"]), 3), held("dmB", MemorySort::Learned, &[], dm("!b:chat", &["@b:chat"]), 6)];
    let all = day.iter().chain(&open).chain(&others).map(|held| (held.id.clone(), held.clone())).collect();
    Context { now: at(T0 + 100), label: "mind-night".into(), counts: BTreeMap::new(), day, open, tide: Vec::new(), all }
}

fn reply(keep: &[&str]) -> serde_json::Value {
    json!({"night": "A night.", "keep": keep.iter().map(|label| id(label)).collect::<Vec<_>>(), "consolidate": [], "retire": [], "remember": []})
}

fn fault(reply: &serde_json::Value) -> String {
    decode(&reply.to_string(), &ledger()).expect_err("the reply should be refused")
}

#[test]
fn every_memory_of_her_day_is_disposed_of_exactly_once() {
    assert!(decode(&reply(&["d1", "d2"]).to_string(), &ledger()).is_ok());
    let missing = fault(&reply(&["d1"]));
    assert!(missing.contains("not covered") && missing.contains(&id("d2")) && !missing.contains(&format!("{}, ", id("d1"))), "{missing}");
    // Disposed of in any of the three ways counts.
    let mut by_retire = reply(&["d1"]);
    by_retire["retire"] = json!([{"item": id("d2"), "outcome": "faded", "why": "It faded."}]);
    assert!(decode(&by_retire.to_string(), &ledger()).is_ok());
    let mut by_consolidation = reply(&[]);
    by_consolidation["consolidate"] = json!([{"consumes": [id("d1"), id("d2")], "sort": "Learned", "text": "Both.", "about": [], "salience_permille": 500}]);
    assert!(decode(&by_consolidation.to_string(), &ledger()).is_ok());
    // Twice, within a list or across them, is refused and the id named.
    let mut twice = reply(&["d1", "d2"]);
    twice["retire"] = json!([{"item": id("d2"), "outcome": "done", "why": "Done."}]);
    assert!(fault(&twice).contains(&format!("{} is named more than once", id("d2"))));
    assert!(fault(&reply(&["d1", "d1", "d2"])).contains("named more than once"));
    let mut across = reply(&["d2"]);
    across["consolidate"] = json!([
        {"consumes": [id("d1")], "sort": "Learned", "text": "One.", "about": [], "salience_permille": 500},
        {"consumes": [id("d1")], "sort": "Learned", "text": "Again.", "about": [], "salience_permille": 500}
    ]);
    assert!(fault(&across).contains("named more than once"));
}

#[test]
fn an_id_must_be_a_memory_of_hers_in_force_whether_or_not_it_is_of_the_day() {
    assert!(decode(&reply(&["d1", "d2", "ask", "x1"]).to_string(), &ledger()).is_ok(), "older items are hers to touch");
    let ghost = fault(&reply(&["d1", "d2", "ghost"]));
    assert!(ghost.contains("ghost is not a memory of yours in force"), "{ghost}");
    let mut retiring = reply(&["d1", "d2"]);
    retiring["retire"] = json!([{"item": "yggdrasil:run:not-a-memory", "outcome": "done", "why": "No."}]);
    assert!(fault(&retiring).contains("not a memory of yours"));
}

#[test]
fn a_consolidation_merges_one_conversation_only() {
    let consolidate = |consumes: &[&str]| {
        let mut r = reply(&["d1", "d2"].iter().filter(|label| !consumes.contains(label)).copied().collect::<Vec<_>>());
        r["consolidate"] = json!([{"consumes": consumes.iter().map(|label| id(label)).collect::<Vec<_>>(), "sort": "Learned", "text": "Merged.", "about": [], "salience_permille": 500}]);
        r
    };
    assert!(fault(&consolidate(&["d1", "dmA"])).contains("one conversation only"), "public with a DM");
    assert!(fault(&consolidate(&["dmA", "dmB", "d1", "d2"])).contains("one conversation only"), "two DMs");
    for same in [&["d1", "d2", "x1"][..], &["dmA"][..]] {
        let mut r = consolidate(same);
        r["keep"] = json!(["d1", "d2"].iter().filter(|label| !same.contains(label)).map(|label| id(label)).collect::<Vec<_>>());
        assert!(decode(&r.to_string(), &ledger()).is_ok(), "{same:?}");
    }
}

#[test]
fn a_reply_holds_the_bounds_its_schema_states() {
    let with = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut r = reply(&["d1", "d2"]);
        edit(&mut r);
        r
    };
    let long = "x".repeat(901);
    assert!(fault(&with(&|r| r["night"] = json!(long.clone()))).contains("night"));
    assert!(fault(&with(&|r| r["night"] = json!("  "))).contains("night"));
    assert!(decode(&with(&|r| r["night"] = json!("x".repeat(900))).to_string(), &ledger()).is_ok());
    let item = |text: &str| json!({"sort": "Idea", "text": text, "about": [], "salience_permille": 100});
    assert!(fault(&with(&|r| r["remember"] = json!((0..17).map(|n| item(&format!("n{n}"))).collect::<Vec<_>>()))).contains("at most"));
    assert!(decode(&with(&|r| r["remember"] = json!((0..16).map(|n| item(&format!("n{n}"))).collect::<Vec<_>>())).to_string(), &ledger()).is_ok());
    assert!(fault(&with(&|r| r["remember"] = json!([item(&"y".repeat(1001))]))).contains("remember.text"));
    assert!(fault(&with(&|r| r["remember"] = json!([{"sort": "Idea", "text": "t", "about": [], "salience_permille": 1001}]))).contains("salience"));
    assert!(fault(&with(&|r| r["retire"] = json!([{"item": id("x1"), "outcome": "done", "why": ""}]))).contains("why"));
    assert!(fault(&with(&|r| r["retire"] = json!([{"item": id("x1"), "outcome": "done", "why": "w".repeat(301)}]))).contains("why"));
    let many: Vec<serde_json::Value> = (0..129).map(|n| json!({"item": format!("missing{n}"), "outcome": "done", "why": "w"})).collect();
    assert!(fault(&with(&|r| r["retire"] = json!(many.clone()))).contains("at most"));
    let empty = json!({"consumes": [], "sort": "Learned", "text": "t", "about": [], "salience_permille": 1});
    assert!(fault(&with(&|r| r["consolidate"] = json!([empty.clone()]))).contains("consumes"));
    let wide = json!({"consumes": (0..17).map(|n| format!("c{n}")).collect::<Vec<_>>(), "sort": "Learned", "text": "t", "about": [], "salience_permille": 1});
    assert!(fault(&with(&|r| r["consolidate"] = json!([wide.clone()]))).contains("consumes"));
    assert!(fault(&with(&|r| r["consolidate"] = json!(vec![empty.clone(); 33]))).contains("at most"));
    assert!(fault(&with(&|r| r["extra"] = json!(1))).contains("not a SleepReply"), "no unknown field");
    assert!(fault(&json!({"night": "n"})).contains("not a SleepReply"));
}

#[test]
fn an_about_is_a_person_a_shown_memory_names() {
    let mut named = reply(&["d1", "d2"]);
    named["remember"] = json!([{"sort": "Rapport", "text": "She likes it plain.", "about": ["@kiko:chat", "@op:chat"], "salience_permille": 300}]);
    assert!(decode(&named.to_string(), &ledger()).is_ok(), "d1 is about kiko and the ask is about the operator");
    named["remember"][0]["about"] = json!(["@stranger:chat"]);
    assert!(fault(&named).contains("remember.about"));
    let mut consolidated = reply(&["d2"]);
    consolidated["consolidate"] = json!([{"consumes": [id("d1")], "sort": "Learned", "text": "t", "about": ["@stranger:chat"], "salience_permille": 100}]);
    assert!(fault(&consolidated).contains("consolidate.about"));
    named["remember"][0]["about"] = json!(vec!["@kiko:chat"; 9]);
    assert!(fault(&named).contains("remember.about"), "more than eight");
}

#[test]
fn faults_are_told_together_so_one_retell_can_mend_them() {
    let both = fault(&json!({"night": "", "keep": [id("d1"), "ghost"], "consolidate": [], "retire": [], "remember": []}));
    assert!(both.contains("night") && both.contains("ghost") && both.contains(&id("d2")), "{both}");
}

fn op(consumes: usize) -> Vec<PipelineDocument> {
    (0..=consumes).map(|n| memory_from(&format!("u{n}"), MemorySort::Idea, "x", &[], wake(), 100)).collect()
}

#[test]
fn operations_pack_into_batches_of_at_most_the_batch_max_and_never_split_one() {
    let units: Vec<Vec<PipelineDocument>> = vec![op(16), op(16), op(16), op(16), op(1)];
    let sizes: Vec<usize> = units.iter().map(Vec::len).collect();
    assert_eq!(sizes, vec![17, 17, 17, 17, 2], "seventy documents");
    let batches = pack(units);
    assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(), vec![51, 19], "three whole consolidations fit, the fourth starts the next batch");
    assert!(batches.iter().all(|batch| batch.len() <= huginn_mind::BATCH_MAX));
    assert_eq!(pack(vec![op(62), op(0)]).iter().map(Vec::len).collect::<Vec<_>>(), vec![63 + 1], "a batch fills to the max exactly");
    assert_eq!(pack(vec![op(62), op(1)]).iter().map(Vec::len).collect::<Vec<_>>(), vec![63, 2]);
    assert!(pack(Vec::new()).is_empty());
}

fn consolidated(context: &Context, consumes: &[&str]) -> PipelineMemory {
    let reply = SleepReply {
        night: "n".into(),
        keep: vec![],
        consolidate: vec![Consolidate { consumes: consumes.iter().map(|label| id(label)).collect(), sort: MemorySort::Learned, text: "One.".into(), about: vec![], salience_permille: 400 }],
        retire: vec![],
        remember: vec![],
    };
    let units = operations(&TestMind::seeded(), context, &reply).unwrap();
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].len(), 1 + consumes.len(), "the memory and a resolution of each consumed memory");
    let PipelineDocument::Memory(memory) = units[0][0].clone() else { panic!("the memory comes first") };
    for (resolution, consumed) in units[0][1..].iter().zip(consumes) {
        let PipelineDocument::Resolution(resolution) = resolution else { panic!("a resolution") };
        assert_eq!(resolution.subject.id.0, id(consumed));
        assert!(matches!(&resolution.outcome, ResolutionOutcome::Superseded { by } if by.len() == 1 && by[0].id.0 == id(&memory.label.0)), "{:?}", resolution.outcome);
    }
    memory
}

#[test]
fn a_consolidation_carries_at_most_eight_sources_newest_first_and_eight_voices() {
    let mut context = ledger();
    let mut old = held("s0", MemorySort::Learned, &[], dm("!a:chat", &[]), 10);
    old.memory.sources = (0..6).map(|n| format!("$old{n}").as_str().into()).collect();
    old.memory.origin = dm("!a:chat", &["@v0:chat", "@v1:chat", "@v2:chat", "@v3:chat", "@v4:chat"]);
    let mut new = held("s1", MemorySort::Learned, &[], dm("!a:chat", &[]), 11);
    new.memory.sources = ["$new0", "$old5", "$new1", "$new2", "$new3"].map(Into::into).to_vec();
    new.memory.origin = dm("!a:chat", &["@v3:chat", "@v5:chat", "@v6:chat", "@v7:chat", "@v8:chat"]);
    for held in [&old, &new] {
        context.all.insert(held.id.clone(), held.clone());
    }
    let memory = consolidated(&context, &["s0", "s1"]);
    let sources: Vec<&str> = memory.sources.iter().map(|source| source.0.as_str()).collect();
    assert_eq!(sources, vec!["$new3", "$new2", "$new1", "$old5", "$new0", "$old4", "$old3", "$old2"], "newest memory's last source first, once each, eight");
    let MemoryOrigin::Direct { room, with } = &memory.origin else { panic!("a DM stays its own") };
    assert_eq!(room.0, "!a:chat");
    assert_eq!(with.iter().map(|who| who.0.as_str()).collect::<Vec<_>>(), vec!["@v0:chat", "@v1:chat", "@v2:chat", "@v3:chat", "@v4:chat", "@v5:chat", "@v6:chat", "@v7:chat"], "the union, in the order they spoke, eight");
    assert_eq!(memory.consolidates.iter().map(|consumed| consumed.id.0.clone()).collect::<Vec<_>>(), vec![id("s0"), id("s1")]);
    assert_eq!(memory.label.0, "mind-night-c0");
    let public = consolidated(&context, &["d1", "d2"]);
    assert_eq!(public.origin, MemoryOrigin::Sleep { run: "mind-night".into() });
    assert_eq!((public.sort, public.text.0.as_str(), public.salience_permille), (MemorySort::Learned, "One.", 400));
}

#[test]
fn retirements_and_remembered_things_become_resolutions_and_sleep_memories() {
    let context = ledger();
    let reply = SleepReply {
        night: "n".into(),
        keep: vec![],
        consolidate: vec![],
        retire: [RetireOutcome::Done, RetireOutcome::Declined, RetireOutcome::Faded, RetireOutcome::Wrong]
            .into_iter()
            .zip(["d1", "d2", "ask", "x1"])
            .map(|(outcome, label)| Retire { item: id(label), outcome, why: format!("why {label}") })
            .collect(),
        remember: vec![Remember { sort: MemorySort::Idea, text: "Later.".into(), about: vec![], salience_permille: 200 }],
    };
    let units = operations(&TestMind::seeded(), &context, &reply).unwrap();
    assert_eq!(units.len(), 5);
    let kinds: Vec<(&str, bool)> = units[..4]
        .iter()
        .map(|unit| {
            let PipelineDocument::Resolution(resolution) = &unit[0] else { panic!("a resolution") };
            assert_eq!(resolution.rationale.0, format!("why {}", resolution.subject.id.0.rsplit(':').next().unwrap()), "her reason");
            match &resolution.outcome {
                ResolutionOutcome::Recorded { reason } => (reason.0.as_str().rsplit(' ').next().unwrap(), true),
                ResolutionOutcome::Withdrawn { reason } => (reason.0.as_str().rsplit(' ').next().unwrap(), false),
                other => panic!("{other:?}"),
            }
        })
        .collect();
    assert_eq!(kinds, vec![("d1", true), ("d2", false), ("ask", true), ("x1", false)], "done and faded are Recorded, declined and wrong Withdrawn");
    let PipelineDocument::Memory(remembered) = &units[4][0] else { panic!("a memory") };
    assert_eq!((remembered.label.0.as_str(), &remembered.origin), ("mind-night-r0", &MemoryOrigin::Sleep { run: "mind-night".into() }));
}

/// Her mind with a Sleep run opened at `T0 + 1000` (and `earlier` Sleep runs, slept, a hundred hours before it),
/// and `memories` admitted at the given offsets: everything admitted in the order of its time.
fn night_over(earlier: usize, memories: &[(String, MemorySort, MemoryOrigin, i64)]) -> (TestMind, PipelineRef) {
    let mind = TestMind::seeded();
    let mut events: Vec<(i64, Vec<Vec<PipelineDocument>>)> = Vec::new();
    for n in 0..earlier {
        let label = format!("before{n}");
        let closed = crate::testkit::close(run_ref(&label), ResolutionOutcome::Slept { line: "ok".into(), day_through: None });
        events.push((T0 - 100 * 3_600_000 + n as i64, vec![vec![run(&label, RunTurn::Sleep, RunOperator::Mind, &[])], vec![closed]]));
    }
    for (label, sort, origin, offset) in memories {
        events.push((T0 + offset, vec![vec![memory_from(label, *sort, "Text.", &[], origin.clone(), 500)]]));
    }
    events.push((T0 + 1000, vec![vec![run("tonight", RunTurn::Sleep, RunOperator::Mind, &[])]]));
    events.sort_by_key(|(when, _)| *when);
    for (when, commits) in events {
        mind.admit_at(at(when));
        for commit in commits {
            mind.committed(commit);
        }
    }
    (mind, run_ref("tonight"))
}

/// A night's own ending, admitted: slept through what it was shown, or failed.
fn close_night(mind: &TestMind, run: &PipelineRef, finish: &Finish) {
    let (outcome, rationale) = closing(finish);
    mind.committed(vec![PipelineDocument::Resolution(PipelineResolution {
        subject: run.clone(),
        sequence: 1,
        outcome,
        rationale: rationale.as_str().into(),
        resolved_on: eureka_pipeline::Date("2026-10-09".into()),
    })]);
}

fn slept_over(context: &Context) -> Finish {
    Finish::Slept { reply: SleepReply { night: "n".into(), keep: vec![], consolidate: vec![], retire: vec![], remember: vec![] }, through: context.day.last().map(|held| reference(&held.id)) }
}

/// A closing someone other than the organ wrote: a Recorded one, whatever its words say.
fn closed_by_hand(mind: &TestMind, run: &PipelineRef, reason: &str, rationale: &str) {
    mind.committed(vec![PipelineDocument::Resolution(PipelineResolution {
        subject: run.clone(),
        sequence: 1,
        outcome: ResolutionOutcome::Recorded { reason: reason.into() },
        rationale: rationale.into(),
        resolved_on: eureka_pipeline::Date("2026-10-09".into()),
    })]);
}

fn failed() -> Finish {
    Finish::Failed { refusals: 3, why: "no reply the ledger took".into() }
}

/// Opens another night of hers at `when` and reads its day.
fn next_night(mind: &TestMind, label: &str, when: i64) -> (PipelineRef, Context) {
    mind.admit_at(at(T0 + when));
    mind.committed(vec![run(label, RunTurn::Sleep, RunOperator::Mind, &[])]);
    let next = run_ref(label);
    let context = context(mind, &next, at(T0 + when + 5000)).unwrap();
    (next, context)
}

fn day_ids(context: &Context) -> Vec<String> {
    context.day.iter().map(|held| held.id.clone()).collect()
}

fn remember_at(mind: &TestMind, label: &str, origin: MemoryOrigin, when: i64) {
    mind.admit_at(at(T0 + when));
    mind.committed(vec![memory_from(label, MemorySort::Learned, "Text.", &[], origin, 500)]);
}

#[test]
fn the_day_is_capped_oldest_first_the_open_items_too_and_the_rest_wait() {
    // No earlier night: her day is the 24 hours before the run opened; the asks are older than that.
    let asks = (0..60).map(|n| (format!("ask{n:03}"), MemorySort::Asked, wake(), -(25 * 3_600_000) + n as i64));
    let days = (0..100).map(|n| (format!("day{n:03}"), MemorySort::Learned, wake(), 10 + n as i64));
    let (mind2, run2) = night_over(0, &asks.chain(days).collect::<Vec<_>>());
    let night = context(&mind2, &run2, at(T0 + 5000)).unwrap();
    assert_eq!(night.day.len(), DAY_MAX);
    assert_eq!((night.day[0].id.as_str(), night.day[95].id.as_str()), (id("day000").as_str(), id("day095").as_str()), "oldest first; day096 and on wait");
    assert_eq!(night.open.len(), OPEN_MAX);
    assert_eq!(night.open[0].id, id("ask000"));
    assert_eq!(night.counts.get("Learned"), Some(&100));
    assert_eq!(night.counts.get("Asked"), Some(&60));
}

#[test]
fn the_tide_moves_thirty_two_memories_a_night_over_the_older_backlog_and_is_whole_when_small() {
    let old = |n: usize| (format!("old{n:02}"), MemorySort::Learned, wake(), -(150 * 3_600_000) + n as i64);
    let backlog: Vec<_> = (0..40).map(old).collect();
    let (mind, run) = night_over(2, &backlog);
    let night = context(&mind, &run, at(T0 + 5000)).unwrap();
    let ids: Vec<&str> = night.tide.iter().map(|held| held.id.as_str()).collect();
    assert_eq!(ids.len(), 32);
    let picked: Vec<String> = [0, 7, 8, 31].map(|at| ids[at].to_string()).to_vec();
    assert_eq!(picked, vec![id("old24"), id("old31"), id("old32"), id("old15")], "two earlier nights: offset 64 mod 40 is 24, and it wraps");
    let (mind, run) = night_over(1, &backlog[..10]);
    assert_eq!(context(&mind, &run, at(T0 + 5000)).unwrap().tide.len(), 10, "a small backlog is shown whole, once each");
    let (mind, run) = night_over(0, &[]);
    assert!(context(&mind, &run, at(T0 + 5000)).unwrap().tide.is_empty());
}

#[test]
fn the_day_begins_when_her_previous_night_opened_and_sleep_made_memories_are_not_in_it() {
    let hour = 3_600_000;
    let memories = vec![
        ("before".to_string(), MemorySort::Learned, wake(), -(120 * hour)),
        ("formed".to_string(), MemorySort::Idea, MemoryOrigin::Sleep { run: "x".into() }, -hour),
        ("after".to_string(), MemorySort::Learned, wake(), -hour),
    ];
    // The earlier night opened 100 hours before tonight's anchor: `before` is older than it, `after` newer.
    let (mind, run) = night_over(1, &memories);
    let night = context(&mind, &run, at(T0 + 5000)).unwrap();
    assert_eq!(night.day.iter().map(|held| held.id.clone()).collect::<Vec<_>>(), vec![id("after")]);
    assert!(night.tide.iter().all(|held| held.id != id("formed")), "no Sleep-origin memory comes round");
}

#[test]
fn the_night_schema_states_the_reply_the_decoder_takes() {
    let schema: serde_json::Value = serde_json::from_str(&sleep_schema()).unwrap();
    assert_eq!(schema["required"], json!(["night", "keep", "consolidate", "retire", "remember"]));
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(schema["properties"]["consolidate"]["maxItems"], json!(32));
    assert_eq!(schema["properties"]["consolidate"]["items"]["properties"]["consumes"]["maxItems"], json!(16));
    assert_eq!(schema["properties"]["retire"]["maxItems"], json!(128));
    assert_eq!(schema["properties"]["remember"]["maxItems"], json!(16));
    assert_eq!(schema["properties"]["retire"]["items"]["properties"]["outcome"]["enum"], json!(["done", "declined", "faded", "wrong"]));
}

#[test]
fn what_the_cap_left_waits_for_the_next_night_and_is_not_dropped_to_the_backlog() {
    let days: Vec<_> = (0..100).map(|n| (format!("day{n:03}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, tonight) = night_over(0, &days);
    let shown = context(&mind, &tonight, at(T0 + 5000)).unwrap();
    assert_eq!(shown.day.len(), DAY_MAX);
    assert!(shown.tide.is_empty() && shown.open.is_empty(), "what waits is neither tide nor open");
    mind.admit_at(at(T0 + 2000));
    close_night(&mind, &tonight, &slept_over(&shown));
    let (_, next) = next_night(&mind, "tomorrow", 3000);
    assert_eq!(day_ids(&next), (96..100).map(|n| id(&format!("day{n:03}"))).collect::<Vec<_>>(), "the four past the cap are tomorrow's day, and only they");
    assert_eq!(next.tide.len(), 32, "the ninety-six she was shown are backlog now");
    assert!(next.tide.iter().all(|held| !day_ids(&next).contains(&held.id)), "a day memory is not also in the tide");
}

#[test]
fn a_night_that_failed_or_never_ran_covers_nothing_so_its_day_comes_back() {
    let days: Vec<_> = (0..3).map(|n| (format!("f{n}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, first) = night_over(0, &days);
    let shown = context(&mind, &first, at(T0 + 5000)).unwrap();
    assert_eq!(day_ids(&shown).len(), 3);
    // Her first night was withdrawn unrun: the day is still the 24 hours before that first night opened.
    mind.admit_at(at(T0 + 2000));
    mind.committed(vec![crate::testkit::close(first.clone(), crate::testkit::withdrawn())]);
    let (second, again) = next_night(&mind, "second", 3000);
    assert_eq!(day_ids(&again), vec![id("f0"), id("f1"), id("f2")]);
    // The second night failed, and f3 arrived while it was open: the third owes all four.
    remember_at(&mind, "f3", wake(), 3500);
    mind.admit_at(at(T0 + 3600));
    close_night(&mind, &second, &failed());
    let (third, owed) = next_night(&mind, "third", 4000);
    assert_eq!(day_ids(&owed), vec![id("f0"), id("f1"), id("f2"), id("f3")]);
    // The third slept through what it was shown; the next day begins after it.
    remember_at(&mind, "g0", wake(), 4500);
    mind.admit_at(at(T0 + 4600));
    close_night(&mind, &third, &slept_over(&owed));
    remember_at(&mind, "g1", wake(), 5000);
    let (fourth, owed_again) = next_night(&mind, "fourth", 6000);
    assert_eq!(day_ids(&owed_again), vec![id("g0"), id("g1")], "g0 arrived while the third night was open and is still owed; f0 to f3 are covered");
    // A failure after a night that slept leaves that night's cover standing: only the nights' own closings are read.
    remember_at(&mind, "g2", wake(), 6500);
    mind.admit_at(at(T0 + 6600));
    close_night(&mind, &fourth, &failed());
    let (_, fifth) = next_night(&mind, "fifth", 7000);
    assert_eq!(day_ids(&fifth), vec![id("g0"), id("g1"), id("g2")]);
}

#[test]
fn a_hand_written_closing_moves_no_cut_line() {
    // Closed Recorded with the words and the numbers the old reader trusted: the day is owed all the same.
    let days: Vec<_> = (0..3).map(|n| (format!("g{n}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, first) = night_over(0, &days);
    mind.admit_at(at(T0 + 2000));
    closed_by_hand(&mind, &first, "slept: closed by hand", "kept 0; day through ordinal 999999999");
    let (_, next) = next_night(&mind, "second", 3000);
    assert_eq!(day_ids(&next), vec![id("g0"), id("g1"), id("g2")], "nothing was shown, so nothing is dropped");

    // A hundred-memory day closed by hand covers none of it; a Slept close over what was then shown covers it.
    let days: Vec<_> = (0..100).map(|n| (format!("day{n:03}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, first) = night_over(0, &days);
    mind.admit_at(at(T0 + 2000));
    closed_by_hand(&mind, &first, "slept: closed by hand", "kept 96 (by hand)");
    let (second, owed) = next_night(&mind, "second", 3000);
    assert_eq!(day_ids(&owed), (0..96).map(|n| id(&format!("day{n:03}"))).collect::<Vec<_>>(), "the overflow is not dropped by a hand");
    mind.admit_at(at(T0 + 3600));
    close_night(&mind, &second, &slept_over(&owed));
    let (_, third) = next_night(&mind, "third", 4000);
    assert_eq!(day_ids(&third), (96..100).map(|n| id(&format!("day{n:03}"))).collect::<Vec<_>>());
}

#[test]
fn a_slept_nights_closing_names_the_last_memory_it_was_shown_even_when_that_night_consumed_it() {
    let days: Vec<_> = (0..100).map(|n| (format!("day{n:03}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, tonight) = night_over(0, &days);
    let shown = context(&mind, &tonight, at(T0 + 5000)).unwrap();
    let (outcome, rationale) = closing(&slept_over(&shown));
    assert_eq!(outcome, ResolutionOutcome::Slept { line: "n".into(), day_through: Some(crate::testkit::reference(eureka_pipeline::PipelineKind::Memory, &id("day095"))) });
    assert_eq!(rationale, "kept 0, consolidated 0 from 0, retired 0, formed 0", "the rationale is the counts and says nothing of the cut line");
    // The night folds the last memory it was shown into a consolidation, so day095 leaves force.
    mind.admit_at(at(T0 + 2000));
    let PipelineDocument::Memory(mut merged) = memory_from("merged", MemorySort::Learned, "Merged.", &[], wake(), 500) else { unreachable!() };
    let consumed = crate::testkit::reference(eureka_pipeline::PipelineKind::Memory, &id("day095"));
    merged.consolidates = vec![consumed.clone()];
    let by = crate::testkit::reference(eureka_pipeline::PipelineKind::Memory, &id("merged"));
    mind.committed(vec![PipelineDocument::Memory(merged), crate::testkit::close(consumed, ResolutionOutcome::Superseded { by: vec![by] })]);
    mind.admit_at(at(T0 + 2500));
    close_night(&mind, &tonight, &slept_over(&shown));
    let (_, next) = next_night(&mind, "tomorrow", 3000);
    assert_eq!(day_ids(&next), (96..100).map(|n| id(&format!("day{n:03}"))).collect::<Vec<_>>(), "the cut line is read from the memory that is no longer in force");
}

#[test]
fn the_newest_slept_nights_closing_decides_not_an_older_ones() {
    let days: Vec<_> = (0..2).map(|n| (format!("f{n}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, first) = night_over(0, &days);
    let shown = context(&mind, &first, at(T0 + 5000)).unwrap();
    mind.admit_at(at(T0 + 2000));
    close_night(&mind, &first, &slept_over(&shown));
    remember_at(&mind, "f2", wake(), 2500);
    remember_at(&mind, "f3", wake(), 2600);
    let (second, owed) = next_night(&mind, "second", 3000);
    assert_eq!(day_ids(&owed), vec![id("f2"), id("f3")]);
    mind.admit_at(at(T0 + 3600));
    close_night(&mind, &second, &slept_over(&owed));
    remember_at(&mind, "g0", wake(), 3800);
    let (_, third) = next_night(&mind, "third", 4000);
    assert_eq!(day_ids(&third), vec![id("g0")], "both nights slept; the second's cover is the one that counts");
}

#[test]
fn a_slept_closing_that_was_withdrawn_covers_nothing() {
    let days: Vec<_> = (0..3).map(|n| (format!("f{n}"), MemorySort::Learned, wake(), 10 + n as i64)).collect();
    let (mind, first) = night_over(0, &days);
    let shown = context(&mind, &first, at(T0 + 5000)).unwrap();
    mind.admit_at(at(T0 + 2000));
    close_night(&mind, &first, &slept_over(&shown));
    // The closing is withdrawn (the run is live again) and the run ends in failure: no Slept closing stands.
    mind.admit_at(at(T0 + 2100));
    let closing_ref = crate::testkit::reference(eureka_pipeline::PipelineKind::Resolution, &format!("{INSTANCE}:resolution:run.tonight.n1"));
    mind.committed(vec![crate::testkit::close(closing_ref, crate::testkit::withdrawn())]);
    mind.admit_at(at(T0 + 2200));
    mind.committed(vec![PipelineDocument::Resolution(PipelineResolution {
        subject: first.clone(),
        sequence: 2,
        outcome: ResolutionOutcome::Recorded { reason: "failed after 3 refusals: no reply the ledger took".into() },
        rationale: "The night ended without a reply the ledger took.".into(),
        resolved_on: eureka_pipeline::Date("2026-10-09".into()),
    })]);
    let (_, next) = next_night(&mind, "second", 3000);
    assert_eq!(day_ids(&next), vec![id("f0"), id("f1"), id("f2")], "a withdrawn Slept closing leaves its day owed");
}

#[test]
fn the_first_nights_day_is_the_twenty_four_hours_before_it_opened_to_the_second() {
    let hour = 3_600_000;
    let memories = vec![
        ("on_the_line".to_string(), MemorySort::Learned, wake(), 1000 - 24 * hour),
        ("just_after".to_string(), MemorySort::Learned, wake(), 2000 - 24 * hour),
    ];
    let (mind, run) = night_over(0, &memories);
    let night = context(&mind, &run, at(T0 + 5000)).unwrap();
    assert_eq!(day_ids(&night), vec![id("just_after")]);
    assert!(night.tide.iter().any(|held| held.id == id("on_the_line")), "the older one is backlog");
}

#[test]
fn a_consolidation_a_night_made_is_not_the_next_days_memory_whatever_its_origin() {
    let (mind, tonight) = night_over(0, &[("d1".to_string(), MemorySort::Learned, dm("!a:chat", &["@a:chat"]), 10)]);
    let shown = context(&mind, &tonight, at(T0 + 5000)).unwrap();
    // The night consolidates the DM memory: the consolidation keeps the room, so its origin is Direct, admitted after the night opened.
    mind.admit_at(at(T0 + 2000));
    let PipelineDocument::Memory(mut merged) = memory_from("dm-merged", MemorySort::Learned, "Merged.", &[], dm("!a:chat", &["@a:chat"]), 500) else { unreachable!() };
    merged.consolidates = vec![crate::testkit::reference(eureka_pipeline::PipelineKind::Memory, &id("d1"))];
    let by = crate::testkit::reference(eureka_pipeline::PipelineKind::Memory, &id("dm-merged"));
    mind.committed(vec![PipelineDocument::Memory(merged), crate::testkit::close(crate::testkit::reference(eureka_pipeline::PipelineKind::Memory, &id("d1")), ResolutionOutcome::Superseded { by: vec![by] })]);
    mind.admit_at(at(T0 + 2500));
    close_night(&mind, &tonight, &slept_over(&shown));
    remember_at(&mind, "fresh", wake(), 2800);
    let (_, next) = next_night(&mind, "tomorrow", 3000);
    assert_eq!(day_ids(&next), vec![id("fresh")], "the consolidation is not shown as her day");
    assert!(next.tide.iter().all(|held| held.id != id("dm-merged")), "nor does it come round as backlog");
}
