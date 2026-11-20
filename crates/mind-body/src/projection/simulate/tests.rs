use std::path::Path;

use serde_json::Value;

use super::*;
use crate::projection::harness::tests::{Scripted, control};
use serde_json::json;
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
    let line = |from: &str, says: &str| Line { from: from.into(), name: String::new(), room: "#minds:x".into(), at_ms: 0, says: says.into(), mentions: vec![], dm_to: None, follow_up: false, reply_to: None, new: false };
    let ask = |policy: WakePolicy, from: &str, says: &str, speaker: Speaker, run: usize| wakes(policy, &libby, &line(from, says).delivery(0, LIBBY, &[]), speaker, LineFacts { mind_run: run, ..LineFacts::default() });
    let paced = |says: &str, own_room: bool, in_exchange: bool| wakes(WakePolicy::HumansPace(4), &libby, &line("@meta:x", says).delivery(0, LIBBY, &[]), Speaker::Operator, LineFacts { mind_run: 0, own_room, in_exchange });
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
    // Humans pace: a Mind's line never wakes a Mind; a human's wakes her in her own room, or in a public room when it
    // addresses her or continues an exchange she is in.
    assert_eq!(ask(WakePolicy::HumansPace(4), TOBIAH, named, Speaker::Mind, 0), None, "a name in prose wakes no one");
    let tagged = Line { mentions: vec![LIBBY.into()], ..line(TOBIAH, unnamed) };
    let explicit = |run: usize| wakes(WakePolicy::HumansPace(4), &libby, &tagged.delivery(0, LIBBY, &[]), Speaker::Mind, LineFacts { mind_run: run, ..LineFacts::default() });
    assert_eq!(explicit(3), Some(AddressedByMind), "a Matrix mention by a Mind wakes her, in any room");
    assert_eq!(explicit(4), None, "the backstop: past the cap of Mind lines in a row, no Mind wakes another");
    assert_eq!(paced(unnamed, false, false), None, "a public room's human line that is not hers to answer");
    assert_eq!(paced(unnamed, true, false), Some(Human));
    assert_eq!(paced(unnamed, false, true), Some(Human));
    assert_eq!(paced(named, false, false), Some(AddressedByHuman));
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
    let (_, paced) = run_plan(&plan(dir.path(), "{ humans_pace = 4 }", 6, ""), &namers());
    assert!(turns(&paced).is_empty(), "under humans_pace an unaddressed public line wakes no one, and no Mind's line ever does");
}

#[test]
fn under_humans_pace_her_own_room_wakes_her_and_a_public_room_only_by_address_or_her_exchange() {
    let dir = tempfile::tempdir().unwrap();
    cast(dir.path());
    let path = dir.path().join("plan.toml");
    let line = |round: usize, room: &str, says: &str| format!("[[message]]\nround = {round}\nfrom = \"@meta:gamecult.org\"\nname = \"Meta\"\nroom = \"{room}\"\nsays = \"{says}\"\n");
    std::fs::write(
        &path,
        format!(
            "rounds = 4\nseed = 11\nwake = {{ humans_pace = 4 }}\nbeat_s = 0\nstart = \"2026-11-20T15:00:00Z\"\nrooms = [\"#minds:gamecult.org\", \"#tobiah:gamecult.org\"]\noperators = [\"@meta:gamecult.org\"]\n[[cast]]\nid = \"{TOBIAH}\"\nname = \"Tobiah\"\nstate = \"tobiah.json\"\nown_rooms = [\"#tobiah:gamecult.org\"]\n[[cast]]\nid = \"{LIBBY}\"\nname = \"Libby\"\nstate = \"libby.json\"\n{}{}{}",
            line(0, "#minds:gamecult.org", "Libby, how was the gig?"),
            line(2, "#minds:gamecult.org", "and the rest of you?"),
            line(3, "#tobiah:gamecult.org", "quiet in here today")
        ),
    )
    .unwrap();
    let (members, run) = run_plan(&path, &namers());
    let turns = turns(&run);
    assert_eq!(
        turns,
        vec![(1, "Libby".to_owned(), "addressed_by_human".to_owned()), (2, "Libby".to_owned(), "human".to_owned()), (3, "Tobiah".to_owned(), "human".to_owned())],
        "named in #minds, then her exchange there, then his own room; Libby's naming Tobiah wakes no one"
    );
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

/// Tobiah says a line that names Libby and holds her id as text; the Interpreter gives it `mentions` or not.
fn tagging(mention: bool) -> Scripted {
    let mentions = if mention { json!([LIBBY]) } else { json!([]) };
    Scripted::new(|prompt| if is_tobiah(prompt) { "\"ask @libby:gamecult.org, and Libby too\"".into() } else { "She lets it be.".into() }).interpreting(move |prompt| {
        if !prompt.contains("Who she is:\nTobiah") {
            return json!({ "say": [] }).to_string();
        }
        json!({ "say": [{ "spans": ["ask @libby:gamecult.org, and Libby too"], "mentions": mentions, "reply_to": null }] }).to_string()
    })
}

#[test]
fn only_the_interpreters_structured_mention_is_a_mention_and_an_at_in_the_text_is_not() {
    // Libby says hello first, so the packet shows her to Tobiah: a mention reaches only someone shown.
    let hello = "[[message]]\nround = 0\nfrom = \"@libby:gamecult.org\"\nname = \"Libby\"\nroom = \"#minds:gamecult.org\"\nsays = \"hello all\"\n";
    let plan = |dir: &Path, wake: &str, rounds: usize| {
        let path = plan(dir, wake, rounds, "");
        std::fs::write(&path, format!("{}{hello}", std::fs::read_to_string(&path).unwrap())).unwrap();
        path
    };
    let dir = tempfile::tempdir().unwrap();
    let (_, plain) = run_plan(&plan(dir.path(), "\"ruled\"", 3), &tagging(false));
    let addressed = &plain["metrics"]["mind_addressed_wakes"];
    assert_eq!(addressed["mention_or_reply"].as_u64(), Some(0), "an @id in the words tags no one: {addressed}");
    let dir = tempfile::tempdir().unwrap();
    let (_, tagged) = run_plan(&plan(dir.path(), "\"ruled\"", 3), &tagging(true));
    let addressed = &tagged["metrics"]["mind_addressed_wakes"];
    let libby_mind_wakes = turns(&tagged).iter().filter(|(_, who, why)| who == "Libby" && why == "addressed_by_mind").count();
    assert_eq!(addressed["mention_or_reply"].as_u64().unwrap() as usize, libby_mind_wakes, "{addressed}");
    // Under humans_pace a name or an @ in the words wakes no one; the structured mention does.
    let dir = tempfile::tempdir().unwrap();
    let path = plan(dir.path(), "{ humans_pace = 4 }", 3);
    let opening = "says = \"good afternoon, everyone\"\n";
    std::fs::write(&path, std::fs::read_to_string(&path).unwrap().replace(opening, &format!("{opening}mentions = [\"{TOBIAH}\"]\n"))).unwrap();
    let woken_libby = |model: &Scripted| turns(&run_plan(&path, model).1).iter().any(|(_, who, _)| who == "Libby");
    assert!(!woken_libby(&tagging(false)));
    assert!(woken_libby(&tagging(true)));
}

#[test]
fn a_direct_message_in_one_of_the_plans_rooms_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = plan(dir.path(), "\"ruled\"", 2, "");
    let dm = "[[message]]\nround = 1\nfrom = \"@meta:gamecult.org\"\nname = \"Meta\"\nroom = \"#minds:gamecult.org\"\ndm = true\nto = \"@libby:gamecult.org\"\nsays = \"psst\"\n";
    std::fs::write(&path, format!("{}{dm}", std::fs::read_to_string(&path).unwrap())).unwrap();
    let model = silent();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    assert!(format!("{:#}", simulate(&harness, &path).unwrap_err()).contains("has its own room"));
}

#[test]
fn the_mind_run_is_counted_across_her_rooms_and_an_uncounted_run_is_at_the_cap() {
    let her = Member { names_from: "test", addressee: Addressee::new("@kiko:x", ["kiko"]), entry: CastEntry { id: "@kiko:x".into(), name: "Kiko".into(), state: PathBuf::new(), rooms: vec![], own_rooms: vec![] }, state: Value::Null, rooms: vec!["#a:x".into(), "#b:x".into()] };
    let cast = vec![TOBIAH.to_owned(), LIBBY.to_owned(), "@kiko:x".to_owned(), "@other:x".to_owned()];
    let line = |from: &str, room: &str| Line { from: from.into(), name: String::new(), room: room.into(), at_ms: 0, says: "hi".into(), mentions: vec![], dm_to: None, follow_up: false, reply_to: None, new: false };
    let lines = [line("@meta:x", "#a:x"), line(TOBIAH, "#a:x"), line("@other:x", "#b:x"), line(TOBIAH, "#b:x"), line("@meta:x", "#c:x")];
    assert_eq!(mind_runs(&lines, &cast, &her), vec![0, 0, 1, 2, 3], "one run across #a and #b; a room she is not in does not break it");
    // Soul's case: Meta chatting in #eureka does not end a loop in #minds; a word from Meta in #minds does.
    let kiko = Member { rooms: vec!["#minds:x".into(), "#eureka:x".into()], ..her };
    let lines = [line(TOBIAH, "#minds:x"), line(LIBBY, "#minds:x"), line("@meta:x", "#eureka:x"), line(TOBIAH, "#minds:x"), line("@meta:x", "#minds:x"), line(LIBBY, "#minds:x")];
    assert_eq!(mind_runs(&lines, &cast, &kiko), vec![0, 1, 2, 2, 3, 0]);
    assert_eq!(LineFacts::default().mind_run, usize::MAX, "fail-safe: an uncounted run is at the cap");
    let tagged = Line { mentions: vec!["@kiko:x".into()], ..line(TOBIAH, "#a:x") };
    assert_eq!(wakes(WakePolicy::HumansPace(4), &kiko.addressee, &tagged.delivery(0, "@kiko:x", &[]), Speaker::Mind, LineFacts::default()), None);
}

#[test]
fn the_report_names_its_gaps_and_carries_representative_lines_and_usage() {
    let dir = tempfile::tempdir().unwrap();
    let model = chatty();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    let report = simulate(&harness, &plan(dir.path(), "\"ruled\"", 3, "")).unwrap();
    for part in ["# Swarm simulation", "State does not evolve during a run", "Speech is the Interpreter's", "group state in the Projector's packet: on", "### Representative lines", "**Meta (human)**: good afternoon, everyone", "**Tobiah** _(woke: human)_: what a day", "### Wakes by round", "| 1 | 0 | 2 | 0 | 0 | 0 |", "## Total usage", "| claude-sonnet-5-5 | 6 |"] {
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

#[test]
fn a_scripted_direct_message_reaches_and_wakes_only_its_recipient() {
    let dir = tempfile::tempdir().unwrap();
    let path = plan(dir.path(), "\"ruled\"", 2, "");
    let dm = "[[message]]\nround = 1\nfrom = \"@meta:gamecult.org\"\nname = \"Meta\"\nroom = \"!dm-libby:gamecult.org\"\ndm = true\nto = \"@libby:gamecult.org\"\nsays = \"just you, how are you?\"\n";
    let text = std::fs::read_to_string(&path).unwrap().replace("[[message]]\nfrom = \"@meta:gamecult.org\"\nname = \"Meta\"\nroom = \"#minds:gamecult.org\"\nsays = \"good afternoon, everyone\"\n", dm);
    std::fs::write(&path, &text).unwrap();
    let (_, run) = run_plan(&path, &silent());
    assert_eq!(turns(&run), vec![(1, "Libby".to_owned(), "addressed_by_human".to_owned())], "only the recipient sees it, and it is for her");
    std::fs::write(&path, text.replace("to = \"@libby:gamecult.org\"\n", "")).unwrap();
    let model = silent();
    let harness = Harness { instance: "eureka".into(), persona: None, model: Some(&model), tuning: dir.path().join("persona-tuning.toml") };
    assert!(format!("{:#}", simulate(&harness, &path).unwrap_err()).contains("name its recipient"), "a direct message without a recipient is refused");
}

#[test]
fn the_turn_is_in_the_room_of_the_line_that_woke_her() {
    let dir = tempfile::tempdir().unwrap();
    cast(dir.path());
    let path = dir.path().join("plan.toml");
    let line = |round: usize, from: &str, room: &str, says: &str| format!("[[message]]\nround = {round}\nfrom = \"{from}\"\nname = \"X\"\nroom = \"{room}\"\nsays = \"{says}\"\n");
    // Meta speaks in Tobiah's own room, then Libby says something in #minds after it: Tobiah's turn is in his room.
    std::fs::write(
        &path,
        format!(
            "rounds = 1\nseed = 11\nwake = {{ humans_pace = 4 }}\nbeat_s = 0\nstart = \"2026-11-20T15:00:00Z\"\nrooms = [\"#minds:gamecult.org\", \"#tobiah:gamecult.org\"]\noperators = [\"@meta:gamecult.org\"]\n[[cast]]\nid = \"{TOBIAH}\"\nname = \"Tobiah\"\nstate = \"tobiah.json\"\nown_rooms = [\"#tobiah:gamecult.org\"]\n[[cast]]\nid = \"{LIBBY}\"\nname = \"Libby\"\nstate = \"libby.json\"\n{}{}",
            line(0, "@meta:gamecult.org", "#tobiah:gamecult.org", "quiet in here"),
            line(0, LIBBY, "#minds:gamecult.org", "hello all")
        ),
    )
    .unwrap();
    let (_, run) = run_plan(&path, &silent());
    assert_eq!(run["turns"][0]["room"], "#tobiah:gamecult.org", "{}", run["turns"]);
}

#[test]
fn soul_r7_woken_in_a_direct_message_and_a_room_at_once_she_takes_a_turn_in_each() {
    let dir = tempfile::tempdir().unwrap();
    let path = plan(dir.path(), "\"ruled\"", 1, "");
    let line = |room: &str, dm: bool, says: &str| format!("[[message]]\nround = 1\nfrom = \"@meta:gamecult.org\"\nname = \"Meta\"\nroom = \"{room}\"\n{}says = \"{says}\"\n", if dm { "dm = true\nto = \"@libby:gamecult.org\"\n" } else { "" });
    let opening = "[[message]]\nfrom = \"@meta:gamecult.org\"\nname = \"Meta\"\nroom = \"#minds:gamecult.org\"\nsays = \"good afternoon, everyone\"\n";
    let text = std::fs::read_to_string(&path).unwrap().replace(opening, &format!("{}{}", line("!dm-libby:gamecult.org", true, "just you: how are you, really?"), line("#minds:gamecult.org", false, "Libby, how was the gig?")));
    std::fs::write(&path, text).unwrap();
    let (_, run) = run_plan(&path, &silent());
    let mut libby: Vec<(String, String)> = run["turns"].as_array().unwrap().iter().filter(|turn| turn["persona"] == "Libby").map(|turn| (turn["room"].as_str().unwrap().to_owned(), turn["kind"].as_str().unwrap().to_owned())).collect();
    libby.sort();
    assert_eq!(libby, vec![("!dm-libby:gamecult.org".to_owned(), "chat".to_owned()), ("#minds:gamecult.org".to_owned(), "chat".to_owned())], "{}", run["turns"]);
    // Each turn read its own room's line, and no other's.
    for turn in run["turns"].as_array().unwrap().iter().filter(|turn| turn["persona"] == "Libby") {
        let stimulus = turn["report"]["renders"][0].to_string() + &turn["report"].to_string();
        let (own, other) = if turn["room"] == "#minds:gamecult.org" { ("how was the gig", "how are you, really") } else { ("how are you, really", "how was the gig") };
        assert!(stimulus.contains(own) && !stimulus.contains(other), "{}: {stimulus}", turn["room"]);
    }
}
