//! The refresh against a fake vector store, a fake embedder and real git: a
//! working repo on disk stands in for the forge, and each test commits to it as
//! history would.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use huginn_mind::eureka_pipeline::{Line, Short};
use huginn_mind::SourceQuery;

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
}

impl FakeForge {
    fn of(repos: &[(&str, &Remote)]) -> Self {
        Self {
            repos: repos
                .iter()
                .map(|(name, remote)| (SourceRepo { name: (*name).into(), default_branch: "main".into() }, remote.path()))
                .collect(),
        }
    }
}

impl Forge for FakeForge {
    fn catalog(&mut self) -> Result<Vec<SourceRepo>> {
        Ok(self.repos.iter().map(|(repo, _)| repo.clone()).collect())
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

/// An upsert that fails mid-repo leaves the cursor on the old commit, and the
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
    assert_eq!(rig.index.cursors().get("Demo"), Some(&a), "the cursor never leads the points");

    rig.index.state.lock().unwrap().fail_upserts_after = None;
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

fn query(text: &str, top_k: u32, repo: Option<&str>) -> SourceQuery {
    SourceQuery { text: Line(text.into()), top_k, repo: repo.map(|repo| Short(repo.into())) }
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
