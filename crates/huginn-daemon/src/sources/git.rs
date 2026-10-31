//! Git, by shelling out: bare mirrors of the forge's repos, what changed
//! between two commits, and the blobs at one. Nothing here knows an index or an
//! embedder; the mirrors are cache and can be deleted whenever.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{Context, Result, bail, ensure};

/// A blob over this many bytes is not indexed.
pub const BLOB_BYTES_MAX: u64 = 256 * 1024;

/// Extensions of files that are not text worth finding, VoidBot's list, plus
/// Unity's serialised YAML (huge, machine written) and lock files.
const SKIPPED_EXTENSIONS: &[&str] = &[
    "7z", "avi", "bmp", "class", "dll", "dylib", "eot", "exe", "gif", "gz", "ico", "jar", "jpeg", "jpg", "lock", "map", "mp3",
    "mp4", "mov", "ogg", "otf", "pdf", "png", "rar", "so", "svg", "tar", "ttf", "wav", "webm", "webp", "woff", "woff2", "zip",
    "meta", "prefab", "unity", "asset", "mat", "anim", "controller", "physicmaterial",
];

/// Lock files whose extension is not `.lock`.
const SKIPPED_NAMES: &[&str] = &["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lockb", "composer.lock"];

const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1";

/// Whether a path is not worth reading, by name alone.
pub fn skipped_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    if SKIPPED_NAMES.contains(&name.as_str()) {
        return true;
    }
    name.rsplit_once('.').is_some_and(|(_, extension)| SKIPPED_EXTENSIONS.contains(&extension))
}

/// The text of a blob worth indexing, or `None`: a NUL byte, bytes that are
/// not UTF-8, or a Git LFS pointer make it not text.
pub fn indexable_text(bytes: &[u8]) -> Option<String> {
    if bytes.contains(&0) || bytes.starts_with(LFS_POINTER) {
        return None;
    }
    String::from_utf8(bytes.to_vec()).ok()
}

/// One file of a tree: its path, the blob it holds and that blob's size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeFile {
    pub path: String,
    pub blob: String,
    pub size: u64,
}

fn git(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args).env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn run(mut command: Command, what: &str) -> Result<Vec<u8>> {
    let output = command.output().with_context(|| format!("starting git to {what}"))?;
    ensure!(
        output.status.success(),
        "git could not {what}: {}",
        String::from_utf8_lossy(&output.stderr).trim().chars().take(400).collect::<String>()
    );
    Ok(output.stdout)
}

/// A bare mirror of one forge repo's default branch.
pub struct Mirror {
    dir: PathBuf,
    branch: String,
}

impl Mirror {
    pub fn path_for(root: &Path, name: &str) -> PathBuf {
        root.join(format!("{name}.git"))
    }

    pub fn new(root: &Path, name: &str, branch: &str) -> Self {
        Self { dir: Self::path_for(root, name), branch: branch.to_owned() }
    }

    /// Fetches the default branch, creating the mirror if it is absent, and
    /// returns the commit it now points at. The branch is forced: a rewritten
    /// history is followed, not refused.
    pub fn fetch(&self, remote: &str) -> Result<String> {
        if !self.dir.join("HEAD").exists() {
            std::fs::create_dir_all(&self.dir).with_context(|| format!("creating {}", self.dir.display()))?;
            run(Command::new("git").arg("init").arg("--bare").arg("--quiet").arg(&self.dir), "initialise a mirror")?;
        }
        let refspec = format!("+refs/heads/{0}:refs/heads/{0}", self.branch);
        run(git(&self.dir, &["fetch", "--quiet", "--no-tags", remote, &refspec]), "fetch the default branch")?;
        self.head()
    }

    pub fn head(&self) -> Result<String> {
        let out = run(git(&self.dir, &["rev-parse", "--verify", &format!("refs/heads/{}", self.branch)]), "read the branch head")?;
        Ok(String::from_utf8_lossy(&out).trim().to_owned())
    }

    /// Whether `old` is an ancestor of `new`. A commit the mirror no longer
    /// holds is not.
    pub fn is_ancestor(&self, old: &str, new: &str) -> bool {
        git(&self.dir, &["merge-base", "--is-ancestor", old, new]).status().is_ok_and(|status| status.success())
    }

    /// The paths that differ between two commits, a rename counted as a delete
    /// and an add.
    pub fn changed_paths(&self, old: &str, new: &str) -> Result<Vec<String>> {
        let out = run(git(&self.dir, &["diff", "--no-renames", "--name-only", "-z", old, new]), "diff two commits")?;
        Ok(out.split(|byte| *byte == 0).filter(|path| !path.is_empty()).map(|path| String::from_utf8_lossy(path).into_owned()).collect())
    }

    /// Every regular file at `commit`; symlinks and submodules are not files.
    pub fn files(&self, commit: &str) -> Result<Vec<TreeFile>> {
        let out = run(git(&self.dir, &["ls-tree", "-r", "-z", "-l", commit]), "list a tree")?;
        let mut files = Vec::new();
        for entry in out.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
            let text = String::from_utf8_lossy(entry);
            let (meta, path) = text.split_once('\t').with_context(|| format!("an ls-tree entry has no tab: {text:?}"))?;
            let mut fields = meta.split_whitespace();
            let (mode, kind, blob, size) = (fields.next(), fields.next(), fields.next(), fields.next());
            let (Some(mode), Some("blob"), Some(blob), Some(size)) = (mode, kind, blob, size) else { continue };
            if mode == "120000" {
                continue;
            }
            let Ok(size) = size.parse::<u64>() else { continue };
            files.push(TreeFile { path: path.to_owned(), blob: blob.to_owned(), size });
        }
        Ok(files)
    }

    /// A reader of blobs by id, one `cat-file --batch` process.
    pub fn blobs(&self) -> Result<Blobs> {
        let mut child = git(&self.dir, &["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("starting git cat-file")?;
        let stdin = child.stdin.take().context("git cat-file has no stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("git cat-file has no stdout")?);
        Ok(Blobs { child, stdin, stdout })
    }
}

/// `git cat-file --batch`, asked one blob at a time.
pub struct Blobs {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Blobs {
    pub fn read(&mut self, blob: &str) -> Result<Vec<u8>> {
        writeln!(self.stdin, "{blob}").and_then(|()| self.stdin.flush()).context("asking git cat-file for a blob")?;
        let mut header = String::new();
        self.stdout.read_line(&mut header).context("reading git cat-file's header")?;
        let mut fields = header.split_whitespace();
        let (Some(_), Some(kind), Some(size)) = (fields.next(), fields.next(), fields.next()) else {
            bail!("git cat-file does not hold blob {blob}: {}", header.trim());
        };
        ensure!(kind == "blob", "git cat-file says {blob} is a {kind}");
        let size: usize = size.parse().with_context(|| format!("a blob size of {size:?}"))?;
        let mut bytes = vec![0; size];
        self.stdout.read_exact(&mut bytes).context("reading a blob")?;
        let mut newline = [0_u8; 1];
        self.stdout.read_exact(&mut newline).context("reading a blob's end")?;
        Ok(bytes)
    }
}

impl Drop for Blobs {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_unity_and_lock_paths_are_skipped_by_name_and_source_is_not() {
        for path in ["a.png", "dir/B.PNG", "x.lock", "Assets/a.prefab", "Assets/a.unity", "a.cs.meta", "p/package-lock.json", "Cargo.lock"] {
            assert!(skipped_path(path), "{path}");
        }
        for path in ["src/main.rs", "README.md", "docs/lock.md", "Assets/Scripts/Player.cs", "Makefile", ".gitignore", "locksmith"] {
            assert!(!skipped_path(path), "{path}");
        }
    }

    #[test]
    fn a_nul_byte_an_lfs_pointer_and_invalid_utf8_are_not_text() {
        assert_eq!(indexable_text(b"fn main() {}\n").as_deref(), Some("fn main() {}\n"));
        assert_eq!(indexable_text(b"a\0b"), None);
        assert_eq!(indexable_text(b"version https://git-lfs.github.com/spec/v1\noid sha256:ab\nsize 12\n"), None);
        assert_eq!(indexable_text(&[0xff, 0xfe, b'a']), None);
    }
}
