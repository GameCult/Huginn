//! The forge's catalog over its own loopback API. JSON for the forge is spoken
//! here and in `index/qdrant.rs`, and nowhere else.

use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{Forge, SourceRepo};
use crate::index::{Http, Method};

/// The organisation whose repositories are the corpus.
pub const ORG: &str = "GameCult";

const PAGE: usize = 50;
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The forge as one HTTP address. Repos are listed through the organisation's
/// API, the unauthenticated view of it: what an anonymous reader may see is
/// what is indexed.
pub struct HttpForge {
    http: Http,
    base: String,
}

impl HttpForge {
    pub fn new(base_url: &str) -> Self {
        Self { http: Http::new(base_url, CALL_TIMEOUT), base: base_url.trim_end_matches('/').to_owned() }
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
    fn catalog(&mut self) -> Result<Vec<SourceRepo>> {
        let mut found = Vec::new();
        for page in 1.. {
            let path = format!("/api/v1/orgs/{ORG}/repos?page={page}&limit={PAGE}");
            let (status, answer) = self.http.request(Method::Get, &path, None)?;
            ensure!(status == 200, "the forge answered {status} to {path}");
            let parsed: Value = serde_json::from_str(&answer).context("the forge's repo page is not JSON")?;
            let listed = parsed.as_array().map_or(0, Vec::len);
            found.extend(indexable(&parsed)?);
            if listed < PAGE {
                break;
            }
        }
        Ok(found)
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

    /// Serves each page in `pages` once, to the request that names it.
    fn serve(pages: Vec<String>) -> String {
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
                write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        format!("http://{address}")
    }

    /// The catalog reads every page until one is short: a full first page
    /// does not end it, and a short second page does; clone URLs name the org.
    #[test]
    fn the_catalog_reads_pages_until_a_short_one_and_clones_from_the_org() {
        let page = |range: std::ops::Range<usize>| {
            let repos: Vec<Value> = range.map(|n| json!({ "name": format!("r{n}"), "default_branch": "main", "private": false, "archived": false, "mirror": false, "empty": false })).collect();
            Value::Array(repos).to_string()
        };
        let base = serve(vec![page(0..PAGE), page(PAGE..PAGE + 3)]);
        let mut forge = HttpForge::new(&format!("{base}/"));
        let names: Vec<String> = forge.catalog().unwrap().into_iter().map(|repo| repo.name).collect();
        assert_eq!(names.len(), PAGE + 3);
        assert_eq!((names[0].as_str(), names[PAGE + 2].as_str()), ("r0", "r52"));
        assert_eq!(forge.clone_url("Demo"), format!("{base}/GameCult/Demo.git"));
    }
}
