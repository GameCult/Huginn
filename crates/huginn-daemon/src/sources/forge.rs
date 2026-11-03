//! The forge's catalog over its own loopback API. JSON for the forge is spoken
//! here and in `index/qdrant.rs`, and nowhere else.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use huginn_mind::eureka_pipeline::{OrgRepo, Short, Slug};
use huginn_mind::{OpenPr, PrAction, RepoActivity, RepoChange, RepoEvent, RepoMoved};
use serde_json::Value;

use super::{Catalog, Forge, SourceRepo};

/// The organisation whose repositories are the corpus.
pub const ORG: &str = "GameCult";

const PAGE: usize = 50;
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The forge as one HTTP address. Repos are listed through the organisation's
/// API, the unauthenticated view of it: what an anonymous reader may see is
/// what is indexed.
pub struct HttpForge {
    agent: ureq::Agent,
    base: String,
}

impl HttpForge {
    pub fn new(base_url: &str) -> Self {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(CALL_TIMEOUT)).http_status_as_error(false).build().into();
        Self { agent, base: base_url.trim_end_matches('/').to_owned() }
    }

    /// One GET: the status, the count of repos the forge says it holds
    /// (`X-Total-Count`, which only a header carries, so this does not go
    /// through `index::Http`), and the body.
    fn page(&self, path: &str) -> Result<(u16, Option<usize>, String)> {
        let url = format!("{}{path}", self.base);
        let mut response = self.agent.get(&url).call().with_context(|| format!("no answer from {url}"))?;
        let status = response.status().as_u16();
        let total = response.headers().get("x-total-count").and_then(|value| value.to_str().ok()).and_then(|value| value.trim().parse().ok());
        let body = response.body_mut().read_to_string().with_context(|| format!("reading the answer from {url}"))?;
        Ok((status, total, body))
    }
}

/// Why the pages listed are not the whole catalog, or `None` when the repos
/// listed (before any are filtered out) are the count the forge states. A forge
/// that states no count cannot vouch for its list.
pub(crate) fn shortfall(listed: usize, stated: Option<usize>) -> Option<String> {
    match stated {
        None => Some("the forge stated no X-Total-Count".to_owned()),
        Some(total) if total == listed => None,
        Some(total) => Some(format!("the forge counts {total} repos and {listed} were listed")),
    }
}

/// The repos worth indexing in one page of the forge's answer: public,
/// unarchived, not a mirror, not empty. A flag the forge does not state counts
/// against the repo.
pub(crate) fn indexable(page: &Value) -> Result<Vec<SourceRepo>> {
    let repos = page.as_array().context("the forge's repo page is not a list")?;
    let flag = |repo: &Value, key: &str| repo[key].as_bool().unwrap_or(true);
    Ok(repos
        .iter()
        .filter(|repo| !flag(repo, "private") && !flag(repo, "archived") && !flag(repo, "mirror") && !flag(repo, "empty"))
        .filter_map(|repo| {
            Some(SourceRepo { name: repo["name"].as_str()?.to_owned(), default_branch: repo["default_branch"].as_str()?.to_owned() })
        })
        .collect())
}

impl Forge for HttpForge {
    fn catalog(&mut self) -> Result<Catalog> {
        let mut found = Vec::new();
        let (mut listed_in_all, mut stated) = (0, None);
        for page in 1.. {
            let path = format!("/api/v1/orgs/{ORG}/repos?page={page}&limit={PAGE}");
            let (status, total, answer) = self.page(&path)?;
            ensure!(status == 200, "the forge answered {status} to {path}");
            stated = stated.or(total);
            let parsed: Value = serde_json::from_str(&answer).context("the forge's repo page is not JSON")?;
            let listed = parsed.as_array().map_or(0, Vec::len);
            listed_in_all += listed;
            found.extend(indexable(&parsed)?);
            if listed < PAGE {
                break;
            }
        }
        Ok(Catalog { repos: found, shortfall: shortfall(listed_in_all, stated) })
    }

    fn clone_url(&self, name: &str) -> String {
        format!("{}/{ORG}/{name}.git", self.base)
    }
}

/// The whole of one activity read, from its first call to its last. A call is given what is left of it, so a forge that
/// answers slowly, and not only one that never answers, cannot hold the daemon's one thread longer than this.
const ACTIVITY_CALL_TIMEOUT: Duration = Duration::from_secs(2);
/// How many pages of one repo's feed a read takes: 200 entries, of anyone. A repo with more inside the read's reach is named in `capped`.
const REPO_FEED_PAGES: usize = 4;
/// How far back a read with no cursor looks.
const FIRST_WINDOW_MS: i64 = 72 * 3600 * 1000;

impl HttpForge {
    /// One GET inside the read's budget, anonymous, and no redirect followed: a redirect names a path on the public address, which the
    /// loopback does not serve. An error names no address: the forge's answer is the reader's to know about, and nothing of it reaches
    /// a refusal.
    fn answer(&self, path: &str, deadline: Instant) -> Result<ureq::http::Response<ureq::Body>> {
        let left = deadline.checked_duration_since(Instant::now()).filter(|left| !left.is_zero()).context("the activity read ran out of time")?;
        self.agent
            .get(format!("{}{path}", self.base))
            .config()
            .max_redirects(0)
            .max_redirects_will_error(false)
            .timeout_global(Some(left))
            .build()
            .call()
            .map_err(|_| anyhow::anyhow!("the forge did not answer"))
    }

    /// A GET that must answer 200 with JSON, or the read fails.
    fn within(&self, path: &str, deadline: Instant) -> Result<Value> {
        json_of(self.answer(path, deadline)?)?.context("the forge refused a read")
    }

    /// One page of one repo's feed. A repo the anonymous read cannot reach answers 404 (private) or a redirect (renamed): that is a fact
    /// about the repo, `Dark`, and the read goes on. Any other refusal fails the read.
    fn feed(&self, repo: &OrgRepo, page: usize, deadline: Instant) -> Result<Feed> {
        let response = self.answer(&format!("/api/v1/repos/{}/activities/feeds?limit={PAGE}&page={page}", repo.0), deadline)?;
        let status = response.status().as_u16();
        if status == 404 || (300..400).contains(&status) {
            let to = response.headers().get("location").and_then(|location| location.to_str().ok()).and_then(redirect_target).filter(|to| !to.0.eq_ignore_ascii_case(&repo.0));
            return Ok(Feed::Dark(to));
        }
        Ok(Feed::Entries(json_of(response)?.context("the forge refused a read")?))
    }

    /// What others did in `repos` on the forge, and on a read with no cursor what is open and what the forge cannot show. Each repo's
    /// own feed is read, newest first, down to the cursor, or on a read with no cursor down to 72 hours ago, so other repos' traffic
    /// cannot crowd hers out. `capped` names a repo whose feed the read stopped at the page cap. A repo whose feed answers 404 or a redirect (private, renamed) is a fact about that repo, never a failure of the read: it is `unseen`, and a redirect that names a repo is `moved` and is not followed. A network failure or the budget still fails the read whole. One read, one budget; the first
    /// call that fails, or that the budget cannot reach, fails the read whole: an answer is complete or names `capped`, never a silent
    /// part. Only ids, instants, logins, repo names, numbers and counts leave: a title, a comment and a commit message are never read.
    pub fn activity(&self, repos: &[OrgRepo], instance: &Slug, after: Option<u64>, now: DateTime<Utc>) -> Result<RepoActivity> {
        let deadline = Instant::now() + ACTIVITY_CALL_TIMEOUT;
        let window_start = if after.is_some() { i64::MIN } else { now.timestamp_millis() - FIRST_WINDOW_MS };
        let (mut head, mut events, mut capped, mut dark, mut moved) = (0_u64, Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for repo in repos {
            let mut reached = false;
            for page in 1..=REPO_FEED_PAGES {
                let feed = match self.feed(repo, page, deadline)? {
                    Feed::Entries(feed) => feed,
                    Feed::Dark(to) => {
                        dark.push(repo.clone());
                        moved.extend(to.map(|to| RepoMoved { from: repo.clone(), to }));
                        reached = true;
                        break;
                    }
                };
                let entries = feed.as_array().context("the forge's feed is not a list")?;
                for entry in entries {
                    let Some(id) = entry["id"].as_u64() else { continue };
                    // The cursor takes every id seen, one past the window too: a quiet forge must not leave it at 0.
                    head = head.max(id);
                    if after.is_some_and(|after| id <= after) || entry["created"].as_str().and_then(instant_ms).is_some_and(|at| at < window_start) {
                        reached = true;
                        break;
                    }
                    events.extend(event_of(entry, id, repos, instance));
                }
                reached |= entries.len() < PAGE;
                if reached {
                    break;
                }
            }
            if !reached {
                capped.push(repo.clone());
            }
        }
        events.sort_by(|a, b| b.id.cmp(&a.id));
        // The cursor never moves backwards: a read that saw nothing new keeps the one it was given.
        head = head.max(after.unwrap_or(0));
        let (mut open_prs, mut unseen) = (Vec::new(), Vec::new());
        let moved = if after.is_none() { moved } else { Vec::new() };
        if after.is_none() {
            // The open PRs and the org listing are paged to the short page, never to a cap: the budget ends a read that runs long, and
            // ends it whole, so each list is complete or the read fails. A cap would say "nothing else is open" past it, falsely.
            for page in 1.. {
                let found = self.within(&format!("/api/v1/repos/issues/search?type=pulls&state=open&owner={ORG}&limit={PAGE}&page={page}"), deadline)?;
                let found = found.as_array().context("the forge's search is not a list")?;
                open_prs.extend(found.iter().filter_map(|pr| open_pr_of(pr, repos, instance)));
                if found.len() < PAGE {
                    break;
                }
            }
            let mut listed = BTreeSet::new();
            for page in 1.. {
                let found = self.within(&format!("/api/v1/orgs/{ORG}/repos?limit={PAGE}&page={page}"), deadline)?;
                let found = found.as_array().context("the forge's listing is not a list")?;
                listed.extend(found.iter().filter_map(|repo| repo["full_name"].as_str().map(str::to_ascii_lowercase)));
                if found.len() < PAGE {
                    break;
                }
            }
            unseen = repos.iter().filter(|repo| !listed.contains(&repo.identity()) || dark.contains(repo)).cloned().collect();
        }
        Ok(RepoActivity { head, events, capped, open_prs, unseen, moved })
    }
}

/// What one page of a repo's feed answered.
enum Feed {
    Entries(Value),
    /// 404 or a redirect; the repo the redirect names, when it names one.
    Dark(Option<OrgRepo>),
}

/// The body of a response as JSON when it answered 200, else `None`.
fn json_of(mut response: ureq::http::Response<ureq::Body>) -> Result<Option<Value>> {
    if response.status().as_u16() != 200 {
        return Ok(None);
    }
    let body = response.body_mut().read_to_string().map_err(|_| anyhow::anyhow!("the forge's answer was cut off"))?;
    serde_json::from_str(&body).map(Some).map_err(|_| anyhow::anyhow!("the forge's answer is not JSON"))
}

/// The repo a redirect's `Location` names: the two segments after `/repos/`, owner and name, wherever the path sits and whatever follows.
fn redirect_target(location: &str) -> Option<OrgRepo> {
    let rest = location.split_once("/repos/")?.1.split(['?', '#']).next()?;
    let mut segments = rest.split('/');
    let (owner, name) = (segments.next().filter(|owner| !owner.is_empty())?, segments.next().filter(|name| !name.is_empty())?);
    Some(OrgRepo(format!("{owner}/{name}")))
}

fn instant_ms(text: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(text).ok().map(|at| at.timestamp_millis())
}

fn is_her(login: &str, instance: &Slug) -> bool {
    login.eq_ignore_ascii_case(&instance.0)
}

/// The PR numbers: element 0 of a PR op's content, which the forge writes as a two-element list whose second element is the
/// title or comment and is never read.
fn pr_number(content: &str) -> Option<u32> {
    let parsed: Value = serde_json::from_str(content).ok()?;
    parsed.as_array()?.first()?.as_str()?.parse().ok()
}

/// One feed entry as an event, or nothing: not hers to see, not a change worth a line, or not parseable.
fn event_of(entry: &Value, id: u64, repos: &[OrgRepo], instance: &Slug) -> Option<RepoEvent> {
    let actor = entry["act_user"]["login"].as_str().filter(|actor| !is_her(actor, instance) && actor.len() <= 200)?;
    let repo = OrgRepo(entry["repo"]["full_name"].as_str()?.to_owned());
    if !repos.contains(&repo) {
        return None;
    }
    let content = entry["content"].as_str().unwrap_or_default();
    let action = match entry["op_type"].as_str()? {
        "commit_repo" => {
            let on_default = entry["ref_name"].as_str()? == format!("refs/heads/{}", entry["repo"]["default_branch"].as_str()?);
            let commits = serde_json::from_str::<Value>(content).ok().and_then(|parsed| parsed["Len"].as_u64()).and_then(|len| u32::try_from(len).ok()).unwrap_or(1);
            return on_default.then(|| RepoEvent { id, at: at_of(entry), repo, actor: Short(actor.to_owned()), change: RepoChange::Pushed { commits } });
        }
        "create_pull_request" => PrAction::Opened,
        "merge_pull_request" => PrAction::Merged,
        "close_pull_request" => PrAction::Closed,
        "reopen_pull_request" => PrAction::Reopened,
        "comment_pull" => PrAction::Commented,
        "approve_pull_request" => PrAction::Approved,
        "reject_pull_request" => PrAction::Rejected,
        "auto_merge_pull_request" => PrAction::AutoMerge,
        _ => return None,
    };
    Some(RepoEvent { id, at: at_of(entry), repo, actor: Short(actor.to_owned()), change: RepoChange::Pr { number: pr_number(content)?, action } })
}

fn at_of(entry: &Value) -> u64 {
    entry["created"].as_str().and_then(instant_ms).and_then(|at| u64::try_from(at).ok()).unwrap_or(0)
}

/// An open PR in one of her repos, or one she opened anywhere.
fn open_pr_of(pr: &Value, repos: &[OrgRepo], instance: &Slug) -> Option<OpenPr> {
    let repo = OrgRepo(pr["repository"]["full_name"].as_str()?.to_owned());
    let author = pr["user"]["login"].as_str().filter(|author| author.len() <= 200)?;
    if !repos.contains(&repo) && !is_her(author, instance) {
        return None;
    }
    let updated_at = pr["updated_at"].as_str().and_then(instant_ms).and_then(|at| u64::try_from(at).ok()).unwrap_or(0);
    Some(OpenPr { repo, number: u32::try_from(pr["number"].as_u64()?).ok()?, author: Short(author.to_owned()), updated_at })
}

#[cfg(test)]
mod tests {
    use huginn_mind::{PrAction, RepoChange, RepoMoved};
    use serde_json::json;

    use super::*;

    /// Only public, live, original, non-empty repos are the corpus; a repo the
    /// forge does not say is safe is not indexed.
    #[test]
    fn only_public_unarchived_original_nonempty_repos_are_indexed() {
        let repo = |name: &str, private: bool, archived: bool, mirror: bool, empty: bool| {
            json!({ "name": name, "default_branch": "main", "private": private, "archived": archived, "mirror": mirror, "empty": empty })
        };
        let page = json!([
            repo("Open", false, false, false, false),
            repo("Secret", true, false, false, false),
            repo("Old", false, true, false, false),
            repo("Copy", false, false, true, false),
            repo("Void", false, false, false, true),
            { "name": "Silent", "default_branch": "main" },
        ]);
        let names: Vec<String> = indexable(&page).unwrap().into_iter().map(|repo| repo.name).collect();
        assert_eq!(names, ["Open"]);
    }

    /// Serves each page in `pages` once, to the request that names it, with
    /// `total` as its `X-Total-Count` when given.
    fn serve(pages: Vec<String>, total: Option<usize>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for _ in 0..pages.len() {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0_u8; 2048];
                let read = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let body = if request.contains("page=1&") { &pages[0] } else { &pages[1] };
                let count = total.map_or_else(String::new, |total| format!("x-total-count: {total}\r\n"));
                write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{count}content-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        format!("http://{address}")
    }

    fn page_of(range: std::ops::Range<usize>) -> String {
        let repos: Vec<Value> = range.map(|n| json!({ "name": format!("r{n}"), "default_branch": "main", "private": false, "archived": false, "mirror": false, "empty": false })).collect();
        Value::Array(repos).to_string()
    }

    /// The catalog reads every page until one is short: a full first page
    /// does not end it, and a short second page does; clone URLs name the org.
    #[test]
    fn the_catalog_reads_pages_until_a_short_one_and_clones_from_the_org() {
        let base = serve(vec![page_of(0..PAGE), page_of(PAGE..PAGE + 3)], Some(PAGE + 3));
        let mut forge = HttpForge::new(&format!("{base}/"));
        let catalog = forge.catalog().unwrap();
        let names: Vec<String> = catalog.repos.into_iter().map(|repo| repo.name).collect();
        assert_eq!(names.len(), PAGE + 3);
        assert_eq!((names[0].as_str(), names[PAGE + 2].as_str()), ("r0", "r52"));
        assert_eq!(catalog.shortfall, None, "the pages sum to the forge's count");
        assert_eq!(forge.clone_url("Demo"), format!("{base}/GameCult/Demo.git"));
    }

    /// A catalog whose pages do not sum to the count the forge states is
    /// marked short: a forge that caps a page below ours stops the paging at
    /// the first short page, and an empty answer or a missing count vouches for
    /// nothing.
    #[test]
    fn a_catalog_that_does_not_sum_to_the_forges_count_is_short() {
        let truncated = serve(vec![page_of(0..30)], Some(62));
        let catalog = HttpForge::new(&truncated).catalog().unwrap();
        assert_eq!(catalog.repos.len(), 30);
        assert!(catalog.shortfall.as_deref().unwrap().contains("counts 62 repos and 30 were listed"));

        let empty = serve(vec!["[]".into()], Some(62));
        let catalog = HttpForge::new(&empty).catalog().unwrap();
        assert!(catalog.repos.is_empty() && catalog.shortfall.is_some());

        let unstated = serve(vec![page_of(0..3)], None);
        assert!(HttpForge::new(&unstated).catalog().unwrap().shortfall.unwrap().contains("X-Total-Count"));
    }

    /// What a fake forge answers: a status, the `Location` header when there is one, and a body.
    struct Reply {
        status: u16,
        location: Option<String>,
        body: String,
    }

    impl From<String> for Reply {
        fn from(body: String) -> Self {
            Self { status: 200, location: None, body }
        }
    }

    impl Reply {
        /// What the live forge answers an anonymous reader for a private repo.
        fn not_found() -> Self {
            Self { status: 404, location: None, body: json!({ "message": "The target couldn't be found." }).to_string() }
        }

        /// What the live forge answers for a renamed repo: a 301 to its public address, `/forge` prefix and all.
        fn moved_to(location: &str) -> Self {
            Self { status: 301, location: Some(location.to_owned()), body: String::new() }
        }
    }

    /// Answers each request by its path from `routes`; a path it has no answer for is held open and never answered, as a hung forge
    /// does. Every path asked is recorded in order.
    fn forge_of<R: Into<Reply>>(routes: impl Fn(&str) -> Option<R> + Send + 'static) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = std::sync::Arc::clone(&asked);
        std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let path = request.split_whitespace().nth(1).unwrap_or_default().to_owned();
                record.lock().unwrap().push(path.clone());
                match routes(&path).map(Into::into) {
                    Some(Reply { status, location, body }) => {
                        let location = location.map(|location| format!("location: {location}\r\n")).unwrap_or_default();
                        write!(stream, "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{location}content-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).unwrap();
                    }
                    None => held.push(stream),
                }
            }
        });
        (base, asked)
    }

    const FEED_SAMPLE: &str = include_str!("../../tests/fixtures/feed.json");

    const NOW: &str = "2026-11-03T12:00:00Z";

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text).unwrap().with_timezone(&Utc)
    }

    fn repos(names: &[&str]) -> Vec<OrgRepo> {
        names.iter().map(|name| OrgRepo::from(*name)).collect()
    }

    fn me() -> Slug {
        Slug("eureka".into())
    }

    fn is_feed(path: &str) -> bool {
        path.contains("/activities/feeds")
    }

    fn page_number(path: &str) -> Option<usize> {
        path.split("page=").nth(1)?.split('&').next()?.parse().ok()
    }

    /// A repo's own feed as the forge serves it: the entries that name the repo in the path, in the order given, the page the path asks.
    fn repo_feed(entries: &[Value], path: &str) -> Option<String> {
        let repo = path.strip_prefix("/api/v1/repos/")?.split("/activities").next()?;
        let page = page_number(path)?;
        let mine = entries.iter().filter(|entry| entry["repo"]["full_name"].as_str().is_some_and(|name| name.eq_ignore_ascii_case(repo)));
        Some(Value::Array(mine.skip((page - 1) * PAGE).take(PAGE).cloned().collect()).to_string())
    }

    fn sample() -> Vec<Value> {
        serde_json::from_str(FEED_SAMPLE).unwrap()
    }

    /// A default-branch push by `login`, `hours` before `NOW`.
    fn push_at(id: u64, repo: &str, login: &str, hours: i64) -> Value {
        let created = (at(NOW) - chrono::Duration::hours(hours)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        json!({ "id": id, "op_type": "commit_repo", "act_user": { "login": login }, "repo": { "full_name": repo, "default_branch": "main" },
            "ref_name": "refs/heads/main", "content": "{\"Len\":1}", "created": created })
    }

    /// From a sample of the real feed (the forge's own shape, with its commit messages and PR text replaced by canaries), served as each
    /// repo's own feed: her own login, repos outside her set, pushes off the default branch, and every other op are dropped; repos and
    /// logins compare without case; a push's commits are its `Len` and a PR event's number is element 0; no byte of the canary text
    /// leaves the read; only her repos' feeds are asked.
    #[test]
    fn activity_keeps_others_default_branch_pushes_and_pr_events() {
        let entries = sample();
        let (base, asked) = forge_of(move |path| repo_feed(&entries, path));
        let names = ["GameCult/Ratatoskr", "gamecult/aquasynth", "GameCult/Huginn"];
        let read = HttpForge::new(&base).activity(&repos(&names), &Slug("EUREKA".into()), Some(1), at("2026-11-02T23:00:00Z")).unwrap();
        let seen: Vec<(u64, String, String, RepoChange)> = read.events.iter().map(|event| (event.id, event.repo.0.clone(), event.actor.0.clone(), event.change)).collect();
        let pr = |number, action| RepoChange::Pr { number, action };
        assert_eq!(
            seen,
            [
                (18690, "GameCult/Ratatoskr".into(), "Nibu".into(), RepoChange::Pushed { commits: 9 }),
                (18453, "GameCult/Ratatoskr".into(), "void".into(), pr(7, PrAction::Closed)),
                (18443, "GameCult/Ratatoskr".into(), "void".into(), pr(7, PrAction::Commented)),
                (18373, "GameCult/Ratatoskr".into(), "nibu".into(), pr(8, PrAction::Opened)),
                (18293, "GameCult/Ratatoskr".into(), "void".into(), pr(1, PrAction::Merged)),
            ]
        );
        assert_eq!(read.events[0].at, at("2026-11-02T18:40:00Z").timestamp_millis() as u64);
        assert!(read.open_prs.is_empty() && read.unseen.is_empty() && read.capped.is_empty() && read.moved.is_empty(), "a cursor asks for neither list");
        let wire = serde_json::to_string(&read).unwrap();
        assert!(!wire.contains("CANARY") && !wire.contains("obey") && !wire.contains("Message"), "no forge text leaves the read: {wire}");
        let asked = asked.lock().unwrap().clone();
        assert!(!asked.is_empty() && asked.iter().all(|path| names.iter().any(|name| path.starts_with(&format!("/api/v1/repos/{name}/activities/feeds")))), "{asked:?}");
    }

    fn pushes(newest: u64, count: u64, login: &str) -> String {
        let entries: Vec<Value> = (0..count)
            .map(|n| {
                json!({ "id": newest - n * 10, "op_type": "commit_repo", "act_user": { "login": login }, "repo": { "full_name": "GameCult/Huginn", "default_branch": "main" },
                "ref_name": "refs/heads/main", "content": "{\"Len\":1}", "created": "2026-11-02T20:00:00Z" })
            })
            .collect();
        Value::Array(entries).to_string()
    }

    /// With a cursor each repo's feed is read page by page down to it and no further; at the cap of four pages it says it stopped, and
    /// `head` is the newest id seen even when every event was filtered away.
    #[test]
    fn activity_after_a_cursor_pages_back_to_it_and_says_when_it_stopped() {
        let feed = |login: &'static str| {
            move |path: &str| {
                let page = page_number(path)? as u64;
                Some(pushes(2500 - (page - 1) * 500, 50, login))
            }
        };
        let wanted = repos(&["GameCult/Huginn"]);
        let now = at("2026-11-02T23:00:00Z");
        let me = me();

        let (base, asked) = forge_of(feed("nibu"));
        let far = HttpForge::new(&base).activity(&wanted, &me, Some(5), now).unwrap();
        assert!(far.capped == repos(&["GameCult/Huginn"]), "stopped at the cap with the cursor not reached");
        assert_eq!((far.events.len(), far.head), (200, 2500));
        assert_eq!(asked.lock().unwrap().len(), 4, "four pages and no more");

        let (base, asked) = forge_of(feed("nibu"));
        let near = HttpForge::new(&base).activity(&wanted, &me, Some(2300), now).unwrap();
        assert!(near.capped.is_empty());
        assert_eq!((near.events.len(), near.head), (20, 2500), "ids above the cursor only");
        assert_eq!(asked.lock().unwrap().len(), 1);

        let (base, _) = forge_of(feed("eureka"));
        let hers = HttpForge::new(&base).activity(&wanted, &me, Some(2300), now).unwrap();
        assert_eq!((hers.events.len(), hers.head), (0, 2500), "her own pushes are filtered and the cursor still moves");

        let (base, _) = forge_of(feed("nibu"));
        let quiet = HttpForge::new(&base).activity(&wanted, &me, Some(9999), now).unwrap();
        assert_eq!((quiet.events.len(), quiet.head), (0, 9999), "the cursor never moves back");
    }

    /// The busy org cannot hide her repo: the org's own feed is full of another repo's pushes, and a change by someone else in hers 60
    /// hours ago still reaches the read, which never asks the org feed.
    #[test]
    fn a_busy_org_does_not_hide_a_change_in_her_repo() {
        let busy: Vec<Value> = (0..500).map(|n| push_at(10_000 - n, "GameCult/Eureka", "eureka", 1)).collect();
        let busy = Value::Array(busy).to_string();
        let hers = vec![push_at(300, "GameCult/Ratatoskr", "Nibu", 60)];
        let (base, asked) = forge_of(move |path| if path.contains("/orgs/GameCult/activities") { Some(busy.clone()) } else { repo_feed(&hers, path).or_else(|| Some("[]".into())) });
        let read = HttpForge::new(&base).activity(&repos(&["GameCult/Ratatoskr"]), &me(), None, at(NOW)).unwrap();
        assert_eq!(read.events.iter().map(|event| event.id).collect::<Vec<_>>(), [300]);
        assert!(read.capped.is_empty());
        assert!(asked.lock().unwrap().iter().all(|path| !path.contains("/orgs/GameCult/activities")));
    }

    /// A first read keeps what is inside 72 hours and nothing older, and its cursor is the newest id it saw, an entry past the window
    /// too: on a forge quiet for days the next read asks after that id and finds no history to tell as news.
    #[test]
    fn the_first_read_stops_at_the_window_and_a_quiet_forge_still_gives_a_cursor() {
        let entries = vec![
            push_at(300, "GameCult/Huginn", "Nibu", 10),
            push_at(200, "GameCult/Huginn", "Nibu", 71),
            push_at(100, "GameCult/Huginn", "Nibu", 73),
            push_at(90, "GameCult/Huginn", "Nibu", 100),
        ];
        let wanted = repos(&["GameCult/Huginn"]);
        let (base, _) = forge_of(move |path| if is_feed(path) { repo_feed(&entries, path) } else { Some("[]".into()) });
        let busy = HttpForge::new(&base).activity(&wanted, &me(), None, at(NOW)).unwrap();
        assert_eq!((busy.events.iter().map(|event| event.id).collect::<Vec<_>>(), busy.head, busy.capped.len()), (vec![300, 200], 300, 0));

        let quiet_entries = vec![push_at(50, "GameCult/Huginn", "Nibu", 100), push_at(40, "GameCult/Huginn", "Nibu", 120)];
        let (base, _) = forge_of(move |path| if is_feed(path) { repo_feed(&quiet_entries, path) } else { Some("[]".into()) });
        let forge = HttpForge::new(&base);
        let first = forge.activity(&wanted, &me(), None, at(NOW)).unwrap();
        assert_eq!((first.events.len(), first.head, first.capped.len()), (0, 50, 0), "nothing in the window, and a cursor all the same");
        let next = forge.activity(&wanted, &me(), Some(first.head), at(NOW)).unwrap();
        assert_eq!((next.events.len(), next.head), (0, 50), "the history is not told as news");
    }

    /// A feed of exactly one full page goes on to the next page, and a short page ends it; a repo whose feed runs past the page cap
    /// is named in `capped` instead of ending quietly.
    #[test]
    fn a_full_feed_page_asks_for_the_next_and_a_repo_past_the_cap_says_more() {
        let wanted = repos(&["GameCult/Huginn"]);
        let entries: Vec<Value> = (0..53).map(|n| push_at(1000 - n, "GameCult/Huginn", "Nibu", 5)).collect();
        let (base, asked) = forge_of(move |path| if is_feed(path) { repo_feed(&entries, path) } else { Some("[]".into()) });
        let read = HttpForge::new(&base).activity(&wanted, &me(), None, at(NOW)).unwrap();
        assert_eq!((read.events.len(), read.capped.len()), (53, 0));
        assert_eq!(asked.lock().unwrap().iter().filter(|path| is_feed(path)).count(), 2, "page 2 was asked, page 3 was not");

        let entries: Vec<Value> = (0..400).map(|n| push_at(5000 - n, "GameCult/Huginn", "Nibu", 5)).collect();
        let (base, asked) = forge_of(move |path| if is_feed(path) { repo_feed(&entries, path) } else { Some("[]".into()) });
        let read = HttpForge::new(&base).activity(&wanted, &me(), None, at(NOW)).unwrap();
        assert_eq!((read.events.len(), read.capped), (REPO_FEED_PAGES * PAGE, repos(&["GameCult/Huginn"])), "200 read and the rest disclosed");
        assert_eq!(asked.lock().unwrap().iter().filter(|path| is_feed(path)).count(), REPO_FEED_PAGES);
    }

    /// With no cursor the read also lists the open PRs in her repos and hers anywhere, and the repos the forge's listing does not hold.
    /// A full page of either goes on to the next page, so a repo or a PR on page 2 is found, and a short page ends the paging.
    #[test]
    fn activity_first_read_lists_prs_and_unseen_across_pages() {
        let pr = |number: u32, login: &str, repo: &str| json!({ "number": number, "user": { "login": login }, "repository": { "full_name": repo }, "updated_at": "2026-11-02T17:09:43Z" });
        let mut first_page: Vec<Value> = (0..PAGE as u32).map(|n| pr(n + 100, "kiko", "GameCult/Elsewhere")).collect();
        first_page[0] = pr(8, "nibu", "GameCult/Ratatoskr");
        first_page[1] = pr(42, "Eureka", "GameCult/Eureka");
        let search = vec![Value::Array(first_page).to_string(), Value::Array(vec![pr(9, "void", "GameCult/Huginn")]).to_string()];
        let names: Vec<Value> = (0..61).map(|n| json!({ "full_name": match n { 0 => "GameCult/ratatoskr".to_owned(), 1 => "GameCult/Huginn".to_owned(), n => format!("GameCult/r{n}") } })).collect();
        let listing = vec![Value::Array(names[..PAGE].to_vec()).to_string(), Value::Array(names[PAGE..].to_vec()).to_string()];
        let sample = sample();
        let (base, asked) = forge_of(move |path| {
            if is_feed(path) && path.contains("/GameCult/Ghost/") {
                Some(Reply::not_found())
            } else if is_feed(path) {
                repo_feed(&sample, path).map(Reply::from)
            } else if path.contains("/issues/search") {
                search.get(page_number(path)? - 1).cloned().map(Reply::from)
            } else if path.contains("/orgs/GameCult/repos") {
                listing.get(page_number(path)? - 1).cloned().map(Reply::from)
            } else {
                None
            }
        });
        let wanted = repos(&["GameCult/Ratatoskr", "GameCult/Huginn", "GameCult/r60", "GameCult/Ghost"]);
        let first = HttpForge::new(&base).activity(&wanted, &me(), None, at(NOW)).unwrap();
        let prs: Vec<(String, u32)> = first.open_prs.iter().map(|pr| (pr.repo.0.clone(), pr.number)).collect();
        assert_eq!(prs, [("GameCult/Ratatoskr".to_string(), 8), ("GameCult/Eureka".to_string(), 42), ("GameCult/Huginn".to_string(), 9)], "page 2 of the search was read");
        assert_eq!(first.unseen, repos(&["GameCult/Ghost"]), "r60 is on page 2 of the listing");
        let window = at("2026-10-31T12:00:00Z").timestamp_millis() as u64;
        assert!(!first.events.is_empty() && first.events.iter().all(|event| event.at >= window), "inside 72 h: {:?}", first.events);
        let paths = asked.lock().unwrap().clone();
        let count = |what: &str| paths.iter().filter(|path| path.contains(what)).count();
        assert_eq!((count("/issues/search"), count("/orgs/GameCult/repos")), (2, 2), "a short page ends each: {paths:?}");

        // A cursor asks for neither list.
        let before = paths.len();
        HttpForge::new(&base).activity(&wanted, &me(), Some(1), at(NOW)).unwrap();
        let after = asked.lock().unwrap().clone();
        assert!(after[before..].iter().all(|path| is_feed(path)), "only the feeds: {after:?}");
    }

    /// More open PRs than two pages hold are all found: the search is paged to its short page, so a PR of hers on page 3 is read and
    /// the read never answers a list it cut short.
    #[test]
    fn open_prs_past_two_pages_are_all_read() {
        let pr = |number: u32, repo: &str| json!({ "number": number, "user": { "login": "kiko" }, "repository": { "full_name": repo }, "updated_at": "2026-11-02T17:09:43Z" });
        let full = |from: u32| Value::Array((0..PAGE as u32).map(|n| pr(from + n, "GameCult/Elsewhere")).collect()).to_string();
        let mut third: Vec<Value> = (0..PAGE as u32).map(|n| pr(300 + n, "GameCult/Elsewhere")).collect();
        third[PAGE - 1] = pr(7, "GameCult/Huginn");
        let search = vec![full(100), full(200), Value::Array(third).to_string(), "[]".to_owned()];
        let (base, asked) = forge_of(move |path| {
            if is_feed(path) {
                Some("[]".to_owned())
            } else if path.contains("/issues/search") {
                search.get(page_number(path)? - 1).cloned()
            } else if path.contains("/orgs/GameCult/repos") {
                Some(json!([{ "full_name": "GameCult/Huginn" }]).to_string())
            } else {
                None
            }
        });
        let read = HttpForge::new(&base).activity(&repos(&["GameCult/Huginn"]), &me(), None, at(NOW)).unwrap();
        assert_eq!(read.open_prs.iter().map(|pr| pr.number).collect::<Vec<_>>(), [7], "page 3 was read");
        assert_eq!(asked.lock().unwrap().iter().filter(|path| path.contains("/issues/search")).count(), 4, "to the short page");
    }

    /// The forge as it answers an anonymous reader: public repos give their feed (`[]` when quiet), a private repo answers 404, a
    /// renamed repo answers 301 to a path under `/forge` that this loopback does not serve (asking it gives 404).
    fn live_forge(entries: Vec<Value>, public: &'static [&'static str], listing: String) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        forge_of(move |path| {
            if path.starts_with("/forge/") {
                return Some(Reply::not_found());
            }
            if is_feed(path) {
                let repo = path.strip_prefix("/api/v1/repos/")?.split("/activities").next()?;
                if repo.eq_ignore_ascii_case("GameCult/cinny") {
                    return Some(Reply::moved_to(&format!("/forge/api/v1/repos/GameCult/Ratatoskr/activities/feeds?limit={PAGE}&page=1")));
                }
                return if public.iter().any(|name| name.eq_ignore_ascii_case(repo)) { repo_feed(&entries, path).map(Reply::from) } else { Some(Reply::not_found()) };
            }
            if path.contains("/issues/search") {
                return Some("[]".to_owned().into());
            }
            path.contains("/orgs/GameCult/repos").then(|| listing.clone().into())
        })
    }

    /// A repo the anonymous read cannot reach is a fact about that repo, not the end of the read: a private repo (404) and a renamed one
    /// (301, whose target is not followed) are `unseen` and the others still read, on a first read and on a later one; the redirect's
    /// target is told; a network failure still fails the read whole.
    #[test]
    fn a_repo_the_anonymous_read_cannot_reach_is_unseen_and_the_others_still_read() {
        let entries = vec![push_at(300, "GameCult/Huginn", "Nibu", 10), push_at(200, "GameCult/Ratatoskr", "void", 11)];
        let listing = json!([{ "full_name": "GameCult/Huginn" }, { "full_name": "GameCult/Ratatoskr" }]).to_string();
        let (base, asked) = live_forge(entries, &["GameCult/Huginn", "GameCult/Ratatoskr"], listing);
        let wanted = repos(&["GameCult/gamecult-ops", "GameCult/Huginn", "GameCult/cinny", "GameCult/Ratatoskr"]);
        let forge = HttpForge::new(&base);

        let first = forge.activity(&wanted, &me(), None, at(NOW)).unwrap();
        assert_eq!(first.events.iter().map(|event| event.id).collect::<Vec<_>>(), [300, 200], "the readable repos were read");
        assert_eq!(first.unseen, repos(&["GameCult/gamecult-ops", "GameCult/cinny"]));
        assert_eq!(first.moved, [RepoMoved { from: "GameCult/cinny".into(), to: "GameCult/Ratatoskr".into() }]);
        assert!(first.capped.is_empty());
        assert!(asked.lock().unwrap().iter().all(|path| !path.starts_with("/forge/")), "a redirect is not followed: {:?}", asked.lock().unwrap());

        let later = forge.activity(&wanted, &me(), Some(250), at(NOW)).unwrap();
        assert_eq!((later.events.iter().map(|event| event.id).collect::<Vec<_>>(), later.head), (vec![300], 300), "a later read goes on past the dark repos");
        assert!(later.unseen.is_empty() && later.moved.is_empty(), "a cursor asks for neither list");

        let (base, _) = forge_of(|_| None::<String>);
        assert!(HttpForge::new(&base).activity(&wanted, &me(), None, at(NOW)).is_err(), "no answer at all still fails the read");
        let (base, _) = forge_of(|path| is_feed(path).then(|| Reply { status: 500, location: None, body: String::new() }));
        assert!(HttpForge::new(&base).activity(&wanted, &me(), None, at(NOW)).is_err(), "a server error is not a fact about a repo");
    }

    /// A redirect names its repo in its path wherever that sits, and a redirect that names none tells nothing.
    #[test]
    fn a_redirect_names_its_repo_only_when_it_does() {
        let target = |text: &str| redirect_target(text).map(|repo| repo.0);
        assert_eq!(target("/forge/api/v1/repos/GameCult/Ratatoskr"), Some("GameCult/Ratatoskr".into()));
        assert_eq!(target("https://eureka.gamecult.org/forge/api/v1/repos/GameCult/Ratatoskr/activities/feeds?limit=50&page=1"), Some("GameCult/Ratatoskr".into()));
        assert_eq!((target("/user/login"), target("/forge/api/v1/repos/GameCult"), target("/api/v1/repos//x")), (None, None, None));
    }

    /// Every PR op the forge writes becomes its event with the number in element 0, and no other op does; the forge's source writes
    /// all eight the same way, though only four have been seen in its live feed.
    #[test]
    fn every_pr_op_becomes_its_event() {
        let wanted = repos(&["GameCult/Huginn"]);
        let entry = |op: &str, login: &str| {
            json!({ "id": 7, "op_type": op, "act_user": { "login": login }, "repo": { "full_name": "GameCult/Huginn", "default_branch": "main" },
                "content": "[\"12\",\"CANARY\"]", "created": "2026-11-02T20:00:00Z" })
        };
        for (op, action) in [
            ("create_pull_request", PrAction::Opened),
            ("merge_pull_request", PrAction::Merged),
            ("close_pull_request", PrAction::Closed),
            ("reopen_pull_request", PrAction::Reopened),
            ("comment_pull", PrAction::Commented),
            ("approve_pull_request", PrAction::Approved),
            ("reject_pull_request", PrAction::Rejected),
            ("auto_merge_pull_request", PrAction::AutoMerge),
        ] {
            let event = event_of(&entry(op, "nibu"), 7, &wanted, &me()).unwrap_or_else(|| panic!("{op} is an event"));
            assert_eq!(event.change, RepoChange::Pr { number: 12, action }, "{op}");
            assert!(event_of(&entry(op, "Eureka"), 7, &wanted, &me()).is_none(), "{op} by her is not news to her");
        }
        for op in ["delete_branch", "create_repo", "push_tag", "rename_repo"] {
            assert!(event_of(&entry(op, "nibu"), 7, &wanted, &me()).is_none(), "{op} is no change worth a line");
        }
    }

    /// A forge that takes the connection and never answers fails the read inside its bound, and one that answers the feed and then
    /// stalls fails it too: the bound is on the whole read.
    #[test]
    fn a_hung_forge_fails_the_read_within_its_bound() {
        let (base, _) = forge_of(|_| None::<String>);
        let started = Instant::now();
        let read = HttpForge::new(&base).activity(&repos(&["GameCult/Huginn"]), &me(), None, at("2026-11-02T23:00:00Z"));
        assert!(read.is_err());
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());

        let entries = sample();
        let (base, _) = forge_of(move |path| if is_feed(path) { repo_feed(&entries, path) } else { None });
        let started = Instant::now();
        let read = HttpForge::new(&base).activity(&repos(&["GameCult/Huginn"]), &me(), None, at("2026-11-02T23:00:00Z"));
        let error = read.expect_err("the search is never answered");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
        assert!(!format!("{error:#}").contains("127.0.0.1"), "an error names no address");
    }
}
