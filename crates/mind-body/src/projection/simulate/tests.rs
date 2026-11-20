use std::path::Path;

use serde_json::Value;

use super::*;
use crate::projection::harness::tests::{Scripted, control};

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

fn plan(dir: &Path, wake: &str, rounds: usize, extra: &str) -> PathBuf {
    cast(dir);
    let path = dir.join("plan.toml");
    std::fs::write(
        &path,
        format!(
            r##"rounds = {rounds}
seed = 11
wake = "{wake}"
start = "2026-11-20T15:00:00Z"
rooms = ["#minds:gamecult.org"]
operators = ["@meta:gamecult.org"]
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
{extra}"##
        ),
    )
    .unwrap();
    path
}

/// Both speak every turn, neither naming the other: a Mind line wakes no one under the ruled set.
fn chatty() -> Scripted {
    Scripted::new(|prompt| if prompt.contains("Who you are:\nTobiah") { "\"what a day\" he says".into() } else { "\"mm, the deploy went fine\" she says".into() })
}

fn run_plan(path: &Path, model: &Scripted) -> (SimFile, Vec<Member>, Value) {
    let plan: SimFile = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let base = path.parent().unwrap();
    let mut members: Vec<Member> = plan
        .cast
        .iter()
        .map(|entry| {
            let state = state_file(&base.join(&entry.state), localpart(&entry.id)).unwrap();
            let public = state["publicName"].as_str().unwrap().to_owned();
            Member { addressee: Addressee::new(&entry.id, [localpart(&entry.id), entry.name.as_str(), public.as_str()]), entry: entry.clone(), state, rooms: plan.rooms.clone() }
        })
        .collect();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(model), tuning: base.join("persona-tuning.toml") };
    let run = run(&harness, &plan, &mut members, 0).unwrap();
    (plan, members, run)
}

fn turns(run: &Value) -> Vec<(u64, String, String)> {
    run["turns"].as_array().unwrap().iter().map(|turn| (turn["round"].as_u64().unwrap(), turn["persona"].as_str().unwrap().to_owned(), turn["woke"].as_str().unwrap().to_owned())).collect()
}

#[test]
fn under_the_ruled_wake_set_a_minds_unaddressed_line_wakes_no_one() {
    let dir = tempfile::tempdir().unwrap();
    let (_, _, run) = run_plan(&plan(dir.path(), "humans_in_rooms", 6, ""), &chatty());
    let turns = turns(&run);
    assert_eq!(turns.len(), 2, "the operator's opening wakes both in round 1, and then nothing does: {turns:?}");
    assert!(turns.iter().all(|(round, _, why)| *round == 1 && why == "a human in her rooms"), "{turns:?}");
    let transcript = run["transcript"].as_array().unwrap();
    assert_eq!(transcript.len(), 3);
    assert!(transcript.iter().any(|line| line["from"] == TOBIAH && line["says"] == "what a day" && line["mind"] == true), "her quoted words join the room as from her id: {transcript:?}");
}

#[test]
fn the_old_loop_control_lets_minds_wake_each_other_into_a_chain() {
    let dir = tempfile::tempdir().unwrap();
    let (_, _, run) = run_plan(&plan(dir.path(), "all_messages", 6, ""), &chatty());
    let turns = turns(&run);
    assert!(turns.len() >= 6, "every round someone answers a Mind: {turns:?}");
    assert!(turns.iter().any(|(_, _, why)| why == "a message in her rooms"));
    let metrics = &run["metrics"];
    assert!(metrics["mind_chain_longest"].as_u64().unwrap() >= 5, "{metrics}");
    assert!(metrics["mind_chains_of_3_or_more"].as_u64().unwrap() >= 1);
    assert_eq!(metrics["silence_rate"].as_f64(), Some(0.0));
    let work = metrics["work_talk_share"].as_f64().unwrap();
    assert!(work > 0.3 && work < 0.7, "Libby's lines name the deploy, Tobiah's do not: {work}");
    assert!(metrics["convergence_lines"]["all"].as_f64().unwrap() < 0.5, "two different lines are not one voice: {metrics}");
}

#[test]
fn a_scripted_interjection_wakes_everyone_in_her_room_and_addressing_is_counted() {
    let dir = tempfile::tempdir().unwrap();
    let interjection = "[[message]]\nround = 4\nfrom = \"@deru:gamecult.org\"\nname = \"Deru\"\nroom = \"#minds:gamecult.org\"\nsays = \"kettle news\"\n";
    let model = Scripted::new(|prompt| if prompt.contains("Who you are:\nTobiah") { "\"Libby, tea?\"".into() } else { "She does not answer.".into() });
    let (_, members, run) = run_plan(&plan(dir.path(), "humans_in_rooms", 5, interjection), &model);
    let turns = turns(&run);
    let fourth: Vec<&String> = turns.iter().filter(|(round, _, _)| *round == 4).map(|(_, who, _)| who).collect();
    assert_eq!(fourth.len(), 2, "Deru's line at round 4 wakes both: {turns:?}");
    assert_eq!(run["metrics"]["personas"]["Tobiah"]["addressed"]["Libby"].as_u64().unwrap(), run["metrics"]["personas"]["Tobiah"]["messages"].as_u64().unwrap());
    let rounds = run["metrics"]["wakes_by_round"].as_array().unwrap();
    assert_eq!((rounds[0]["human"].as_u64(), rounds[3]["human"].as_u64()), (Some(2), Some(2)), "the opening and Deru wake both: {rounds:?}");
    let by_minds: u64 = rounds.iter().map(|row| row["mind_addressed"].as_u64().unwrap()).sum();
    assert_eq!(by_minds as usize, run["metrics"]["mind_addressed"].as_array().unwrap().len(), "one line addressed each Mind-addressed wake");
    assert_eq!(run["metrics"]["personas"]["Libby"]["silence_rate"].as_f64(), Some(1.0));
    // The seam: no state moved.
    assert_eq!(members[0].state, control());
}

#[test]
fn the_same_seed_replays_the_same_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = plan(dir.path(), "all_messages", 4, "");
    let (_, _, first) = run_plan(&path, &chatty());
    let (_, _, second) = run_plan(&path, &chatty());
    assert_eq!(turns(&first), turns(&second));
}

#[test]
fn the_report_names_the_state_gap_and_carries_the_transcript_and_usage() {
    let dir = tempfile::tempdir().unwrap();
    let model = chatty();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    let report = simulate(&harness, &plan(dir.path(), "humans_in_rooms", 3, "")).unwrap();
    for part in ["# Swarm simulation", "State does not evolve during a run", "## Repeat 1 (seed 11)", "### Transcript", "**Meta (human)**: good afternoon, everyone", "**Tobiah**: what a day", "<details><summary>round 1, ", "### Wakes by round", "| 1 | 2 | 0 | 0 | 0 |", "## Total usage", "| claude-sonnet-5-5 | 4 |"] {
        assert!(report.contains(part), "{part}\n{report}");
    }
}

#[test]
fn a_cast_past_eight_or_a_foreign_state_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let model = chatty();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    let path = plan(dir.path(), "humans_in_rooms", 2, "");
    let text = std::fs::read_to_string(&path).unwrap().replace("state = \"libby.json\"", "state = \"tobiah.json\"");
    std::fs::write(&path, text).unwrap();
    assert!(format!("{:#}", simulate(&harness, &path).unwrap_err()).contains("refused by a put"), "Libby cannot hold Eureka's state");
    let many: String = (0..9).map(|at| format!("[[cast]]\nid = \"@m{at}:x\"\nname = \"M{at}\"\nstate = \"m.json\"\n")).collect();
    std::fs::write(&path, format!("rounds = 1\nstart = \"2026-11-20T15:00:00Z\"\nrooms = [\"#a:x\"]\n{many}")).unwrap();
    assert!(format!("{:#}", simulate(&harness, &path).unwrap_err()).contains("1 to 8"));
}

#[test]
fn wake_reason_is_the_ruled_set_and_the_control_widens_it_to_minds() {
    let libby = Member { addressee: Addressee::new(LIBBY, ["libby", "Libby"]), entry: CastEntry { id: LIBBY.into(), name: "Libby".into(), state: PathBuf::new(), rooms: vec![], names: vec![] }, state: Value::Null, rooms: vec!["#minds:x".into()] };
    let cast = vec![TOBIAH.to_owned(), LIBBY.to_owned()];
    let line = |from: &str, room: &str, says: &str| Line { from: from.into(), name: String::new(), room: room.into(), at_ms: 0, says: says.into(), mentions: vec![], dm: false, follow_up: false, reply_to: None, new: false };
    let reason = |lines: &[Line], wake: Wake, beat: bool| wake_reason(lines, 0, &libby, &cast, wake, beat).map(|(why, _)| why);
    let named = [line(TOBIAH, "#minds:x", "Libby, tea?")];
    let unnamed = [line(TOBIAH, "#minds:x", "what a day")];
    let human = [line("@deru:x", "#minds:x", "kettle news")];
    let elsewhere = [line("@deru:x", "#other:x", "kettle news")];
    assert_eq!(reason(&named, Wake::HumansInRooms, false), Some("addressed by a Mind"));
    let by_id = [line(TOBIAH, "#minds:x", "ask @libby:gamecult.org about it")];
    let (why, by) = wake_reason(&by_id, 0, &libby, &cast, Wake::HumansInRooms, false).unwrap();
    assert_eq!((why, by[0]["how"].as_str()), ("addressed by a Mind", Some("id as text")));
    assert_eq!(wake_reason(&named, 0, &libby, &cast, Wake::HumansInRooms, false).unwrap().1[0]["how"], "name in text");
    let both = [line(TOBIAH, "#minds:x", "Libby, tea?"), line("@deru:x", "#minds:x", "Libby, kettle?")];
    assert_eq!(reason(&both, Wake::HumansInRooms, false), Some("addressed by a human"), "a human's address outranks a Mind's");
    assert_eq!(reason(&unnamed, Wake::HumansInRooms, false), None, "another Mind's unaddressed line wakes no one");
    assert_eq!(reason(&unnamed, Wake::AllMessages, false), Some("a message in her rooms"));
    assert_eq!(reason(&human, Wake::HumansInRooms, false), Some("a human in her rooms"));
    assert_eq!(reason(&elsewhere, Wake::HumansInRooms, false), None, "not her room");
    assert_eq!(reason(&unnamed, Wake::HumansInRooms, true), Some("the beat"));
    assert!(wake_reason(&named, 1, &libby, &cast, Wake::HumansInRooms, false).is_none(), "a line she has seen is not new");
}
