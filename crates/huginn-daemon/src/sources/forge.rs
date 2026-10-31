//! The forge's catalog over its own loopback API. JSON for the forge is spoken
//! here and in `index/qdrant.rs`, and nowhere else.

use std::time::Duration;

use anyhow::{Context, Result, ensure};
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

#[cfg(test)]
mod tests {
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
}
