//! One run of the job: forge refresh and chat refresh over a fake Synapse reached through the real client,
//! a fake vector store and the fake embedder; the exit status; the chat-off retirement; the purge request.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde_json::json;

use super::chat::{CHAT_PURGE_PREFIX, RoomHistory};
use super::job::{Chat, Purge, purge, run};
use super::matrix::HttpMatrix;
use super::matrix_tests::{Room, Rooms, TOKEN, message, state, synapse};
use super::room_scope::Estate;
use super::tests::{FakeForge, Remote};
use super::test_http::forge_of;
use super::*;
use crate::index::fakes::{FakeEmbedder, FakeSources};

const DAY0: u64 = 1_794_268_800_000; // 2026-11-10T00:00:00Z

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-17T00:00:00Z").unwrap().with_timezone(&Utc)
}

fn estate() -> Estate {
    Estate::new(["!e1:s", "!e2:s", "!e3:s"].map(str::to_owned).into()).unwrap()
}

struct Out {
    said: Vec<String>,
    complained: Vec<String>,
    status: u8,
}

struct Rig {
    embedder: FakeEmbedder,
    index: FakeSources,
    forge: FakeForge,
    _remote: Remote,
    root: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let remote = Remote::new();
        remote.commit(&[("README.md", b"# Demo\n")], &[]);
        Self { embedder: FakeEmbedder::new("d1"), index: FakeSources::default(), forge: FakeForge::of(&[("Demo", &remote)]), _remote: remote, root: tempfile::tempdir().unwrap() }
    }

    fn go<H: RoomHistory>(&mut self, history: Option<&mut H>) -> Out {
        let (mut said, mut complained) = (Vec::new(), Vec::new());
        let estate = estate();
        let chat = history.map(|history| Chat { history, estate: &estate });
        let status = run(&mut self.embedder, &mut self.index, &mut self.forge, self.root.path(), chat, &mut |line| said.push(line), &mut |line| complained.push(line));
        Out { said, complained, status }
    }

    fn chat_bodies(&self) -> Vec<String> {
        let mut bodies: Vec<String> = self.index.state.lock().unwrap().chat.values().flat_map(|point| point.payload.messages.iter().map(|m| m.body.clone())).collect();
        bodies.sort();
        bodies
    }

    fn chat_rooms(&self) -> std::collections::BTreeSet<String> {
        self.index.state.lock().unwrap().chat.values().map(|point| point.payload.room_id.clone()).collect()
    }
}

fn rooms(list: Vec<(&str, Vec<serde_json::Value>, Vec<serde_json::Value>)>) -> Rooms {
    Arc::new(Mutex::new(list.into_iter().map(|(id, state, timeline)| (id.to_owned(), Room { state, timeline })).collect::<BTreeMap<_, _>>()))
}

fn platform() -> String {
    ["disc", "ord"].concat()
}

fn world() -> Rooms {
    let relay = format!("@_{}_1:s", platform());
    rooms(vec![
        ("!swarm:s", vec![state("m.room.canonical_alias", json!({ "alias": "#swarm:gamecult.org" })), state("m.room.join_rules", json!({ "join_rule": "invite" }))], vec![
            message("$m1", "@a:s", DAY0 - 5000, "first words"),
            message("$m2", "@b:s", DAY0 - 4000, "to be redacted"),
            message("$m3", "@a:s", DAY0 + 1000, "inside the window"),
        ]),
        ("!bridged:s", vec![state("m.bridge", json!({}))], vec![message("$b1", "@a:s", DAY0, "bridged talk")]),
        ("!relay:s", vec![], vec![message("$d1", &relay, DAY0, "relayed talk")]),
        ("!aliased:s", vec![state("m.room.canonical_alias", json!({ "alias": "#Users:gamecult.org" }))], vec![message("$u1", "@a:s", DAY0, "estate talk")]),
    ])
}

fn all_lines(out: &Out) -> String {
    out.said.iter().chain(&out.complained).cloned().collect::<Vec<_>>().join("\n")
}

#[test]
fn a_full_run_over_a_fake_synapse() {
    let world = world();
    let (base, asked) = synapse(&world);
    let mut matrix = HttpMatrix::new(&base, TOKEN);
    let mut rig = Rig::new();

    let first = rig.go(Some(&mut matrix));
    assert_eq!(first.status, 1, "joined bridged, relay and estate rooms are refused: {:?}", first.complained);
    assert_eq!(rig.chat_rooms(), ["!swarm:s".to_owned()].into(), "an invite-only room is ingested; nothing of the refused rooms is");
    assert_eq!(rig.chat_bodies(), ["first words", "inside the window", "to be redacted"]);
    for refused in ["!bridged:s", "!relay:s", "!aliased:s"] {
        assert!(first.complained.iter().any(|line| line.starts_with("refused:") && line.contains(refused)), "{refused}: {:?}", first.complained);
    }
    assert!(!rig.embedder.texts().iter().any(|text| text.contains("bridged talk") || text.contains("relayed talk") || text.contains("estate talk")));
    assert!(first.said.iter().any(|line| line.starts_with("Demo ")), "the forge's line is journaled too");
    assert!(first.said.iter().any(|line| line.starts_with("#swarm:gamecult.org (!swarm:s)") && line.contains("events=3")), "{:?}", first.said);
    assert!(asked.seen().iter().all(|seen| seen.method == "GET"));

    // The steward asks for the window; then the room edits, redacts and leaks a key.
    let request = Purge::check("!swarm:s", "2026-11-10T00:00:00Z", "2026-11-11T00:00:00Z", "follow_up:x", now()).unwrap();
    assert_eq!(purge(&mut rig.index, &request).unwrap(), "chat-purge requested: !swarm:s 2026-11-10T00:00:00Z..2026-11-11T00:00:00Z request=\"follow_up:x\" windows=1");
    let before = rig.index.cursors();
    assert_eq!(purge(&mut rig.index, &request).unwrap().ends_with("windows=1"), true);
    assert_eq!(rig.index.cursors(), before, "a repeated request writes nothing");
    let secret = format!("key ghp_{}", "k2L9".repeat(9));
    {
        let mut held = world.lock().unwrap();
        let room = held.get_mut("!swarm:s").unwrap();
        room.timeline.push(json!({ "type": "m.room.message", "event_id": "$edit", "sender": "@a:s", "origin_server_ts": DAY0 + 90_000_000, "content": {
            "msgtype": "m.text", "body": "* first words, amended", "m.new_content": { "msgtype": "m.text", "body": "first words, amended" }, "m.relates_to": { "rel_type": "m.replace", "event_id": "$m1" } } }));
        room.timeline.push(json!({ "type": "m.room.redaction", "event_id": "$red", "sender": "@b:s", "origin_server_ts": DAY0 + 91_000_000, "content": { "redacts": "$m2" } }));
        room.timeline.push(message("$m4", "@a:s", DAY0 + 92_000_000, &secret));
    }
    let second = rig.go(Some(&mut matrix));
    assert_eq!(rig.chat_bodies().len(), 2, "{:?}", rig.chat_bodies());
    let bodies = rig.chat_bodies().join("\n");
    assert!(bodies.contains("first words, amended") && !bodies.contains("to be redacted") && !bodies.contains("inside the window"), "{bodies}");
    assert!(!bodies.contains("ghp_k2L9") && !rig.embedder.texts().iter().any(|text| text.contains("ghp_k2L9")), "the key was scrubbed before it was embedded");
    let text = all_lines(&second);
    assert!(second.said.iter().any(|line| line.contains("purged=")), "{text}");
    assert!(second.complained.iter().any(|line| line == "scrubbed: !swarm:s $m4 key-shape" || line.starts_with("scrubbed: !swarm:s $m4 ")), "{text}");
    for body in ["first words", "to be redacted", "inside the window", "relayed talk", "ghp_"] {
        assert!(!text.contains(body), "the journal carries ids and counts, not bodies: {body}");
    }
    let third = rig.go(Some(&mut matrix));
    assert!(!rig.chat_bodies().join("\n").contains("inside the window"), "a purged window does not come back");
    assert!(third.complained.iter().all(|line| line.starts_with("refused:")), "{:?}", third.complained);
}

#[test]
fn no_chat_flags_purge_the_corpus_and_make_no_matrix_call() {
    let world = world();
    let (base, _) = synapse(&world);
    let mut matrix = HttpMatrix::new(&base, TOKEN);
    let mut rig = Rig::new();
    rig.go(Some(&mut matrix));
    let request = Purge::check("!swarm:s", "2026-11-10T00:00:00Z", "2026-11-11T00:00:00Z", "r", now()).unwrap();
    purge(&mut rig.index, &request).unwrap();
    assert!(!rig.chat_bodies().is_empty());

    let (hung, asked) = forge_of(|_| None::<String>);
    drop(hung);
    let out = rig.go::<HttpMatrix>(None);
    assert_eq!(out.status, 0, "{:?}", out.complained);
    assert!(rig.chat_bodies().is_empty() && rig.chat_rooms().is_empty());
    assert!(rig.index.cursors().keys().any(|key| key.starts_with(CHAT_PURGE_PREFIX)), "tombstones stay");
    assert!(!rig.index.cursors().keys().any(|key| key.starts_with("chat:")), "cursors go");
    assert!(asked.seen().is_empty());
}

struct BrokenForge;

impl Forge for BrokenForge {
    fn catalog(&mut self) -> Result<Catalog> {
        bail!("the forge is down")
    }

    fn clone_url(&self, _name: &str) -> String {
        String::new()
    }
}

struct DeadSynapse;

impl RoomHistory for DeadSynapse {
    fn joined_rooms(&mut self) -> Result<Vec<String>> {
        bail!("synapse is down")
    }

    fn facts(&mut self, _room_id: &str) -> Result<super::room_scope::RoomFacts> {
        bail!("synapse is down")
    }

    fn events_after(&mut self, _room_id: &str, _after: Option<&str>, _limit: u32) -> Result<Option<super::chat::EventPage>> {
        bail!("synapse is down")
    }
}

#[test]
fn exit_status_is_clean_only_when_forge_and_chat_both_are() {
    let clean = rooms(vec![("!swarm:s", vec![], vec![message("$m1", "@a:s", 1, "words")])]);
    let (base, _) = synapse(&clean);
    let mut rig = Rig::new();
    let out = rig.go(Some(&mut HttpMatrix::new(&base, TOKEN)));
    assert_eq!((out.status, out.complained.len()), (0, 0));

    // Scrub hits alone are not a failure; they are reported.
    clean.lock().unwrap().get_mut("!swarm:s").unwrap().timeline.push(message("$m2", "@a:s", 2, &format!("ghp_{}", "k2L9".repeat(9))));
    let out = rig.go(Some(&mut HttpMatrix::new(&base, TOKEN)));
    assert_eq!(out.status, 0, "{:?}", out.complained);
    assert!(out.complained.iter().any(|line| line.starts_with("scrubbed: !swarm:s $m2")));

    // A chat failure still reports the forge, and a forge failure still runs chat.
    let out = rig.go(Some(&mut DeadSynapse));
    assert_eq!(out.status, 1);
    assert!(out.complained.iter().any(|line| line.contains("synapse is down")));
    let (mut said, mut complained) = (Vec::new(), Vec::new());
    let (estate, mut ok) = (estate(), HttpMatrix::new(&base, TOKEN));
    let status = run(&mut rig.embedder, &mut rig.index, &mut BrokenForge, rig.root.path(), Some(Chat { history: &mut ok, estate: &estate }), &mut |l| said.push(l), &mut |l| complained.push(l));
    assert_eq!(status, 1);
    assert!(complained.iter().any(|line| line.contains("the forge is down")), "{complained:?}");
    assert!(said.iter().any(|line| line.contains("(!swarm:s)")), "chat ran after the forge failed: {said:?}");
}

#[test]
fn a_request_cannot_forge_a_journal_line() {
    let mut rig = Rig::new();
    rig.go::<HttpMatrix>(None);
    let request = Purge::check("!swarm:s", "2026-11-10T00:00:00Z", "2026-11-11T00:00:00Z", "x\nrefused: !forged:s", now()).unwrap();
    let line = purge(&mut rig.index, &request).unwrap();
    assert_eq!(line.lines().count(), 1, "{line:?}");
}

#[test]
fn a_failed_room_report_goes_to_the_complaint_stream_and_fails_the_run() {
    let mut rig = Rig::new();
    struct FactsDown(Rooms);
    impl RoomHistory for FactsDown {
        fn joined_rooms(&mut self) -> Result<Vec<String>> {
            Ok(vec!["!swarm:s".to_owned()])
        }
        fn facts(&mut self, _room_id: &str) -> Result<super::room_scope::RoomFacts> {
            bail!("facts are down")
        }
        fn events_after(&mut self, _room_id: &str, _after: Option<&str>, _limit: u32) -> Result<Option<super::chat::EventPage>> {
            bail!("unreachable")
        }
    }
    let out = rig.go(Some(&mut FactsDown(rooms(vec![]))));
    assert_eq!(out.status, 1);
    assert!(out.complained.iter().any(|line| line.contains("!swarm:s") && line.contains("facts are down")), "{:?}", out.complained);
    assert!(!out.said.iter().any(|line| line.contains("facts are down")), "{:?}", out.said);
}

#[test]
fn chat_purge_makes_no_matrix_request() {
    let world = world();
    let (base, asked) = synapse(&world);
    let mut rig = Rig::new();
    rig.go(Some(&mut HttpMatrix::new(&base, TOKEN)));
    let made = asked.seen().len();
    let request = Purge::check("!swarm:s", "2026-11-10T00:00:00Z", "2026-11-11T00:00:00Z", "r", now()).unwrap();
    purge(&mut rig.index, &request).unwrap();
    assert_eq!(asked.seen().len(), made);
}
