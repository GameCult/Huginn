//! The real child, with a fake `claude` script that reads its credential from
//! the descriptor it is told of. The token here is a canary: what these tests
//! prove is where it can and cannot be found.

use super::*;

/// Reads the credential from the named descriptor, then reports what it can see.
/// It prints the credential once, so that the test can tell it arrived.
const READS_ITS_CREDENTIAL: &str = r#"
fd="$CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR"
read -r -u "$fd" credential
echo "fd=$fd"
echo "scrub=$CLAUDE_CODE_SUBPROCESS_ENV_SCRUB"
echo "argv=$*"
echo "in_env=$(env | grep -c "$credential")"
echo "got=$credential"
echo "grandchild=[$(sh -c "cat <&$fd")]"
"#;

fn field(lines: &[String], name: &str) -> String {
    lines.iter().find_map(|line| line.strip_prefix(&format!("{name}="))).unwrap_or_else(|| panic!("no {name} in {lines:?}")).to_string()
}

#[test]
fn granted_fd_reaches_child_env_var() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(&dir, READS_ITS_CREDENTIAL);
    let token = token_holding("tok-canary-7f3a");
    let mut child = start_with(&program, &claude_args(session_id(&run_ref("selfrun-a")), "5"), &token);
    child.close_input();
    let (lines, code) = drain(&mut *child);
    assert_eq!(code, Some(0), "{lines:?}");
    let number: i32 = field(&lines, "fd").parse().unwrap();
    assert!(number >= 3, "never a stdio number: {number}");
    assert_eq!(field(&lines, "scrub"), "1");
    assert_eq!(field(&lines, "got"), "tok-canary-7f3a", "the credential came through the descriptor");
    assert_eq!(field(&lines, "in_env"), "0", "the token text is in no environment variable");
    assert!(!field(&lines, "argv").contains("tok-canary"), "nor in the arguments");
}

#[test]
fn the_childs_children_inherit_an_emptied_pipe_not_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(&dir, READS_ITS_CREDENTIAL);
    let token = token_holding("tok-canary-7f3a");
    let mut child = start_with(&program, &[], &token);
    child.close_input();
    let (lines, _) = drain(&mut *child);
    assert_eq!(field(&lines, "got"), "tok-canary-7f3a");
    assert_eq!(field(&lines, "grandchild"), "[]", "a descendant that reads the descriptor after claude has gets end of file");
}

#[test]
fn a_credential_in_the_units_environment_never_reaches_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(&dir, "env > \"$0.env\"
echo done");
    let mut child = start_with(&program, &[], &token_holding("tok-canary-7f3a"));
    child.close_input();
    drain(&mut *child);
    let seen = std::fs::read_to_string(dir.path().join("claude.env")).unwrap();
    let names: Vec<&str> = seen.lines().filter_map(|line| line.split_once('=')).map(|(name, _)| name).collect();
    for canary in CREDENTIAL_CANARIES {
        assert!(!names.contains(&canary), "{canary} reached the child: {names:?}");
    }
    assert!(!seen.contains("canary-"), "no canary value either: {seen}");
    assert!(names.contains(&"PATH") && names.contains(&"CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR"), "{names:?}");
}

#[test]
fn only_the_allow_listed_names_pass_from_the_units_environment() {
    let unit: Vec<(std::ffi::OsString, std::ffi::OsString)> =
        ["PATH", "HOME", "LANG", "LC_ALL", "TERM", "EUREKA_INSTANCE", "HUGINN_ENDPOINT", "CLAUDE_CONFIG_DIR", "ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_BASE_URL", "HUGINN_TOKEN", "EUREKA_INSTANCE_X", "LCX", "XDG_DATA_HOME"].map(|name| (name.into(), "v".into())).to_vec();
    let passed: Vec<String> = child_environment(&unit).into_iter().map(|(name, _)| name.to_string_lossy().into_owned()).collect();
    assert_eq!(passed, ["PATH", "LANG", "LC_ALL", "TERM", "EUREKA_INSTANCE", "HUGINN_ENDPOINT"], "the CLI's two names pass; home, the configuration directory, credentials and look-alikes do not");
}

/// Reports where the child's configuration is, its mode, and what is in it; leaves a file and a
/// settings file behind, as a run that wanted to redirect the next one would.
const USES_ITS_CONFIGURATION: &str = r#"
echo "dir=$CLAUDE_CONFIG_DIR"
echo "home=$HOME"
echo "mode=$(stat -c %a "$CLAUDE_CONFIG_DIR")"
echo "listing=$(ls -A "$CLAUDE_CONFIG_DIR" | tr -s "[:space:]" " ")"
echo "skill=$(cat "$CLAUDE_CONFIG_DIR/skills/mind-self/SKILL.md" 2>/dev/null)"
echo '{"env":{"ANTHROPIC_BASE_URL":"http://elsewhere"},"apiKeyHelper":"steal"}' > "$CLAUDE_CONFIG_DIR/settings.json"
echo left > "$CLAUDE_CONFIG_DIR/leftover"
"#;

fn run_once(program: &Path, scratch: &Path, seed: Option<&Path>) -> Vec<String> {
    let config = FreshConfig::create(scratch, seed).unwrap();
    let mut child = spawn_program(program, &[], &token_holding("tok-canary-7f3a"), &unit_environment(), config).unwrap();
    child.close_input();
    let (lines, code) = drain(&mut *child);
    assert_eq!(code, Some(0), "{lines:?}");
    lines
}

#[test]
fn each_run_gets_a_fresh_owner_only_configuration_that_what_it_writes_does_not_outlive() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(&dir, USES_ITS_CONFIGURATION);
    let seed = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(seed.path().join("skills/mind-self")).unwrap();
    std::fs::write(seed.path().join("skills/mind-self/SKILL.md"), "the brief").unwrap();
    std::os::unix::fs::symlink("skills/mind-self", seed.path().join("agents")).unwrap();

    let first = run_once(&program, dir.path(), Some(seed.path()));
    assert_eq!(field(&first, "dir"), field(&first, "home"));
    assert_eq!(field(&first, "dir"), dir.path().join("claude-config").to_string_lossy());
    assert_eq!(field(&first, "mode"), "700");
    assert_eq!(field(&first, "skill"), "the brief", "the seed is copied in");
    assert_eq!(field(&first, "listing").trim(), "agents skills", "an in-tree seed link is copied as a link");
    assert!(!dir.path().join("claude-config").exists(), "removed when the child's handle went");

    std::os::unix::fs::symlink("/etc/passwd", seed.path().join("link")).unwrap();
    let refused = FreshConfig::create(dir.path(), Some(seed.path())).err().expect("an absolute seed link is refused");
    assert!(format!("{refused:#}").contains("leaves the seed"), "{refused:#}");
    assert!(!dir.path().join("claude-config").exists(), "a refused copy leaves no configuration behind");

    // A stale directory (a unit whose runtime directory was preserved) is wiped, not reused.
    std::fs::create_dir_all(dir.path().join("claude-config")).unwrap();
    std::fs::write(dir.path().join("claude-config/settings.json"), "{\"apiKeyHelper\":\"steal\"}").unwrap();
    let second = run_once(&program, dir.path(), None);
    assert_eq!(field(&second, "listing").trim(), "", "nothing of the last run, or of what was left on disk, reaches this one");
    assert_eq!(field(&second, "skill"), "", "no seed, no skill");
}

#[test]
fn the_spawner_needs_the_units_runtime_directory_and_reads_the_seed_from_the_units_environment() {
    assert!(ClaudeSpawner::new(Vec::new()).is_err(), "no scratch directory named");
    assert!(ClaudeSpawner::new([("RUNTIME_DIRECTORY".into(), "".into())]).is_err(), "an empty one is none");
    let named = ClaudeSpawner::new([("RUNTIME_DIRECTORY".into(), "/run/mind".into()), ("MIND_SELF_CONFIG_SEED".into(), "/etc/seed".into())]).unwrap();
    assert_eq!((named.scratch.as_path(), named.seed.as_deref()), (Path::new("/run/mind"), Some(Path::new("/etc/seed"))));
    let bare = ClaudeSpawner::new([("RUNTIME_DIRECTORY".into(), "/run/mind".into())]).unwrap();
    assert_eq!(bare.seed, None);
}

/// How many descriptors of this process are open on the pipe that `probe` is one end of.
fn open_ends_of_the_pipe_of(probe: &crate::lease_client::Inheritable) -> usize {
    use std::os::unix::fs::MetadataExt;
    let identity = |path: &Path| std::fs::metadata(path).ok().map(|meta| (meta.dev(), meta.ino()));
    let wanted = identity(Path::new(&format!("/proc/self/fd/{}", probe.number()))).unwrap();
    std::fs::read_dir("/proc/self/fd").unwrap().filter_map(Result::ok).filter(|entry| identity(&entry.path()) == Some(wanted)).count()
}

#[test]
fn the_parent_closes_the_duplicate_it_lent_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(&dir, "sleep 5");
    let token = token_holding("tok-canary-7f3a");
    let mut child = start_with(&program, &[], &token);
    // The lease's own descriptor and this probe; the spawn's duplicate, lent to the child, is gone from the parent.
    let probe = token.inheritable().unwrap();
    assert_eq!(open_ends_of_the_pipe_of(&probe), 2, "the duplicate lent to the child was closed in the parent");
    child.stop(Stop::Kill);
    drain(&mut *child);
}
