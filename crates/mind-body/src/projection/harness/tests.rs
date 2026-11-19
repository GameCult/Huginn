use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{Value, json};

use super::*;
use crate::address::is_for;
use crate::projection::narrate::{Answer, Failed, Spent, would_say};

const HER: &str = "@eureka:gamecult.org";

pub(crate) fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/harness")
}

pub(crate) fn control() -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixtures().join("states/control.json")).unwrap()).unwrap()
}

fn room(name: &str) -> Fixture {
    Fixture::load(&fixtures().join("rooms").join(name)).unwrap()
}

/// A recorded model: the Projector narrates a fixed line; the Persona stage answers by `persona(prompt)`. Every call is
/// kept, with its usage counted.
pub(crate) struct Scripted {
    pub calls: RefCell<Vec<(String, String)>>,
    pub persona: Box<dyn Fn(&str) -> String>,
}

impl Scripted {
    pub(crate) fn new(persona: impl Fn(&str) -> String + 'static) -> Self {
        Self { calls: RefCell::default(), persona: Box::new(persona) }
    }
}

impl Model for Scripted {
    fn ask(&self, _request_id: &str, model: &str, prompt: &str) -> std::result::Result<Answer, Failed> {
        self.calls.borrow_mut().push((model.to_owned(), prompt.to_owned()));
        let text = if prompt.starts_with("<!-- membrane:") { "The room is slow and so is he.".to_owned() } else { (self.persona)(prompt) };
        Ok(Answer { text, usage: Some(Spent { model: model.to_owned(), input_tokens: 10, output_tokens: 2, cache_read_tokens: 1, cache_write_tokens: 0, reasoning_tokens: None }) })
    }
}

/// Fails the test if her stored state is asked for.
pub(crate) struct NeverAsked;

impl PersonaSource for NeverAsked {
    fn persona(&self) -> Result<(Value, String)> {
        panic!("the stored state was read")
    }
}

fn harness<'a>(dir: &Path, persona: Option<&'a dyn PersonaSource>, model: Option<&'a dyn Model>) -> Harness<'a> {
    Harness { instance: "eureka".into(), persona, model, tuning: dir.join("persona-tuning.toml") }
}

#[test]
fn a_room_fixture_parses_into_the_mouths_pending_read() {
    let fixture = room("operator-warm.toml");
    assert_eq!((fixture.her.as_str(), fixture.kind, fixture.here.as_deref()), (HER, TurnKind::Chat, Some("!dm-meta:gamecult.org")));
    let pending = MouthReader::pending(&fixture).unwrap();
    assert_eq!(pending.deliveries.len(), 1, "only the new message is delivered");
    let dm = &pending.deliveries[0];
    assert_eq!((dm.sender.as_str(), dm.sender_display.as_str(), dm.is_direct, dm.basis), ("@meta:gamecult.org", "Meta", true, Some(Basis::Direct)));
    assert!(dm.text.contains("\"busy\""), "a quoted body survives TOML: {}", dm.text);
    assert_eq!(dm.at_unix_ms, unix_ms(fixture.now - Duration::minutes(1)));
    assert_eq!(pending.tail.unwrap().len(), 2, "the tail holds every line, delivered or not");
    assert_eq!(fixture.last_heard, vec![("@meta:gamecult.org".to_owned(), unix_ms(fixture.now - Duration::days(2)))]);
}

#[test]
fn is_for_decides_whether_a_fixture_message_is_for_her() {
    let her = Addressee::new(HER, ["eureka"]);
    let for_her = |fixture: &Fixture| MouthReader::pending(fixture).unwrap().deliveries.iter().map(|delivery| is_for(&her, delivery)).collect::<Vec<_>>();
    // Kiko's reply tags her and answers her own line.
    let teased = room("mind-teases.toml");
    let delivery = &MouthReader::pending(&teased).unwrap().deliveries[0];
    assert_eq!((delivery.basis, delivery.mentions.as_slice()), (Some(Basis::Tag), [HER.to_owned()].as_slice()));
    assert_eq!(for_her(&teased), vec![true]);
    // Deru's aside is not.
    assert_eq!(for_her(&room("deru-passing.toml")), vec![false]);
    // A name in the text is, a reply to her line is, and an aside is not.
    let fixture = Fixture::parse(
        r##"
her = "@eureka:gamecult.org"
kind = "chat"
now = "2026-11-20T15:00:00Z"
rooms = ["#minds:gamecult.org"]
[[message]]
from = "@eureka:gamecult.org"
room = "#minds:gamecult.org"
ago = "1h"
says = "mine"
[[message]]
from = "@deru:gamecult.org"
room = "#minds:gamecult.org"
new = true
says = "Eureka, tea?"
[[message]]
from = "@deru:gamecult.org"
room = "#minds:gamecult.org"
new = true
reply_to = 1
says = "agreed"
[[message]]
from = "@deru:gamecult.org"
room = "#minds:gamecult.org"
new = true
says = "the kettle again"
"##,
    )
    .unwrap();
    assert_eq!(for_her(&fixture), vec![true, true, false]);
}

#[test]
fn a_fixture_that_is_not_a_room_is_refused_naming_what_is_wrong() {
    let head = "her = \"@eureka:gamecult.org\"\nkind = \"chat\"\nnow = \"2026-11-20T15:00:00Z\"\nrooms = [\"#a:x\"]\n";
    for (tail, names) in [
        ("[[message]]\nfrom = \"@d:x\"\nroom = \"#b:x\"\nsays = \"hi\"\n", "not one of the rooms"),
        ("[[message]]\nfrom = \"@d:x\"\nroom = \"#a:x\"\nreply_to = 1\nsays = \"hi\"\n", "not an earlier one"),
        ("[[message]]\nfrom = \"@d:x\"\nroom = \"#a:x\"\nround = 2\nsays = \"hi\"\n", "round"),
        ("[[message]]\nfrom = \"@d:x\"\nroom = \"#a:x\"\nago = \"soon\"\nsays = \"hi\"\n", "90s"),
        ("[[message]]\nfrom = \"deru\"\nroom = \"#a:x\"\nsays = \"hi\"\n", "Matrix id"),
        ("mood = \"sunny\"\n", "unknown field"),
    ] {
        let fault = format!("{:#}", Fixture::parse(&format!("{head}{tail}")).unwrap_err());
        assert!(fault.contains(names), "{tail}: {fault}");
    }
}

#[test]
fn would_say_is_the_quoted_spans_of_her_prose() {
    assert_eq!(would_say("She looks up. \"Fine,\" she says, then \u{201C}mostly.\u{201D}"), vec!["Fine,", "mostly."]);
    assert!(would_say("She says nothing, and means it.").is_empty());
    assert!(would_say("She starts: \"well").is_empty(), "an unclosed quote says nothing");
    assert!(would_say("\"\" and \"  \"").is_empty());
}

#[test]
fn through_persona_continues_the_narration_in_her_prose_and_reports_what_she_would_say() {
    let dir = tempfile::tempdir().unwrap();
    let model = Scripted::new(|_| "He smiles at the screen. \"Missed you too, friend,\" he types. He does not mention the kettle.".into());
    let fixture = room("operator-warm.toml");
    let harness = harness(dir.path(), Some(&NeverAsked), Some(&model));
    let organs = Organs { persona: harness.persona, chat: Some(&fixture), addressing: fixture.addressee(), model: harness.model };
    let report = project(&organs, &fixture_request(&harness, &fixture, Some(control()), 3, Some(1), Through::Persona), fixture.now).unwrap();
    let render = &report["renders"][0];
    assert_eq!(render["persona"]["would_say"], json!(["Missed you too, friend,"]));
    assert_eq!(render["persona"]["words"], 17);
    assert_eq!((render["usage"]["input_tokens"].as_u64(), render["persona"]["usage"]["model"].as_str()), (Some(10), Some("claude-sonnet-5-5")));
    assert_eq!(report["persona_model"], "claude-sonnet-5-5");
    let calls = model.calls.borrow();
    assert_eq!(calls.len(), 2, "one Projector call and one Persona call");
    let persona_prompt = &calls[1].1;
    assert!(persona_prompt.starts_with("Everything below is prose from inside one person's life."), "Ghostlight's Persona membrane");
    assert!(persona_prompt.contains("The room is slow and so is he.") && persona_prompt.contains("hey you. I missed talking to you this week"), "the narration, then what she heard verbatim: {persona_prompt}");
    assert!(persona_prompt.contains("Use at most 180 words"), "the map's Persona budget");
    for field in ["socialBonds", "currentActivation", "bond-operator", "trust\":"] {
        assert!(!persona_prompt.contains(field), "no field name reaches the Persona: {field}");
    }
}

#[test]
fn prose_over_budget_is_cut_and_silence_says_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let long = format!("{} \"late words\"", "word ".repeat(200));
    let model = Scripted::new(move |_| long.clone());
    let fixture = room("deru-passing.toml");
    let harness = harness(dir.path(), None, Some(&model));
    let organs = Organs { persona: None, chat: Some(&fixture), addressing: None, model: harness.model };
    let report = project(&organs, &fixture_request(&harness, &fixture, Some(control()), 1, Some(1), Through::Persona), fixture.now).unwrap();
    let persona = &report["renders"][0]["persona"];
    assert_eq!((persona["words"].as_u64(), persona["cut_at_budget"].as_bool()), (Some(180), Some(true)));
    assert_eq!(persona["would_say"], json!([]), "the quote past the budget is never said");
}

#[test]
fn a_batch_runs_every_state_by_scenario_by_sample_into_one_report() {
    let dir = tempfile::tempdir().unwrap();
    let plan = dir.path().join("plan.toml");
    let rooms = fixtures().join("rooms");
    std::fs::write(
        &plan,
        format!(
            "samples = 2\nseed = 7\nthrough = \"persona\"\n[[state]]\nname = \"control\"\nfile = {:?}\n[[scenario]]\nname = \"quiet\"\nroom = {:?}\n[[scenario]]\nname = \"teased\"\nroom = {:?}\n",
            fixtures().join("states/control.json").display().to_string(),
            rooms.join("quiet-beat.toml").display().to_string(),
            rooms.join("mind-teases.toml").display().to_string()
        ),
    )
    .unwrap();
    let model = Scripted::new(|prompt| if prompt.contains("Kiko") { "\"Winter, still.\" He grins.".into() } else { "He lets the hour pass.".into() });
    let report = batch(&harness(dir.path(), Some(&NeverAsked), Some(&model)), &plan).unwrap();
    for heading in ["# Persona seed batch", "## Summary", "## control × quiet, sample 1 (seed 7)", "## control × quiet, sample 2 (seed 8)", "## control × teased, sample 2 (seed 8)", "## Total usage"] {
        assert!(report.contains(heading), "{heading}\n{report}");
    }
    assert!(report.contains("| control | teased | 2/2 |") && report.contains("| control | quiet | 0/2 |"), "{report}");
    assert!(report.contains("- \"Winter, still.\"") && report.contains("**Would say:** nothing (silence)"), "{report}");
    assert!(report.contains("| claude-sonnet-5-5 | 8 | 80 | 16 |"), "eight calls, two a case, in the footer: {report}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "the batch wrote nothing; the caller writes the report");
}

#[test]
fn a_batch_state_without_a_file_is_her_stored_state() {
    struct Stored;
    impl PersonaSource for Stored {
        fn persona(&self) -> Result<(Value, String)> {
            Ok((control(), "mind-commit-x".into()))
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let plan = dir.path().join("plan.toml");
    std::fs::write(&plan, format!("[[state]]\nname = \"stored\"\n[[scenario]]\nname = \"quiet\"\nroom = {:?}\n", fixtures().join("rooms/quiet-beat.toml").display().to_string())).unwrap();
    let model = Scripted::new(|_| String::new());
    let report = batch(&harness(dir.path(), Some(&Stored), Some(&model)), &plan).unwrap();
    assert!(report.contains("## stored × quiet, sample 1") && !report.contains("failed"), "{report}");
}
