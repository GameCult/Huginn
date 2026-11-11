//! The refresh against a fake vector store, a fake embedder and real git: a
//! working repo on disk stands in for the forge, and each test commits to it as
//! history would.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use huginn_mind::eureka_pipeline::{Line, Short};
use huginn_mind::{ChatHits, ChatQuery, SourceQuery};

use super::*;
use crate::index::fakes::{FakeEmbedder, FakeSources};

fn git_in(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A working repo on disk, committed to by the test.
struct Remote {
    dir: tempfile::TempDir,
}

impl Remote {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "--quiet", "-b", "main"]);
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    /// Writes files, removes paths, commits everything and returns the commit.
    fn commit(&self, write: &[(&str, &[u8])], remove: &[&str]) -> String {
        for (path, bytes) in write {
            let full = self.dir.path().join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, bytes).unwrap();
        }
        for path in remove {
            std::fs::remove_file(self.dir.path().join(path)).unwrap();
        }
        git_in(self.dir.path(), &["add", "-A"]);
        git_in(self.dir.path(), &["commit", "--quiet", "--allow-empty", "-m", "c"]);
        git_in(self.dir.path(), &["rev-parse", "HEAD"])
    }
}

struct FakeForge {
    repos: Vec<(SourceRepo, PathBuf)>,
    shortfall: Option<String>,
}

impl FakeForge {
    fn of(repos: &[(&str, &Remote)]) -> Self {
        Self {
            repos: repos
                .iter()
                .map(|(name, remote)| (SourceRepo { name: (*name).into(), default_branch: "main".into() }, remote.path()))
                .collect(),
            shortfall: None,
        }
    }
}

impl Forge for FakeForge {
    fn catalog(&mut self) -> Result<Catalog> {
        Ok(Catalog { repos: self.repos.iter().map(|(repo, _)| repo.clone()).collect(), shortfall: self.shortfall.clone() })
    }

    fn clone_url(&self, name: &str) -> String {
        self.repos.iter().find(|(repo, _)| repo.name == name).map_or_else(|| "/nowhere".into(), |(_, path)| path.display().to_string())
    }
}

struct Rig {
    root: tempfile::TempDir,
    embedder: FakeEmbedder,
    index: FakeSources,
}

impl Rig {
    fn new() -> Self {
        Self { root: tempfile::tempdir().unwrap(), embedder: FakeEmbedder::new("d1"), index: FakeSources::default() }
    }

    fn run(&mut self, forge: &mut FakeForge) -> (Result<Refreshed>, Vec<Report>) {
        let mut reports = Vec::new();
        let refreshed = refresh(&mut self.embedder, &mut self.index, forge, self.root.path(), &mut |report| reports.push(report.clone()));
        (refreshed, reports)
    }

    fn converged(&mut self, forge: &mut FakeForge) -> Vec<Report> {
        let (refreshed, reports) = self.run(forge);
        assert!(refreshed.unwrap().failed.is_empty());
        reports
    }

    fn mirror(&self, name: &str) -> PathBuf {
        Mirror::path_for(&mirrors_dir(self.root.path()), name)
    }
}

fn held(repo: &str, path: &str, chunk: u32, commit: &str) -> (String, String, u32, String) {
    (repo.into(), path.into(), chunk, commit.into())
}

/// An empty collection plus a repo at a commit ends with that commit's chunks
/// and the cursor on it.
#[test]
fn first_build_indexes_head() {
    let remote = Remote::new();
    let a = remote.commit(&[("src/a.rs", b"fn a() {}\nfn b() {}\n"), ("README.md", b"# Demo\n")], &[]);
    let mut rig = Rig::new();
    let reports = rig.converged(&mut FakeForge::of(&[("Demo", &remote)]));
    assert_eq!(
        rig.index.held(),
        [held("Demo", "README.md", 0, &a), held("Demo", "src/a.rs", 0, &a)],
        "one chunk per small file, at the head commit"
    );
    assert_eq!(rig.index.cursors().get("Demo"), Some(&a));
    assert_eq!((reports.len(), reports[0].files, reports[0].chunks, reports[0].old.clone()), (1, 2, 2, None));
    let point = rig.index.state.lock().unwrap().points.values().find(|point| point.payload.path == "src/a.rs").unwrap().clone();
    assert_eq!((point.payload.line_start, point.payload.line_end, point.payload.ext.as_str()), (1, 2, "rs"));
    assert_eq!(point.payload.text, "fn a() {}\nfn b() {}", "the payload text is the chunk alone");
    assert!(rig.embedder.texts().contains(&"Demo/src/a.rs\nfn a() {}\nfn b() {}".to_owned()), "the embedded text leads with repo and path");
    assert!(reports[0].to_string().starts_with("Demo -..") && reports[0].to_string().contains("files=2 chunks=2"));
}

/// From A to B with one file edited and one deleted, only those two paths'
/// points change: untouched files keep their point ids, and the cursor moves.
#[test]
fn refresh_replaces_only_changed_paths() {
    let remote = Remote::new();
    let a = remote.commit(&[("a.rs", b"one\n"), ("b.rs", b"two\n"), ("c.rs", b"three\n")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);
    let ids_before = rig.index.ids();
    let untouched: Vec<String> = {
        let state = rig.index.state.lock().unwrap();
        state.points.iter().filter(|(_, point)| point.payload.path == "c.rs").map(|(id, _)| id.clone()).collect()
    };
    let embedded_before = rig.embedder.texts().len();

    let b = remote.commit(&[("a.rs", b"one edited\n")], &["b.rs"]);
    let reports = rig.converged(&mut forge);

    assert_eq!(rig.index.held(), [held("Demo", "a.rs", 0, &b), held("Demo", "c.rs", 0, &a)]);
    assert!(untouched.iter().all(|id| rig.index.ids().contains(id)), "c.rs's point id survived");
    assert!(ids_before.len() == 3 && rig.index.ids().len() == 2);
    assert_eq!(rig.index.cursors().get("Demo"), Some(&b));
    assert_eq!(rig.embedder.texts().len() - embedded_before, 1, "only the edited file is embedded again");
    assert_eq!((reports[0].old.clone(), reports[0].files), (Some(a), 1));
}

/// A head equal to the cursor is the whole refresh: nothing is embedded,
/// written, deleted or advanced.
#[test]
fn unchanged_repo_makes_no_embed_call() {
    let remote = Remote::new();
    remote.commit(&[("a.rs", b"one\n")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);
    let (embeds, upserts, deletes, cursor_writes) = {
        let state = rig.index.state.lock().unwrap();
        (rig.embedder.state.lock().unwrap().embedded.len(), state.upsert_calls, state.deletes.len(), state.cursor_writes.len())
    };
    let reports = rig.converged(&mut forge);
    let state = rig.index.state.lock().unwrap();
    assert_eq!(rig.embedder.state.lock().unwrap().embedded.len(), embeds, "no embed call");
    assert_eq!((state.upsert_calls, state.deletes.len(), state.cursor_writes.len()), (upserts, deletes, cursor_writes));
    assert_eq!((reports[0].files, reports[0].chunks), (0, 0));
}

/// An upsert that fails mid-repo leaves no cursor (no commit is held whole), and the
/// next run converges to the points a clean run of the same history makes.
#[test]
fn cursor_never_leads() {
    let remote = Remote::new();
    let many: Vec<(String, Vec<u8>)> = (0..40).map(|n| (format!("f{n}.rs"), format!("file {n}\n").into_bytes())).collect();
    let files: Vec<(&str, &[u8])> = many.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    let a = remote.commit(&files, &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);

    let edited: Vec<(String, Vec<u8>)> = (0..40).map(|n| (format!("f{n}.rs"), format!("file {n} edited\n").into_bytes())).collect();
    let files: Vec<(&str, &[u8])> = edited.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    let b = remote.commit(&files, &[]);
    {
        let mut state = rig.index.state.lock().unwrap();
        state.fail_upserts_after = Some(state.upsert_calls + 1);
    }
    let (refreshed, _) = rig.run(&mut forge);
    assert_eq!(refreshed.unwrap().failed, ["Demo"], "one repo failed, the run finished");
    assert_eq!(rig.index.cursors().get("Demo"), None, "no commit's chunks are held whole, so no cursor says one is");
    assert_ne!(a, b);

    rig.index.state.lock().unwrap().fail_upserts_after = None;
    rig.converged(&mut forge);
    assert_eq!(rig.index.cursors().get("Demo"), Some(&b));
    let mut clean = Rig::new();
    clean.converged(&mut forge);
    assert_eq!(rig.index.held(), clean.index.held());
    assert_eq!(rig.index.ids(), clean.index.ids());
}

/// A run that fails part way through a repo's upserts leaves chunks of two
/// commits. A head that then reverts the files to the old content makes an
/// empty diff from the old commit, which would keep the half-written new chunks;
/// the repo is ingested whole instead and only the head's chunks remain.
#[test]
fn a_failed_run_then_a_revert_converges() {
    let remote = Remote::new();
    let at_a: Vec<(String, Vec<u8>)> = (0..40).map(|n| (format!("f{n}.rs"), format!("file {n}
").into_bytes())).collect();
    let original: Vec<(&str, &[u8])> = at_a.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    remote.commit(&original, &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);

    let at_b: Vec<(String, Vec<u8>)> = (0..40).map(|n| (format!("f{n}.rs"), format!("file {n} edited
").into_bytes())).collect();
    let edited: Vec<(&str, &[u8])> = at_b.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    remote.commit(&edited, &[]);
    {
        let mut state = rig.index.state.lock().unwrap();
        state.fail_upserts_after = Some(state.upsert_calls + 1);
    }
    let (refreshed, _) = rig.run(&mut forge);
    assert_eq!(refreshed.unwrap().failed, ["Demo"]);
    let commits: BTreeSet<String> = rig.index.held().into_iter().map(|(.., commit)| commit).collect();
    assert_eq!(commits.len(), 2, "the failed run left chunks of two commits");

    rig.index.state.lock().unwrap().fail_upserts_after = None;
    let c = remote.commit(&original, &[]);
    rig.converged(&mut forge);
    assert_eq!(rig.index.cursors().get("Demo"), Some(&c));
    let mut clean = Rig::new();
    clean.converged(&mut forge);
    assert_eq!(rig.index.held().len(), 40, "every file of the reverted head is held");
    assert_eq!(rig.index.held(), clean.index.held());
    assert_eq!(rig.index.ids(), clean.index.ids());
}

/// While embedding fails and the identity read still answers (every GPU dark,
/// the CPU too slow), a pushed repo keeps every chunk it held, run after run,
/// and the first run after the outage converges to a clean ingest.
#[test]
fn an_embed_outage_deletes_nothing_the_index_held() {
    let remote = Remote::new();
    remote.commit(&[("a.rs", b"one
"), ("b.rs", b"two
"), ("c.rs", b"three
")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);
    let before = rig.index.held();
    let ids_before = rig.index.ids();

    let b = remote.commit(&[("a.rs", b"one edited
")], &["b.rs"]);
    rig.embedder.state.lock().unwrap().embed_fails = true;
    for _ in 0..2 {
        let (refreshed, _) = rig.run(&mut forge);
        assert_eq!(refreshed.unwrap().failed, ["Demo"]);
        assert_eq!(rig.index.held(), before, "a failed embed left the index as it was");
        assert_eq!(rig.index.ids(), ids_before);
    }

    rig.embedder.state.lock().unwrap().embed_fails = false;
    rig.converged(&mut forge);
    assert_eq!(rig.index.cursors().get("Demo"), Some(&b));
    let mut clean = Rig::new();
    clean.converged(&mut forge);
    assert_eq!(rig.index.held(), clean.index.held());
    assert_eq!(rig.index.ids(), clean.index.ids());
}

/// One repo failing is logged and the others go on.
#[test]
fn one_repos_failure_does_not_stop_the_others() {
    let good = Remote::new();
    let g = good.commit(&[("a.rs", b"one\n")], &[]);
    let nowhere = Remote::new();
    nowhere.commit(&[("a.rs", b"x\n")], &[]);
    let mut forge = FakeForge::of(&[("Broken", &nowhere), ("Good", &good)]);
    forge.repos[0].1 = PathBuf::from("/nowhere/at/all");
    let mut rig = Rig::new();
    let (refreshed, reports) = rig.run(&mut forge);
    assert_eq!(refreshed.unwrap().failed, ["Broken"]);
    assert_eq!(rig.index.held(), [held("Good", "a.rs", 0, &g)]);
    assert_eq!(reports.len(), 1);
}

/// A history that no longer contains the cursor starts the repo over: the
/// old commit's files that the new history lacks do not linger.
#[test]
fn a_rewritten_history_replaces_the_whole_repo() {
    let remote = Remote::new();
    remote.commit(&[("keep.rs", b"keep\n")], &[]);
    let old = remote.commit(&[("stale.rs", b"stale\n")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);
    git_in(&remote.path(), &["reset", "--quiet", "--hard", "HEAD~1"]);
    let new = remote.commit(&[("other.rs", b"other\n")], &[]);
    rig.converged(&mut forge);
    assert_ne!(old, new);
    assert_eq!(rig.index.cursors().get("Demo"), Some(&new));
    let paths: Vec<String> = rig.index.held().into_iter().map(|(_, path, _, _)| path).collect();
    assert_eq!(paths, ["keep.rs", "other.rs"], "stale.rs was in the old history only");
}

/// A repo the forge stops listing leaves no points, no cursor key and no
/// mirror; the one still listed is untouched.
#[test]
fn retired_repo_leaves_nothing() {
    let (keep, gone) = (Remote::new(), Remote::new());
    let k = keep.commit(&[("a.rs", b"a\n")], &[]);
    gone.commit(&[("b.rs", b"b\n")], &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("Keep", &keep), ("Gone", &gone)]));
    assert!(rig.mirror("Gone").exists() && rig.index.cursors().contains_key("Gone"));
    rig.converged(&mut FakeForge::of(&[("Keep", &keep)]));
    assert_eq!(rig.index.held(), [held("Keep", "a.rs", 0, &k)]);
    assert_eq!(rig.index.cursors().keys().collect::<Vec<_>>(), ["Keep"]);
    assert!(!rig.mirror("Gone").exists() && rig.mirror("Keep").exists());
}

/// An empty catalog, and a catalog the forge says is short, retire nothing and
/// the run says why; the same forge answering whole retires what it dropped.
#[test]
fn an_empty_or_short_catalog_retires_nothing() {
    let (keep, gone) = (Remote::new(), Remote::new());
    let k = keep.commit(&[("a.rs", b"a\n")], &[]);
    gone.commit(&[("b.rs", b"b\n")], &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("Keep", &keep), ("Gone", &gone)]));
    let both = rig.index.held();

    let mut empty = FakeForge::of(&[]);
    let (refreshed, reports) = rig.run(&mut empty);
    let refreshed = refreshed.unwrap();
    assert!(reports.is_empty() && refreshed.retirement_withheld.as_deref().unwrap().contains("no indexable repos"));
    assert_eq!(rig.index.held(), both, "an empty answer deleted nothing");
    assert!(rig.mirror("Gone").exists() && rig.mirror("Keep").exists());
    assert_eq!(rig.index.cursors().len(), 2);

    let mut short = FakeForge::of(&[("Keep", &keep)]);
    short.shortfall = Some("the forge counts 2 repos and 1 were listed".into());
    let (refreshed, _) = rig.run(&mut short);
    assert!(refreshed.unwrap().retirement_withheld.unwrap().contains("incomplete"));
    assert_eq!(rig.index.held(), both, "a short answer deleted nothing");
    assert!(rig.mirror("Gone").exists());

    let (refreshed, _) = rig.run(&mut FakeForge::of(&[("Keep", &keep)]));
    assert_eq!(refreshed.unwrap().retirement_withheld, None);
    assert_eq!(rig.index.held(), [held("Keep", "a.rs", 0, &k)], "a whole answer retires what the forge dropped");
}

/// A retirement that fails is that repo's failure alone: it is named, the
/// other retirements and the live repos go on, and the failed one keeps its
/// cursor and mirror so the next run finds it again.
#[test]
fn a_retire_error_skips_that_repo_and_the_run_goes_on() {
    let (keep, first, second) = (Remote::new(), Remote::new(), Remote::new());
    let k = keep.commit(&[("a.rs", b"a\n")], &[]);
    first.commit(&[("b.rs", b"b\n")], &[]);
    second.commit(&[("c.rs", b"c\n")], &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("Keep", &keep), ("First", &first), ("Second", &second)]));
    let edited = keep.commit(&[("a.rs", b"a edited\n")], &[]);
    rig.index.state.lock().unwrap().fail_deletes_of = Some("First".into());

    let (refreshed, reports) = rig.run(&mut FakeForge::of(&[("Keep", &keep)]));
    assert_eq!(refreshed.unwrap().failed, ["First"]);
    assert!(!rig.mirror("Second").exists() && rig.mirror("First").exists());
    assert_eq!(rig.index.cursors().into_keys().collect::<Vec<_>>(), ["First", "Keep"]);
    assert_eq!(reports.len(), 1, "the live repo was refreshed after the failed retirement");
    assert_ne!(k, edited);

    rig.index.state.lock().unwrap().fail_deletes_of = None;
    rig.converged(&mut FakeForge::of(&[("Keep", &keep)]));
    assert!(!rig.mirror("First").exists());
    assert_eq!(rig.index.cursors().into_keys().collect::<Vec<_>>(), ["Keep"]);
    assert_eq!(rig.index.held(), [held("Keep", "a.rs", 0, &edited)]);
}

/// A repo the forge now spells with another case is another repo to the
/// index: the points under the old spelling do not stay behind.
#[test]
fn a_repo_renamed_only_by_case_keeps_no_stale_points() {
    let remote = Remote::new();
    remote.commit(&[("a.rs", b"a\n")], &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("Demo", &remote)]));
    let head = remote.commit(&[("b.rs", b"b\n")], &[]);
    rig.converged(&mut FakeForge::of(&[("demo", &remote)]));
    let repos: BTreeSet<String> = rig.index.held().into_iter().map(|(repo, _, _, _)| repo).collect();
    assert_eq!(repos, BTreeSet::from(["demo".to_owned()]));
    assert_eq!(rig.index.cursors().into_iter().collect::<Vec<_>>(), [("demo".to_owned(), head)]);
}

/// A run is clean only when no repo failed and retirement was not withheld.
#[test]
fn a_run_is_clean_only_when_nothing_failed_and_retirement_was_not_withheld() {
    assert!(Refreshed::default().is_clean());
    assert!(!Refreshed { failed: vec!["Demo".into()], retirement_withheld: None }.is_clean());
    assert!(!Refreshed { failed: Vec::new(), retirement_withheld: Some("short".into()) }.is_clean());
}

/// A mirror no cursor knows (a first ingest that never finished) is retired
/// with its points too.
#[test]
fn a_retired_repo_with_no_cursor_is_still_cleaned() {
    let keep = Remote::new();
    keep.commit(&[("a.rs", b"a\n")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Keep", &keep)]);
    rig.converged(&mut forge);
    std::fs::create_dir_all(rig.mirror("Orphan")).unwrap();
    rig.converged(&mut forge);
    assert!(!rig.mirror("Orphan").exists());
    assert!(rig.index.state.lock().unwrap().deletes.iter().any(|(repo, paths)| repo == "Orphan" && paths.is_none()));
}

/// Another digest or chunker version recreates the collection and refills it;
/// an unlabelled collection is refused untouched.
#[test]
fn rebuild_on_model_or_chunker_change() {
    let remote = Remote::new();
    let a = remote.commit(&[("a.rs", b"one\n")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);
    assert_eq!(rig.index.state.lock().unwrap().recreated, 1, "created once, empty");
    rig.converged(&mut forge);
    assert_eq!(rig.index.state.lock().unwrap().recreated, 1, "same label, no rebuild");

    rig.embedder = FakeEmbedder::new("d2");
    rig.converged(&mut forge);
    assert_eq!(rig.index.state.lock().unwrap().recreated, 2, "another digest rebuilds");
    assert_eq!(rig.index.held(), [held("Demo", "a.rs", 0, &a)], "and refills from the cursorless collection");

    let mut old_chunker = match rig.index.state.lock().unwrap().described.clone() {
        Some(DescribedSources::Labelled { meta, cursors }) => (meta, cursors),
        other => panic!("{other:?}"),
    };
    old_chunker.0.chunker_version = CHUNKER_VERSION - 1;
    rig.index.plant(DescribedSources::Labelled { meta: old_chunker.0, cursors: old_chunker.1 });
    rig.converged(&mut forge);
    assert_eq!(rig.index.state.lock().unwrap().recreated, 3, "another chunker version rebuilds");

    rig.index.plant(DescribedSources::Unlabelled);
    let before = rig.index.held();
    let (refreshed, _) = rig.run(&mut forge);
    assert!(format!("{:#}", refreshed.unwrap_err()).contains("no Huginn metadata"));
    let recreated = rig.index.state.lock().unwrap().recreated;
    assert_eq!((recreated, rig.index.held()), (3, before), "untouched");
}

/// A NUL blob, an LFS pointer, an oversize file, Unity YAML and a lock file
/// yield no chunks; the plain file beside them does.
#[test]
fn skips_binary_lfs_oversize() {
    let remote = Remote::new();
    let huge = vec![b'x'; 300 * 1024];
    let big_but_ok = vec![b'y'; 100 * 1024];
    let c = remote.commit(
        &[
            ("nul.txt", b"abc\0def\n"),
            ("lfs.txt", b"version https://git-lfs.github.com/spec/v1\noid sha256:aa\nsize 9\n"),
            ("huge.txt", &huge),
            ("scene.prefab", b"%YAML 1.1\n"),
            ("Cargo.lock", b"[[package]]\n"),
            ("ok.txt", b"hello\n"),
            ("big.txt", &big_but_ok),
        ],
        &[],
    );
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("Demo", &remote)]));
    let paths: Vec<String> = rig.index.held().into_iter().map(|(_, path, _, _)| path).collect();
    assert_eq!(paths, ["big.txt", "ok.txt"], "100 KiB is under the 256 KiB bound; one long line is one chunk");
    assert_eq!(rig.index.cursors().get("Demo"), Some(&c));
}

fn front_query(text: &str, top_k: u32, repo: Option<&str>) -> SourceQuery {
    SourceQuery { front: true, ..query(text, top_k, repo) }
}

fn query(text: &str, top_k: u32, repo: Option<&str>) -> SourceQuery {
    SourceQuery { text: Line(text.into()), texts: vec![], top_k, repo: repo.map(|repo| Short(repo.into())), front: false }
}

/// A daemon whose embedder is not the one the collection was built under
/// refuses and names both identities; a missing collection says it is not built.
#[test]
fn source_search_refuses_foreign_model() {
    let remote = Remote::new();
    remote.commit(&[("a.rs", b"one\n")], &[]);
    let mut rig = Rig::new();
    let within = Duration::from_secs(5);
    let error = search(&mut FakeEmbedder::new("d1"), &mut rig.index, &query("one", 3, None), within).unwrap_err();
    assert_eq!(format!("{error:#}"), "source index not built");

    rig.converged(&mut FakeForge::of(&[("Demo", &remote)]));
    let mut foreign = FakeEmbedder::new("d2");
    let error = format!("{:#}", search(&mut foreign, &mut rig.index, &query("one", 3, None), within).unwrap_err());
    assert!(error.contains("built under model d1") && error.contains("embeds under model d2"), "{error}");
    assert!(foreign.state.lock().unwrap().embedded.is_empty(), "nothing was embedded for a foreign model");

    foreign.state.lock().unwrap().identity.dimensions = 8;
    let error = format!("{:#}", search(&mut FakeEmbedder::new("d1"), &mut rig.index, &query("one", 3, None), within).map(|_| ()).err().map_or(String::new(), |e| format!("{e:#}")));
    assert_eq!(error, "", "the same model still answers");
}

/// A search answers the chunks as the collection holds them, nearest first,
/// within the repo asked for and no more than `top_k`; the query carries the
/// instruction and not the path prefix.
#[test]
fn a_source_search_answers_held_chunks_within_a_repo_and_a_limit() {
    let (one, two) = (Remote::new(), Remote::new());
    let c1 = one.commit(&[("a.rs", b"alpha\n"), ("b.rs", b"beta\nbeta\n")], &[]);
    two.commit(&[("z.rs", b"zeta\n")], &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("One", &one), ("Two", &two)]));
    let within = Duration::from_secs(5);

    let all = search(&mut rig.embedder, &mut rig.index, &query("alpha", 10, None), within).unwrap();
    assert_eq!(all.hits.len(), 3);
    assert_eq!((all.indexed_under.name.as_str(), all.indexed_under.digest.as_str()), ("model", "d1"));
    assert!(all.hits.windows(2).all(|pair| pair[0].score >= pair[1].score), "nearest first");

    let in_one = search(&mut rig.embedder, &mut rig.index, &query("alpha", 10, Some("One")), within).unwrap();
    assert_eq!(in_one.hits.len(), 2);
    assert!(in_one.hits.iter().all(|hit| hit.repo == "One" && hit.commit == c1));
    let b = in_one.hits.iter().find(|hit| hit.path == "b.rs").unwrap();
    assert_eq!((b.line_start, b.line_end, b.text.as_str()), (1, 2, "beta\nbeta"));

    let limited = search(&mut rig.embedder, &mut rig.index, &query("alpha", 1, None), within).unwrap();
    assert_eq!(limited.hits.len(), 1);
    let asked = rig.embedder.texts().last().cloned().unwrap();
    assert_eq!(asked, format!("Instruct: {SOURCE_QUERY_INSTRUCTION}\nQuery: alpha"));
}

/// A front search answers only the front-page files at a repo's root, in every
/// repo or in the one asked for; a nested README or a look-alike is not one.
#[test]
fn a_front_search_answers_only_root_front_pages_within_the_repo_asked() {
    let (one, two) = (Remote::new(), Remote::new());
    one.commit(&[("README.md", b"one front\n"), ("docs/README.md", b"nested\n"), ("README.md.bak", b"bak\n"), ("a.rs", b"code\n"), ("AGENTS.md", b"agents\n")], &[]);
    two.commit(&[("CLAUDE.md", b"two front\n"), ("src/lib.rs", b"lib\n")], &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("One", &one), ("Two", &two)]));
    let within = Duration::from_secs(5);
    let mut held = |query: &SourceQuery| {
        let mut found: Vec<(String, String)> =
            search(&mut rig.embedder, &mut rig.index, query, within).unwrap().hits.into_iter().map(|hit| (hit.repo, hit.path)).collect();
        found.sort();
        found
    };
    let pair = |repo: &str, path: &str| (repo.to_owned(), path.to_owned());
    assert_eq!(held(&front_query("front", 50, None)), [pair("One", "AGENTS.md"), pair("One", "README.md"), pair("Two", "CLAUDE.md")]);
    assert_eq!(held(&front_query("front", 50, Some("Two"))), [pair("Two", "CLAUDE.md")]);
    assert_eq!(held(&query("front", 50, Some("Two"))).len(), 2, "without front, the repo's other files answer too");
}

/// Chunk ids are the content address: the same blob chunked the same way is
/// the same point, and any part of the address changes it.
#[test]
fn a_chunk_id_is_its_content_address() {
    let base = chunk_id("R", "p", "blob", 0);
    assert_eq!(base, chunk_id("R", "p", "blob", 0));
    for other in [chunk_id("S", "p", "blob", 0), chunk_id("R", "q", "blob", 0), chunk_id("R", "p", "blob2", 0), chunk_id("R", "p", "blob", 1)] {
        assert_ne!(base, other);
    }
    assert_ne!(chunk_id("R", "pb", "lob", 0), chunk_id("R", "p", "blob", 0), "fields are separated, not concatenated");
}

/// Chunks are embedded in batches of at most `BATCH`, however many a repo has.
#[test]
fn chunks_are_embedded_in_bounded_batches() {
    let remote = Remote::new();
    let many: Vec<(String, Vec<u8>)> = (0..70).map(|n| (format!("f{n}.txt"), format!("file {n}\n").into_bytes())).collect();
    let files: Vec<(&str, &[u8])> = many.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    remote.commit(&files, &[]);
    let mut rig = Rig::new();
    rig.converged(&mut FakeForge::of(&[("Demo", &remote)]));
    let sizes: Vec<usize> = rig.embedder.state.lock().unwrap().embedded.iter().map(Vec::len).collect();
    assert_eq!(sizes, [BATCH, BATCH, 6]);
    assert_eq!(rig.index.held().len(), 70);
}

fn fused(lists: &[&[&str]], top_k: usize) -> Vec<(f32, String)> {
    fuse(lists.iter().map(|list| list.iter().map(|item| (*item).to_owned()).collect()).collect(), |item| item.clone(), FUSE_K, top_k)
}

/// A chunk that two lists put first outranks one that a single list puts first:
/// 1/61 + 1/61 against 1/61, with k = 60 and ranks counted from 1.
#[test]
fn a_chunk_first_in_two_lists_outranks_a_chunk_first_in_one() {
    let found = fused(&[&["both", "x"], &["both", "y"]], 10);
    let names: Vec<&str> = found.iter().map(|(_, item)| item.as_str()).collect();
    assert_eq!(names[0], "both");
    assert!((found[0].0 - 2.0 / 61.0).abs() < 1e-6, "{}", found[0].0);
    assert!((found[1].0 - 1.0 / 62.0).abs() < 1e-6, "second place in one list is 1/(60+2)");
    assert_eq!(names, ["both", "x", "y"], "a tie keeps the earlier list's item ahead");
    let deeper = fused(&[&["a", "b", "c"], &["c"]], 10);
    assert_eq!(deeper[0].1, "c", "1/63 + 1/61 beats 1/61");
}

/// The fused list is cut to `top_k`, and an empty input fuses to nothing.
#[test]
fn a_fused_list_is_cut_to_top_k() {
    assert_eq!(fused(&[&["a", "b", "c"]], 2).len(), 2);
    assert!(fused(&[], 5).is_empty());
}

/// An embedder whose vector is the axis of the topic a text names (`topic-N`
/// anywhere in it), so a test says which chunk each text is near.
struct TopicEmbedder(FakeEmbedder);

impl Embedder for TopicEmbedder {
    fn model_identity(&mut self, within: Duration) -> Result<crate::index::ModelIdentity> {
        self.0.model_identity(within)
    }

    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>> {
        self.0.embed(texts, within)?;
        let dimensions = self.0.state.lock().unwrap().identity.dimensions as usize;
        Ok(texts
            .iter()
            .map(|text| {
                let axis: usize = text.split("topic-").nth(1).and_then(|rest| rest.get(..1)).and_then(|digit| digit.parse().ok()).expect("a text names topic-N");
                (0..dimensions).map(|at| if at == axis { 1.0 } else { 0.0 }).collect()
            })
            .collect())
    }
}

/// A rig whose collection holds one chunk per axis, `chunk-N` near `topic-N`,
/// built under an embedder of `dimensions`.
fn topic_rig(dimensions: u32) -> (Rig, TopicEmbedder) {
    let remote = Remote::new();
    remote.commit(&[("seed.rs", b"seed\n")], &[]);
    let mut rig = Rig::new();
    rig.embedder.state.lock().unwrap().identity.dimensions = dimensions;
    rig.converged(&mut FakeForge::of(&[("Demo", &remote)]));
    {
        let mut state = rig.index.state.lock().unwrap();
        state.points.clear();
        for axis in 0..dimensions as usize {
            let payload = SourcePayload {
                repo: "Demo".into(),
                path: format!("chunk-{axis}.rs"),
                commit: "c".into(),
                blob: format!("b{axis}"),
                line_start: 1,
                line_end: 1,
                chunk_index: 0,
                ext: "rs".into(),
                text: format!("chunk {axis}"),
            };
            let id = chunk_id(&payload.repo, &payload.path, &payload.blob, 0);
            state.points.insert(id.clone(), SourcePoint { id, vector: (0..dimensions as usize).map(|at| if at == axis { 1.0 } else { 0.0 }).collect(), payload });
        }
        state.searches.clear();
    }
    rig.embedder.state.lock().unwrap().embedded.clear();
    let embedder = TopicEmbedder(rig.embedder.clone());
    (rig, embedder)
}

fn queries(texts: &[&str], top_k: u32) -> SourceQuery {
    SourceQuery { text: Line::default(), texts: texts.iter().map(|text| Line((*text).into())).collect(), top_k, repo: None, front: false }
}

fn paths(hits: &SourceHits) -> Vec<&str> {
    hits.hits.iter().map(|hit| hit.path.as_str()).collect()
}

/// One text answers exactly as it did before texts existed, whether it came in
/// `text` or in `texts`: the index's own cosine scores, unfused, top_k deep.
#[test]
fn one_text_answers_exactly_as_before() {
    let (mut rig, mut embedder) = topic_rig(4);
    let within = Duration::from_secs(5);
    let old = search(&mut embedder, &mut rig.index, &query("topic-2", 3, None), within).unwrap();
    let new = search(&mut embedder, &mut rig.index, &queries(&["topic-2"], 3), within).unwrap();
    assert_eq!(old, new);
    assert_eq!(old.hits.len(), 3);
    assert_eq!((old.hits[0].path.as_str(), old.hits[0].score), ("chunk-2.rs", 1.0), "the cosine, not a reciprocal-rank sum");
    assert!(old.hits[1..].iter().all(|hit| hit.score == 0.0));
    let searches = rig.index.state.lock().unwrap().searches.clone();
    assert_eq!(searches, [(None, 3), (None, 3)], "unfused, so asked for top_k and no more");
}

/// Two texts that are equal once trimmed are one text: one search, and the
/// answer of the single text.
#[test]
fn a_duplicated_text_does_not_double_count() {
    let (mut rig, mut embedder) = topic_rig(4);
    let within = Duration::from_secs(5);
    let single = search(&mut embedder, &mut rig.index, &queries(&["topic-1"], 4), within).unwrap();
    rig.index.state.lock().unwrap().searches.clear();
    let twice = search(&mut embedder, &mut rig.index, &queries(&["topic-1", "  topic-1\n"], 4), within).unwrap();
    assert_eq!(twice, single);
    assert_eq!(rig.index.state.lock().unwrap().searches.len(), 1);
}

/// n distinct texts are one embed call of n prompts and n searches, each
/// `FUSE_DEPTH` deep; the answer is fused and cut to top_k.
#[test]
fn n_texts_make_n_searches_and_one_embed_call() {
    let (mut rig, mut embedder) = topic_rig(4);
    let within = Duration::from_secs(5);
    let found = search(&mut embedder, &mut rig.index, &queries(&["topic-0", "topic-1", "topic-3"], 2), within).unwrap();
    let embedded = rig.embedder.state.lock().unwrap().embedded.clone();
    assert_eq!(embedded.len(), 1, "one embed call");
    assert_eq!(embedded[0].len(), 3);
    assert!(embedded[0].iter().all(|prompt| prompt.starts_with("Instruct: ")));
    let searches = rig.index.state.lock().unwrap().searches.clone();
    assert_eq!(searches, vec![(None, FUSE_DEPTH); 3]);
    assert_eq!(found.hits.len(), 2);
    assert!(found.hits.iter().all(|hit| hit.score >= 1.0 / 61.0), "a reciprocal-rank sum, at least the 1/(60+1) of a first place");
    assert!(found.hits[0].score >= found.hits[1].score);
}

/// The reason the cut exists: a brief of eight 1,000-byte topics reaches the
/// chunk that only its last topic is near.
#[test]
fn a_long_brief_of_eight_topics_reaches_the_chunk_only_the_last_is_near() {
    let (mut rig, mut embedder) = topic_rig(8);
    let within = Duration::from_secs(5);
    let topics: Vec<String> = (0..8).map(|n| format!("topic-{n} {}", "x".repeat(1000 - 8))).collect();
    assert!(topics.iter().all(|topic| topic.len() == 1000));
    let refs: Vec<&str> = topics.iter().map(String::as_str).collect();
    let found = search(&mut embedder, &mut rig.index, &queries(&refs, 8), within).unwrap();
    assert!(paths(&found).contains(&"chunk-7.rs"), "{:?}", paths(&found));
    assert_eq!(paths(&found).len(), 8);
}

// ---- chat search ----

fn msg(event_id: &str, ts_ms: u64) -> ChatMessage {
    ChatMessage { event_id: event_id.into(), sender: "@a:gamecult.org".into(), ts_ms, body: format!("body of {event_id}") }
}

/// A chunk in `room` (with `alias`) of messages `(id, ts)`, near `axis`.
fn chat_point(room: &str, alias: Option<&str>, axis: usize, dimensions: u32, messages: &[(&str, u64)]) -> ChatPoint {
    let messages: Vec<ChatMessage> = messages.iter().map(|(id, ts)| msg(id, *ts)).collect();
    ChatPoint {
        id: point_id(&format!("{room}{}", messages[0].event_id)),
        vector: (0..dimensions as usize).map(|at| if at == axis { 1.0 } else { 0.0 }).collect(),
        payload: ChatPayload { room_id: room.into(), room_alias: alias.map(str::to_owned), messages, text: format!("chunk near topic-{axis}") },
    }
}

fn chat_query(texts: &[&str], rooms: &[&str], after: Option<u64>, before: Option<u64>, top_k: u32) -> ChatQuery {
    ChatQuery {
        texts: texts.iter().map(|text| Line((*text).into())).collect(),
        rooms: rooms.iter().map(|room| Short((*room).into())).collect(),
        after_ms: after,
        before_ms: before,
        top_k,
    }
}

/// A labelled collection that also holds the chat chunks given. Two rooms:
/// `!a` (alias `#swarm`), `!b` (no alias).
fn chat_rig(points: Vec<ChatPoint>) -> (Rig, TopicEmbedder) {
    let (mut rig, embedder) = topic_rig(4);
    rig.index.state.lock().unwrap().points.clear();
    rig.index.upsert_chat(&points).unwrap();
    (rig, embedder)
}

fn ids(hits: &ChatHits) -> Vec<&str> {
    hits.hits.iter().map(|hit| hit.first_event_id.as_str()).collect()
}

const WITHIN: Duration = Duration::from_secs(5);

fn three_rooms() -> Vec<ChatPoint> {
    vec![
        chat_point("!a:x", Some("#swarm:x"), 0, 4, &[("$a1", 100), ("$a2", 200)]),
        chat_point("!a:x", Some("#swarm:x"), 1, 4, &[("$a3", 300), ("$a4", 400)]),
        chat_point("!b:x", None, 0, 4, &[("$b1", 150)]),
    ]
}

#[test]
fn a_room_filter_by_id_and_by_alias_returns_only_that_room() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    let by_alias = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["#swarm:x"], None, None, 10), WITHIN).unwrap();
    assert_eq!(ids(&by_alias), ["$a1", "$a3"]);
    let by_id = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["!b:x"], None, None, 10), WITHIN).unwrap();
    assert_eq!(ids(&by_id), ["$b1"], "a room with no alias is reachable by id");
    assert_eq!(by_id.hits[0].room_alias, None);
    let both = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["#swarm:x", "!b:x"], None, None, 10), WITHIN).unwrap();
    assert_eq!(ids(&both), ["$a1", "$b1", "$a3"], "any-of, oldest first");
    let nobody = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["#nobody:x"], None, None, 10), WITHIN).unwrap();
    assert!(nobody.hits.is_empty() && !nobody.truncated, "an unknown alias is zero hits, not an error");
}

#[test]
fn a_time_span_returns_the_overlapping_chunks_and_trims_boundary_messages() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    let hits = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["#swarm:x"], Some(150), Some(350), 10), WITHIN).unwrap();
    assert_eq!(ids(&hits), ["$a2", "$a3"], "the boundary chunks keep only in-span messages and first ids are recomputed");
    let (first, second) = (&hits.hits[0], &hits.hits[1]);
    assert_eq!((first.ts_start_ms, first.ts_end_ms, first.last_event_id.as_str(), first.messages.len()), (200, 200, "$a2", 1));
    assert_eq!((second.ts_start_ms, second.ts_end_ms, second.last_event_id.as_str(), second.messages.len()), (300, 300, "$a3", 1));
    let none = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["#swarm:x"], Some(1000), None, 10), WITHIN).unwrap();
    assert!(none.hits.is_empty(), "a chunk wholly outside is absent");
}

#[test]
fn list_mode_is_oldest_first_truncated_when_more_exist_and_needs_no_embed() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    let page = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &[], None, None, 2), WITHIN).unwrap();
    assert_eq!(ids(&page), ["$a1", "$b1"]);
    assert!(page.truncated);
    assert!(embedder.0.state.lock().unwrap().embedded.is_empty(), "no texts, no embed call");
    // Within one room chunks never overlap, so the last hit's end + 1 continues without a repeat.
    let first = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["!a:x"], None, None, 1), WITHIN).unwrap();
    assert_eq!((ids(&first), first.truncated), (vec!["$a1"], true));
    let rest = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["!a:x"], Some(first.hits[0].ts_end_ms + 1), None, 1), WITHIN).unwrap();
    assert_eq!((ids(&rest), rest.truncated), (vec!["$a3"], false));
}

#[test]
fn texts_rank_by_meaning_and_compose_with_room_and_span() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    let near = search_chat(&mut embedder, &mut rig.index, &chat_query(&["topic-0"], &[], None, None, 5), WITHIN).unwrap();
    assert_eq!(near.hits.len(), 3);
    assert!(near.hits[0].score.unwrap() > near.hits[2].score.unwrap());
    assert_eq!(near.hits[2].first_event_id, "$a3", "the far chunk is last");
    let narrowed = search_chat(&mut embedder, &mut rig.index, &chat_query(&["topic-0"], &["!a:x"], Some(250), None, 5), WITHIN).unwrap();
    assert_eq!(ids(&narrowed), ["$a3"], "a near chunk in another room or outside the span never appears");
    let fused = search_chat(&mut embedder, &mut rig.index, &chat_query(&["topic-0", "topic-1"], &["!a:x"], None, None, 5), WITHIN).unwrap();
    assert_eq!(fused.hits.len(), 2);
    assert!(fused.hits[0].score.unwrap() > 0.0 && fused.hits[0].score != Some(1.0), "two texts carry the fused reciprocal-rank score");
    assert_eq!(embedder.0.state.lock().unwrap().embedded.last().unwrap().len(), 2, "one embed call for two texts");
}

#[test]
fn every_hit_cites_room_event_ids_and_timestamps() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    for texts in [&[][..], &["topic-0"][..]] {
        let hits = search_chat(&mut embedder, &mut rig.index, &chat_query(texts, &[], None, None, 10), WITHIN).unwrap();
        assert!(!hits.hits.is_empty());
        for hit in &hits.hits {
            assert!(hit.room_id.starts_with('!') && !hit.first_event_id.is_empty() && !hit.last_event_id.is_empty());
            assert!(hit.ts_start_ms <= hit.ts_end_ms && !hit.messages.is_empty());
            assert_eq!((hit.messages[0].event_id.as_str(), hit.messages.last().unwrap().event_id.as_str()), (hit.first_event_id.as_str(), hit.last_event_id.as_str()));
            for message in &hit.messages {
                assert!(message.event_id.starts_with('$') && message.sender.starts_with('@') && message.ts_ms > 0);
            }
        }
    }
}

#[test]
fn a_forge_search_never_returns_a_chat_point_and_a_chat_search_never_a_forge_point() {
    let (mut rig, mut embedder) = topic_rig(4);
    rig.index.upsert_chat(&three_rooms()).unwrap();
    let forge = search(&mut embedder, &mut rig.index, &queries(&["topic-0"], 10), WITHIN).unwrap();
    assert_eq!(forge.hits.len(), 4, "only the four forge chunks");
    let chat = search_chat(&mut embedder, &mut rig.index, &chat_query(&["topic-0"], &[], None, None, 10), WITHIN).unwrap();
    assert_eq!(chat.hits.len(), 3, "only the three chat chunks");
    let listed = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &[], None, None, 10), WITHIN).unwrap();
    assert_eq!(listed.hits.len(), 3);
}

#[test]
fn an_absent_unlabelled_or_other_model_collection_is_a_loud_refusal_for_chat() {
    let mut rig = Rig::new();
    let mut embedder = TopicEmbedder(FakeEmbedder::new("d1"));
    for texts in [&[][..], &["topic-0"][..]] {
        let error = search_chat(&mut embedder, &mut rig.index, &chat_query(texts, &[], None, None, 3), WITHIN).unwrap_err();
        assert_eq!(format!("{error:#}"), "source index not built");
    }
    rig.index.plant(DescribedSources::Unlabelled);
    assert!(search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &[], None, None, 3), WITHIN).is_err());
    let (mut rig, _) = chat_rig(three_rooms());
    let mut foreign = TopicEmbedder(FakeEmbedder::new("d2"));
    let error = format!("{:#}", search_chat(&mut foreign, &mut rig.index, &chat_query(&["topic-0"], &[], None, None, 3), WITHIN).unwrap_err());
    assert!(error.contains("built under model d1") && error.contains("embeds under model d2"), "{error}");
}

/// An old forge-only collection (labelled with the old corpus) is rebuilt once
/// under the new corpus, and a second refresh leaves it.
#[test]
fn an_old_forge_only_collection_rebuilds_once() {
    let remote = Remote::new();
    remote.commit(&[("a.rs", b"one\n")], &[]);
    let mut rig = Rig::new();
    let mut forge = FakeForge::of(&[("Demo", &remote)]);
    rig.converged(&mut forge);
    let mut old = match rig.index.state.lock().unwrap().described.clone() {
        Some(DescribedSources::Labelled { meta, cursors }) => (meta, cursors),
        other => panic!("{other:?}"),
    };
    assert_eq!(old.0.corpus, "forge:GameCult+chat:rooms");
    old.0.corpus = "forge:GameCult".into();
    rig.index.plant(DescribedSources::Labelled { meta: old.0, cursors: old.1 });
    let before = rig.index.state.lock().unwrap().recreated;
    rig.converged(&mut forge);
    rig.converged(&mut forge);
    assert_eq!(rig.index.state.lock().unwrap().recreated, before + 1);
}

#[test]
fn the_chat_read_path_holds_no_discord_matrix_or_second_fuse() {
    let source = include_str!("mod.rs");
    let read = &source[source.find("pub fn search_chat").unwrap()..];
    for banned in ["discord", "puppet", "bridge", "matrix", "synapse", "access_token"] {
        assert!(!read.to_ascii_lowercase().contains(banned), "{banned} in the chat read path");
    }
    assert_eq!(source.matches("pub fn fuse").count(), 1);
}

/// An old forge-only collection answers both searches with the stale-index
/// error until the rebuild lands, not with an empty page.
#[test]
fn an_old_corpus_collection_is_refused_until_it_is_rebuilt() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    let Some(DescribedSources::Labelled { mut meta, cursors }) = rig.index.state.lock().unwrap().described.clone() else { panic!() };
    meta.corpus = "forge:GameCult".into();
    rig.index.plant(DescribedSources::Labelled { meta, cursors });
    let error = format!("{:#}", search(&mut embedder, &mut rig.index, &query("one", 3, None), WITHIN).unwrap_err());
    assert!(error.contains("stale") && error.contains("forge:GameCult") && error.contains("rebuild is pending"), "{error}");
    for texts in [&[][..], &["topic-0"][..]] {
        let error = format!("{:#}", search_chat(&mut embedder, &mut rig.index, &chat_query(texts, &[], None, None, 3), WITHIN).unwrap_err());
        assert!(error.contains("stale") && error.contains("rebuild is pending"), "{error}");
    }
}

/// A message exactly on `after` or `before` is in the span, and so is a chunk
/// whose end is exactly `after` or whose start is exactly `before`.
#[test]
fn the_span_bounds_are_inclusive_for_chunks_and_for_messages() {
    let (mut rig, mut embedder) = chat_rig(three_rooms());
    let mut hits = |after, before| {
        let found = search_chat(&mut embedder, &mut rig.index, &chat_query(&[], &["#swarm:x"], after, before, 10), WITHIN).unwrap();
        found.hits.iter().map(|hit| (hit.first_event_id.clone(), hit.messages.len())).collect::<Vec<_>>()
    };
    let owned = |pairs: &[(&str, usize)]| pairs.iter().map(|(id, n)| ((*id).to_owned(), *n)).collect::<Vec<_>>();
    // Chunk $a1..$a2 ends at 200: after=200 keeps it, trimmed to the message on the bound.
    assert_eq!(hits(Some(200), None), owned(&[("$a2", 1), ("$a3", 2)]));
    // Chunk $a3..$a4 starts at 300: before=300 keeps it, trimmed to the message on the bound.
    assert_eq!(hits(None, Some(300)), owned(&[("$a1", 2), ("$a3", 1)]));
    // Both bounds on one message.
    assert_eq!(hits(Some(200), Some(200)), owned(&[("$a2", 1)]));

    let filter = ChatFilter { rooms: vec![], after_ms: Some(200), before_ms: Some(300) };
    assert!(filter.keeps("!a:x", None, 100, 200) && filter.keeps("!a:x", None, 300, 400));
    assert!(!filter.keeps("!a:x", None, 100, 199) && !filter.keeps("!a:x", None, 301, 400));
}
