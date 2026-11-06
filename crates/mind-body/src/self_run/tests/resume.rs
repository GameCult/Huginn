//! A Mind's next Self process resumes the last one's conversation while its cache
//! is warm, and clears it once cold (rulings `eureka-body:ruling:session-rotate-and-resume`
//! and `eureka-body:ruling:live-session-floor-cache-evicts`). `decide` owns the
//! timing and is pinned in its own tests; here are the executions: the child's
//! arguments and first line, the one keep call per process, the files.

use std::path::PathBuf;

use super::closure::reason_of;
use super::*;

const PREVIOUS: &str = "11111111-1111-4111-8111-111111111111";

fn previous() -> Uuid {
    Uuid::parse_str(PREVIOUS).unwrap()
}

fn kept_for(warm_for_s: i64, same_seed: bool) -> Kept {
    Kept { session: previous(), transcript: PathBuf::from(format!("/state/session/{PREVIOUS}.jsonl")), warm_until: at_start() + ChronoDuration::seconds(warm_for_s), same_seed }
}

fn session_a() -> Uuid {
    Uuid::parse_str(SESSION_OF_RUN_A).unwrap()
}

/// The events of `ran` after the child was started, in order, that are keeps.
fn keeps_after_spawn(ran: &Ran) -> Vec<Option<(Uuid, chrono::DateTime<chrono::Utc>)>> {
    let spawned = ran.all.iter().position(|event| *event == Event::Spawned).expect("a child was started");
    ran.all[spawned..]
        .iter()
        .filter_map(|event| match event {
            Event::Kept(keep) => Some(*keep),
            _ => None,
        })
        .collect()
}

fn answered() -> Vec<Step> {
    vec![Out(at("m1", 10)), Out(echo()), Out(ok_result())]
}

#[test]
fn claude_args_resumes_a_kept_transcript_by_its_path_forked_and_otherwise_is_the_fixed_list() {
    let fresh = claude_args(session_a(), None);
    let path = PathBuf::from(format!("/state/session/{PREVIOUS}.jsonl"));
    let resumed = claude_args(session_a(), Some(&path));
    assert_eq!(resumed[..fresh.len()], fresh[..], "the fixed list is unchanged");
    assert_eq!(resumed[fresh.len()..], ["--resume".to_string(), path.to_string_lossy().into_owned(), "--fork-session".to_string()]);
    assert!(!fresh.iter().any(|arg| arg == "--resume" || arg == "--fork-session"));
    for forbidden in ["--continue", "--model", "--system-prompt", "--max-budget"] {
        assert!(!resumed.iter().any(|arg| arg.starts_with(forbidden) || arg.contains("dangerously")), "{forbidden}");
    }
}

#[test]
fn a_warm_kept_conversation_is_resumed_forked_and_told_it_continues_under_the_new_run() {
    let mind = with_run();
    let ran = carry_kept(&Seen::of(&mind), answered(), (Some(0), None, &[], false), Some(kept_for(1800, true)));
    ran.outcome.as_ref().unwrap();
    let path = PathBuf::from(format!("/state/session/{PREVIOUS}.jsonl"));
    assert_eq!(ran.spawns, vec![claude_args(session_a(), Some(&path))]);
    let first = &ran.written()[0];
    assert!(first.starts_with("Your session continues in a new process under a new run: your run is now `yggdrasil:run:selfrun-a`; the previous run is closed. Admit as faculty SelfFaculty, agent mind-self-run."), "{first}");
    assert!(first.contains("Plan usage: five_hour 42.3%"), "{first}");
    assert!(!first.contains("Use the eureka skill"), "the conversation holds the skill already: {first}");
    assert!(ran.out.contains(&format!("session resumed {PREVIOUS} warm until 2026-10-09T13:45:00Z")), "{}", ran.out);
    assert!(reason_of(&mind, "selfrun-a").ends_with(&format!("; resumed {PREVIOUS}")), "{}", reason_of(&mind, "selfrun-a"));
}

#[test]
fn a_resumed_start_line_carries_the_open_briefs_as_a_fresh_one_does() {
    let first = |kept: Option<Kept>| {
        let mind = live_mind();
        persona_commit(&mind, vec![brief("p1", "Still open.")]);
        carry_kept(&Seen::of(&mind), answered(), (Some(0), None, &[], false), kept).written().remove(0)
    };
    let (resumed, fresh) = (first(Some(kept_for(1800, true))), first(None));
    let task = |line: &str| line[line.find("\n\nAlso open for you").expect("the briefs section")..].to_string();
    assert_eq!(task(&resumed), task(&fresh));
    assert!(task(&resumed).contains("Still open."));
}

#[test]
fn a_cold_or_seed_changed_or_absent_kept_conversation_is_cleared_before_the_child_starts_and_the_start_is_fresh() {
    let cases: [(&str, Option<Kept>); 4] = [
        ("none kept", None),
        ("cold", Some(kept_for(-1, true))),
        ("cold", Some(kept_for(0, true))),
        ("seed changed", Some(kept_for(1800, false))),
    ];
    for (why, kept) in cases {
        let mind = with_run();
        let ran = carry_kept(&Seen::of(&mind), answered(), (Some(0), None, &[], false), kept);
        ran.outcome.as_ref().unwrap();
        let first_keep = ran.all.iter().position(|event| matches!(event, Event::Kept(_))).unwrap();
        let spawned = ran.all.iter().position(|event| *event == Event::Spawned).unwrap();
        assert_eq!(ran.all[first_keep], Event::Kept(None), "{why}");
        assert!(first_keep < spawned, "the clear precedes the child: {why}");
        assert_eq!(ran.spawns, vec![claude_args(session_a(), None)], "{why}");
        assert!(ran.written()[0].starts_with("Use the eureka skill."), "{why}");
        assert!(ran.out.contains(&format!("session fresh: {why}")), "{why}: {}", ran.out);
        assert!(reason_of(&mind, "selfrun-a").ends_with("; fresh"), "{why}");
    }
}

#[test]
fn after_the_child_ends_a_clean_end_is_kept_under_this_runs_session_and_any_other_end_clears() {
    // A clean end: one keep, naming this run's session and the outcome's warm_until.
    let mind = with_run();
    let ran = carry_with(&Seen::of(&mind), answered(), Some(0), false);
    let SelfRunOutcome::Ended { warm_until: Some(until), .. } = ran.outcome.as_ref().unwrap() else { panic!("not kept: {:?}", ran.outcome) };
    assert_eq!(keeps_after_spawn(&ran), vec![Some((session_a(), *until))]);
    assert!(ran.out.contains("session kept until"), "{}", ran.out);

    // Ends that are not clean: one clearing keep each.
    let ceiling = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Out(echo()), Out(ok_result())], Some(0), false);
    let failed = carry_with(&Seen::of(&with_run()), answered(), Some(1), false);
    let unanswered = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", 10))], Some(0), false);
    for (name, ran) in [("ceiling", ceiling), ("non-zero exit", failed), ("no result", unanswered)] {
        assert_eq!(keeps_after_spawn(&ran), vec![None], "{name}");
        assert!(ran.out.contains("session cleared"), "{name}: {}", ran.out);
    }
}

#[test]
fn a_drive_that_fails_clears_the_conversation() {
    struct Refuses;
    impl Write for Refuses {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if String::from_utf8_lossy(bytes).contains("result subtype") {
                return Err(std::io::Error::other("closed"));
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mind = with_run();
    let log = Rc::new(RefCell::new(Vec::new()));
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let steps = answered();
    let fake = Fake::with(Some(Scripted { steps: steps.into(), log: log.clone(), clock: clock.clone(), code: Some(0), dies_on: None, refuses: vec![], hangs: false, exited: false, eof: false, asked: 0 }), false);
    *fake.kept.borrow_mut() = Some(kept_for(1800, true));
    let ran = carry(&Seen::of(&mind), &*clock, &FakeLender::logging(log.clone(), Answer::Grant), &fake, TEST_CAP_S, &mut Refuses);
    assert!(ran.is_err());
    let keeps: Vec<_> = log.borrow().iter().filter(|event| matches!(event, Event::Kept(_))).cloned().collect();
    assert_eq!(keeps, vec![Event::Kept(None)], "a resumed start keeps nothing, and the failed drive clears");
}

// ---- the files -----------------------------------------------------------------

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn the_configs_end_moves_the_main_transcripts_out_and_nothing_else_of_it() {
    let scratch = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let keep = state.path().join("session");
    let config = FreshConfig::create(scratch.path(), None, Some(keep.clone())).unwrap();
    let dir = scratch.path().join(CONFIG_DIR_NAME);
    write(&dir.join("projects/x/aaa.jsonl"), "main");
    write(&dir.join("projects/y/bbb.jsonl"), "other project");
    write(&dir.join("projects/x/memory/m.md"), "memory");
    write(&dir.join("projects/x/memory/n.jsonl"), "memory transcript");
    write(&dir.join("projects/x/aaa/sub.jsonl"), "session subdirectory");
    write(&dir.join("projects/x/deep/a/c.jsonl"), "too deep");
    write(&dir.join("projects/loose.jsonl"), "no project");
    write(&dir.join("projects/x/notes.txt"), "not a transcript");
    write(&dir.join("settings.json"), "{}");
    drop(config);
    assert_eq!(names(&keep), ["aaa.jsonl", "bbb.jsonl"]);
    assert_eq!(std::fs::read_to_string(keep.join("aaa.jsonl")).unwrap(), "main");
    assert!(!dir.exists(), "the config still goes");
    #[cfg(unix)]
    assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&keep).unwrap().permissions()) & 0o777, 0o700);

    // Where nothing is kept, nothing is made.
    let config = FreshConfig::create(scratch.path(), None, None).unwrap();
    write(&scratch.path().join(CONFIG_DIR_NAME).join("projects/x/ccc.jsonl"), "x");
    drop(config);
    assert_eq!(names(&keep), ["aaa.jsonl", "bbb.jsonl"]);
}

fn spawner_over(state: &Path, scratch: &Path, seed: Option<&Path>) -> ClaudeSpawner {
    let mut environment = vec![(SCRATCH_VARIABLE.into(), scratch.into()), (STATE_DIRECTORY_VARIABLE.into(), state.into())];
    if let Some(seed) = seed {
        environment.push((SEED_VARIABLE.into(), seed.into()));
    }
    ClaudeSpawner::new(environment).unwrap()
}

fn transcript(state: &Path, session: Uuid) -> PathBuf {
    let path = state.join("session").join(format!("{session}.jsonl"));
    write(&path, "{}");
    path
}

#[test]
fn keeping_one_conversation_removes_the_one_before_it_and_clearing_leaves_the_dir_empty() {
    let state = tempfile::tempdir().unwrap();
    let spawner = spawner_over(state.path(), state.path(), None);
    let (older, newer) = (previous(), session_a());
    let until = at_start() + ChronoDuration::seconds(900);
    transcript(state.path(), older);
    let newer_path = transcript(state.path(), newer);
    write(&state.path().join("session/stray.jsonl.tmp"), "half");

    spawner.keep(Some((newer, until))).unwrap();
    assert_eq!(names(&state.path().join("session")), [format!("{newer}.jsonl"), "kept.json".to_string()], "at most one transcript, and no stray");
    assert_eq!(spawner.kept(), Some(Kept { session: newer, transcript: newer_path, warm_until: until, same_seed: true }));

    spawner.keep(None).unwrap();
    assert!(names(&state.path().join("session")).is_empty());
    assert_eq!(spawner.kept(), None);

    // A conversation whose transcript did not come out is a fresh start, not a record naming nothing.
    spawner.keep(Some((older, until))).unwrap();
    assert!(names(&state.path().join("session")).is_empty());
}

#[test]
fn a_record_that_is_missing_unreadable_or_names_a_missing_transcript_keeps_nothing() {
    let state = tempfile::tempdir().unwrap();
    let spawner = spawner_over(state.path(), state.path(), None);
    let dir = state.path().join("session");
    assert_eq!(spawner.kept(), None, "no directory");
    transcript(state.path(), previous());
    assert_eq!(spawner.kept(), None, "a transcript with no record");
    write(&dir.join("kept.json"), "not json");
    assert_eq!(spawner.kept(), None, "unparseable");
    write(&dir.join("kept.json"), &format!(r#"{{"session":"{PREVIOUS}","warm_until":"yesterday","seed":""}}"#));
    assert_eq!(spawner.kept(), None, "an unreadable instant");
    write(&dir.join("kept.json"), &format!(r#"{{"session":"{}","warm_until":"2026-10-09T13:45:00Z","seed":""}}"#, session_a()));
    assert_eq!(spawner.kept(), None, "names a transcript that is not there");
    write(&dir.join("kept.json"), &format!(r#"{{"session":"{PREVIOUS}","warm_until":"2026-10-09T13:45:00Z","seed":""}}"#));
    assert!(spawner.kept().is_some_and(|kept| kept.same_seed));
}

#[test]
fn without_a_state_directory_nothing_is_kept_and_nothing_is_made() {
    let scratch = tempfile::tempdir().unwrap();
    let spawner = ClaudeSpawner::new([(SCRATCH_VARIABLE.into(), scratch.path().into())]).unwrap();
    spawner.keep(Some((previous(), at_start()))).unwrap();
    spawner.keep(None).unwrap();
    assert_eq!(spawner.kept(), None);
    assert!(names(scratch.path()).is_empty());
}

#[test]
fn a_changed_seed_file_or_link_target_changes_the_digest_and_an_identical_copy_does_not() {
    #[cfg(unix)]
    {
        let seed = tempfile::tempdir().unwrap();
        write(&seed.path().join("skills/a.md"), "alpha");
        write(&seed.path().join("settings.json"), "{}");
        std::os::unix::fs::symlink("skills", seed.path().join("link")).unwrap();
        let digest = seed_digest(seed.path()).unwrap();
        assert_eq!(digest.len(), 64);
        let copy = tempfile::tempdir().unwrap();
        copy_tree(seed.path(), copy.path()).unwrap();
        assert_eq!(seed_digest(copy.path()).unwrap(), digest, "an identical copy");
        write(&copy.path().join("skills/a.md"), "alphb");
        assert_ne!(seed_digest(copy.path()).unwrap(), digest, "one byte of a file");
        write(&copy.path().join("skills/a.md"), "alpha");
        std::fs::remove_file(copy.path().join("link")).unwrap();
        std::os::unix::fs::symlink("skills/../skills", copy.path().join("link")).unwrap();
        assert_ne!(seed_digest(copy.path()).unwrap(), digest, "a link's target");
        std::fs::rename(copy.path().join("settings.json"), copy.path().join("setting.json")).unwrap();
        std::fs::remove_file(copy.path().join("link")).unwrap();
        std::os::unix::fs::symlink("skills", copy.path().join("link")).unwrap();
        assert_ne!(seed_digest(copy.path()).unwrap(), digest, "a file's name");

        // The spawner reports a kept conversation made under another seed as not the same seed.
        let state = tempfile::tempdir().unwrap();
        transcript(state.path(), previous());
        let until = at_start() + ChronoDuration::seconds(900);
        spawner_over(state.path(), state.path(), Some(seed.path())).keep(Some((previous(), until))).unwrap();
        assert!(spawner_over(state.path(), state.path(), Some(seed.path())).kept().unwrap().same_seed);
        assert!(!spawner_over(state.path(), state.path(), Some(copy.path())).kept().unwrap().same_seed);
        assert!(!spawner_over(state.path(), state.path(), None).kept().unwrap().same_seed);
    }
}
