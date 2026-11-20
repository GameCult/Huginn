use std::path::Path;

use serde_json::Value;

use super::*;
use crate::projection::harness::tests::{Scripted, control};
use crate::projection::{Answer, Failed, Model, Spent};

const TOBIAH: &str = "@eureka:gamecult.org";
const LIBBY: &str = "@libby:gamecult.org";

/// A cast of two in a temp dir: the control character as Eureka, and a copy of it as Libby ("Libby Test").
fn cast(dir: &Path) {
    std::fs::write(dir.join("tobiah.json"), control().to_string()).unwrap();
    let mut libby = control();
    libby["personaId"] = "libby".into();
    libby["provenance"]["sourceDocumentId"] = "libby".into();
    libby["publicName"] = "Libby Test".into();
    std::fs::write(dir.join("libby.json"), libby.to_string()).unwrap();
}

/// A plan over the two, the operator opening; `wake` is the TOML value of the policy; no beat unless `extra` sets one.
fn plan(dir: &Path, wake: &str, rounds: usize, extra: &str) -> PathBuf {
    cast(dir);
    let path = dir.join("plan.toml");
    let beat = if extra.contains("beat_s") { "" } else { "beat_s = 0\n" };
    std::fs::write(
        &path,
        format!(
            r##"rounds = {rounds}
seed = 11
wake = {wake}
start = "2026-11-20T15:00:00Z"
rooms = ["#minds:gamecult.org"]
operators = ["@meta:gamecult.org"]
{beat}{extra}
[[cast]]
id = "{TOBIAH}"
name = "Tobiah"
state = "tobiah.json"
[[cast]]
id = "{LIBBY}"
name = "Libby"
state = "libby.json"
[[message]]
from = "@meta:gamecult.org"
name = "Meta"
room = "#minds:gamecult.org"
says = "good afternoon, everyone"
"##
        ),
    )
    .unwrap();
    path
}

fn is_tobiah(prompt: &str) -> bool {
    prompt.contains("Who you are:\nTobiah")
}

/// Both speak every turn, neither naming the other.
fn chatty() -> Scripted {
    Scripted::new(|prompt| if is_tobiah(prompt) { "\"what a day\" he says".into() } else { "\"mm, the deploy went fine\" she says".into() })
}

/// Both speak every turn, each naming the other.
fn namers() -> Scripted {
    Scripted::new(|prompt| if is_tobiah(prompt) { "\"Libby, tea?\"".into() } else { "\"Tobiah, yes please.\"".into() })
}

fn silent() -> Scripted {
    Scripted::new(|_| "Nobody needs anything from her.".into())
}

fn run_plan(path: &Path, model: &dyn Model) -> (Vec<Member>, Value) {
    let plan: SimFile = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let base = path.parent().unwrap();
    let mut members = cast_of(&plan, base).unwrap();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(model), tuning: base.join("persona-tuning.toml") };
    let run = run(&harness, &plan, &mut members, 0).unwrap();
    (members, run)
}

fn turns(run: &Value) -> Vec<(u64, String, String)> {
    run["turns"].as_array().unwrap().iter().map(|turn| (turn["round"].as_u64().unwrap(), turn["persona"].as_str().unwrap().to_owned(), turn["woke"].as_str().unwrap().to_owned())).collect()
}

#[test]
fn the_wake_policies_are_the_heartbeats_one_function() {
    let libby = Addressee::new(LIBBY, ["libby", "Libby"]);
    let line = |from: &str, says: &str| Line { from: from.into(), name: String::new(), room: "#minds:x".into(), at_ms: 0, says: says.into(), mentions: vec![], dm: false, follow_up: false, reply_to: None, new: false };
    let ask = |policy: WakePolicy, from: &str, says: &str, speaker: Speaker, run: usize| wakes(policy, &libby, &line(from, says).delivery(0, LIBBY, &[]), speaker, run);
    let (named, unnamed) = ("Libby, tea?", "what a day");
    use WakeCause::*;
    // Ruled: anyone addressing her, any human line.
    assert_eq!(ask(WakePolicy::Ruled, TOBIAH, named, Speaker::Mind, 0), Some(AddressedByMind));
    assert_eq!(ask(WakePolicy::Ruled, TOBIAH, unnamed, Speaker::Mind, 0), None, "another Mind's unaddressed line wakes no one");
    assert_eq!(ask(WakePolicy::Ruled, "@deru:x", unnamed, Speaker::Member, 0), Some(Human));
    assert_eq!(ask(WakePolicy::Ruled, "@deru:x", named, Speaker::Member, 0), Some(AddressedByHuman));
    // Live, the heartbeat's today: a member's line does not wake her, the operator's or a Mind's address does.
    assert_eq!(ask(WakePolicy::Live, "@deru:x", named, Speaker::Member, 0), None);
    assert_eq!(ask(WakePolicy::Live, "@meta:x", named, Speaker::Operator, 0), Some(AddressedByHuman));
    assert_eq!(ask(WakePolicy::Live, TOBIAH, named, Speaker::Mind, 0), Some(AddressedByMind));
    assert_eq!(ask(WakePolicy::Live, "@meta:x", unnamed, Speaker::Operator, 0), None);
    // Humans pace: a Mind's line never wakes a Mind.
    assert_eq!(ask(WakePolicy::HumansPace, TOBIAH, named, Speaker::Mind, 0), None);
    assert_eq!(ask(WakePolicy::HumansPace, "@meta:x", unnamed, Speaker::Operator, 0), Some(Human));
    // The cap: a Mind's address wakes her only while the Mind run before the line is under it.
    assert_eq!(ask(WakePolicy::NamesWithCap(2), TOBIAH, named, Speaker::Mind, 1), Some(AddressedByMind));
    assert_eq!(ask(WakePolicy::NamesWithCap(2), TOBIAH, named, Speaker::Mind, 2), None);
    assert_eq!(ask(WakePolicy::NamesWithCap(2), "@meta:x", unnamed, Speaker::Operator, 9), Some(Human), "a human always may");
    // The control, and her own line never.
    assert_eq!(ask(WakePolicy::AllMessages, TOBIAH, unnamed, Speaker::Mind, 0), Some(Mind));
    assert_eq!(ask(WakePolicy::AllMessages, LIBBY, named, Speaker::Her, 0), None);
}

#[test]
fn under_the_ruled_set_unaddressed_mind_lines_wake_no_one() {
    let dir = tempfile::tempdir().unwrap();
    let (_, run) = run_plan(&plan(dir.path(), "\"ruled\"", 6, ""), &chatty());
    let turns = turns(&run);
    assert_eq!(turns.len(), 2, "the operator's opening wakes both in round 1, and then nothing does: {turns:?}");
    assert!(turns.iter().all(|(round, _, why)| *round == 1 && why == "human"), "{turns:?}");
    assert!(run["transcript"].as_array().unwrap().iter().any(|line| line["from"] == TOBIAH && line["says"] == "what a day" && line["woke"] == "human"), "her quoted words join the room as from her id");
}

#[test]
fn the_control_lets_minds_wake_each_other_into_a_reply_chain_counted_per_room() {
    let dir = tempfile::tempdir().unwrap();
    let (_, run) = run_plan(&plan(dir.path(), "\"all_messages\"", 6, ""), &chatty());
    let metrics = &run["metrics"];
    assert!(metrics["wakes"]["mind"].as_u64().unwrap() >= 5, "{metrics}");
    let chain = &metrics["reply_chains"]["#minds:gamecult.org"];
    // The two round-1 answers are to the operator; every later line is a reply to a Mind.
    assert_eq!(chain["mind_to_mind_replies"].as_u64(), Some(metrics["mind_messages"].as_u64().unwrap() - 2), "{metrics}");
    assert!(chain["longest"].as_u64().unwrap() >= 5 && chain["chains_of_3_or_more"] == 1, "{chain}");
    let work = metrics["work_talk_share"].as_f64().unwrap();
    assert!(work > 0.3 && work < 0.7, "Libby's lines name the deploy, Tobiah's do not: {work}");
}

#[test]
fn naming_runs_away_under_the_ruled_set_and_stops_at_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let (_, ruled) = run_plan(&plan(dir.path(), "\"ruled\"", 6, ""), &namers());
    assert!(turns(&ruled).iter().filter(|(_, _, why)| why == "addressed_by_mind").count() >= 5, "{:?}", turns(&ruled));
    assert_eq!(ruled["metrics"]["mind_addressed_wakes"]["name_in_text"], ruled["metrics"]["mind_addressed_wakes"]["total"]);
    let dir = tempfile::tempdir().unwrap();
    let (_, capped) = run_plan(&plan(dir.path(), "{ names_with_cap = 2 }", 6, ""), &namers());
    let capped = turns(&capped);
    assert!(capped.iter().all(|(round, _, _)| *round <= 2), "after two Mind lines in a row no Mind's address wakes anyone: {capped:?}");
    let dir = tempfile::tempdir().unwrap();
    let (_, paced) = run_plan(&plan(dir.path(), "\"humans_pace\"", 6, ""), &namers());
    assert_eq!(turns(&paced).len(), 2, "under humans_pace only the operator's line wakes anyone");
}

#[test]
fn a_human_interjection_wakes_everyone_in_her_room_under_humans_pace() {
    let dir = tempfile::tempdir().unwrap();
    let interjection = "[[message]]\nround = 4\nfrom = \"@deru:gamecult.org\"\nname = \"Deru\"\nroom = \"#minds:gamecult.org\"\nsays = \"kettle news\"\n";
    let path = plan(dir.path(), "\"humans_pace\"", 5, "");
    std::fs::write(&path, format!("{}{interjection}", std::fs::read_to_string(&path).unwrap())).unwrap();
    let (members, run) = run_plan(&path, &namers());
    let rounds = run["metrics"]["wakes_by_round"].as_array().unwrap();
    assert_eq!((rounds[0]["causes"]["human"].as_u64(), rounds[3]["causes"]["human"].as_u64(), rounds[1]["causes"]["addressed_by_mind"].as_u64()), (Some(2), Some(2), Some(0)), "{rounds:?}");
    assert_eq!(run["metrics"]["personas"]["Tobiah"]["addressed"]["Libby"].as_u64(), run["metrics"]["personas"]["Tobiah"]["messages"].as_u64());
    assert_eq!(members[0].state, control(), "the seam: no state moved");
}

#[test]
fn the_beat_wakes_one_persona_by_the_heartbeats_pick() {
    let dir = tempfile::tempdir().unwrap();
    let (_, run) = run_plan(&plan(dir.path(), "\"ruled\"", 4, "beat_s = 120"), &silent());
    let turns = turns(&run);
    for round in 2..=4 {
        let beats: Vec<&(u64, String, String)> = turns.iter().filter(|(at, _, _)| *at == round).collect();
        assert_eq!(beats.len(), 1, "one wake a beat: {turns:?}");
        assert_eq!(beats[0].2, "beat");
    }
    // The pick takes whoever waited longest: the two alternate.
    assert_ne!(turns[2].1, turns[3].1, "{turns:?}");
    assert_eq!(run["metrics"]["silence_rate"].as_f64(), Some(1.0));
}

/// A model whose Persona stage fails: a failure, never a silence.
struct FailingPersona;

impl Model for FailingPersona {
    fn ask(&self, _request_id: &str, model: &str, prompt: &str) -> std::result::Result<Answer, Failed> {
        if prompt.starts_with("<!-- membrane:") {
            Ok(Answer { text: "The room is slow.".into(), usage: None })
        } else {
            Err(Failed { reason: "the provider failed the call: Overloaded".into(), usage: Some(Spent { model: model.into(), input_tokens: 5, output_tokens: 0, cache_read_tokens: 0, cache_write_tokens: 0, reasoning_tokens: None }) })
        }
    }
}

#[test]
fn a_failed_call_is_a_failure_and_not_a_silence() {
    let dir = tempfile::tempdir().unwrap();
    let (_, run) = run_plan(&plan(dir.path(), "\"ruled\"", 2, ""), &FailingPersona);
    let metrics = &run["metrics"];
    assert_eq!((metrics["failed"].as_u64(), metrics["silence_rate"].is_null()), (Some(2), true), "{metrics}");
    assert!(run["turns"].as_array().unwrap().iter().all(|turn| turn["outcome"] == "failed" && turn["failed"].as_str().unwrap().contains("Overloaded")));
}

#[test]
fn a_seed_replays_its_order_and_another_seed_changes_it() {
    let items: Vec<usize> = (0..8).collect();
    assert_eq!(shuffled(items.clone(), 1), shuffled(items.clone(), 1));
    assert_ne!(shuffled(items.clone(), 1), shuffled(items.clone(), 2));
    let mut sorted = shuffled(items.clone(), 2);
    sorted.sort_unstable();
    assert_eq!(sorted, items, "a shuffle, nothing lost");
    let dir = tempfile::tempdir().unwrap();
    let path = plan(dir.path(), "\"all_messages\"", 4, "");
    assert_eq!(turns(&run_plan(&path, &chatty()).1), turns(&run_plan(&path, &chatty()).1));
}

#[test]
fn a_wake_by_an_id_written_as_text_is_counted_once_and_listed() {
    let dir = tempfile::tempdir().unwrap();
    let model = Scripted::new(|prompt| if is_tobiah(prompt) { "\"ask @libby:gamecult.org, and Libby too\"".into() } else { "\"Tobiah?\"".into() });
    let (_, run) = run_plan(&plan(dir.path(), "\"ruled\"", 3, ""), &model);
    let addressed = &run["metrics"]["mind_addressed_wakes"];
    let libby_mind_wakes = turns(&run).iter().filter(|(_, who, why)| who == "Libby" && why == "addressed_by_mind").count();
    assert!(libby_mind_wakes >= 1, "{:?}", turns(&run));
    assert_eq!(addressed["total"].as_u64().unwrap() as usize, turns(&run).iter().filter(|(_, _, why)| why == "addressed_by_mind").count());
    assert_eq!(addressed["id_as_text"].as_u64().unwrap() as usize, libby_mind_wakes, "the strongest evidence of each wake, once per wake: {addressed}");
    assert_eq!(addressed["id_as_text_wakes"].as_array().unwrap().len(), libby_mind_wakes);
}

#[test]
fn the_report_names_its_gaps_and_carries_representative_lines_and_usage() {
    let dir = tempfile::tempdir().unwrap();
    let model = chatty();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    let report = simulate(&harness, &plan(dir.path(), "\"ruled\"", 3, "")).unwrap();
    for part in ["# Swarm simulation", "State does not evolve during a run", "No explicit addressee", "group state in the Projector's packet: on", "### Representative lines", "**Meta (human)**: good afternoon, everyone", "**Tobiah** _(woke: human)_: what a day", "### Wakes by round", "| 1 | 0 | 2 | 0 | 0 | 0 |", "## Total usage", "| claude-sonnet-5-5 | 4 |"] {
        assert!(report.contains(part), "{part}\n{report}");
    }
}

#[test]
fn a_cast_past_eight_or_a_foreign_state_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let model = chatty();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    let path = plan(dir.path(), "\"ruled\"", 2, "");
    let text = std::fs::read_to_string(&path).unwrap().replace("state = \"libby.json\"", "state = \"tobiah.json\"");
    std::fs::write(&path, text).unwrap();
    assert!(format!("{:#}", simulate(&harness, &path).unwrap_err()).contains("refused by a put"), "Libby cannot hold Eureka's state");
    let many: String = (0..9).map(|at| format!("[[cast]]\nid = \"@m{at}:x\"\nname = \"M{at}\"\nstate = \"m.json\"\n")).collect();
    std::fs::write(&path, format!("rounds = 1\nstart = \"2026-11-20T15:00:00Z\"\nrooms = [\"#a:x\"]\n{many}")).unwrap();
    assert!(format!("{:#}", simulate(&harness, &path).unwrap_err()).contains("1 to 8"));
}
