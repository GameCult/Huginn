//! The stream's HTTP agent ignores the process environment's proxy settings and follows no redirects.
//!
//! An agent reads `ALL_PROXY` and friends when it is built, so the check runs in a child process
//! that has them. The parent hosts three loopback listeners: the model host (it answers every
//! request with a redirect), the proxy the child's environment names, and the redirect's target.

// The suite re-executes its own test binary as the child and hands it the endpoint through the
// environment. That is test plumbing in its own crate: it reaches no library code, which
// `#![forbid]`s these readers.
#![allow(clippy::disallowed_methods, reason = "the test re-executes itself and hands the child its role through the environment")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mind_body::stream::{Continuer, OllamaContinuer, Sampling};

/// Counts the connections a loopback listener accepts, answering each with `reply` when given.
fn listen(reply: Option<String>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&hits);
    std::thread::spawn(move || {
        for connection in listener.incoming().flatten() {
            let mut connection = connection;
            counted.fetch_add(1, Ordering::SeqCst);
            if let Some(reply) = &reply {
                let mut seen = [0u8; 4096];
                let _ = connection.read(&mut seen);
                let _ = connection.write_all(reply.as_bytes());
            }
        }
    });
    (format!("http://{address}"), hits)
}

fn settled(hits: &AtomicUsize, wanted: bool) -> bool {
    for _ in 0..40 {
        if (hits.load(Ordering::SeqCst) > 0) == wanted {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn child(role: &str, endpoint: &str, proxy: &str) {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "the_stream_agent_ignores_the_proxy_environment", "--nocapture"])
        .env("STREAM_PROXY_CHILD", role)
        .env("STREAM_PROXY_ENDPOINT", endpoint)
        .env("ALL_PROXY", proxy)
        .env("HTTP_PROXY", proxy)
        .status()
        .unwrap();
    assert!(status.success(), "the {role} child failed");
}

#[test]
fn the_stream_agent_ignores_the_proxy_environment() {
    if let Ok(role) = std::env::var("STREAM_PROXY_CHILD") {
        let endpoint = std::env::var("STREAM_PROXY_ENDPOINT").unwrap();
        if role == "plain" {
            let plain: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(2))).build().into();
            let _ = plain.get(&endpoint).call();
        } else {
            let sampling = Sampling { temperature_permille: 1000, min_p_permille: 50, max_tokens: 16 };
            let _ = OllamaContinuer::open(&endpoint, "b").unwrap().continue_raw("hello", &sampling, 1);
        }
        return;
    }
    let (target, target_hits) = listen(None);
    let (proxy, proxy_hits) = listen(None);
    let redirect = format!("HTTP/1.1 302 Found\r\nLocation: {target}/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let (endpoint, endpoint_hits) = listen(Some(redirect));

    child("stream", &endpoint, &proxy);
    assert!(settled(&endpoint_hits, true), "the stream never reached its own endpoint");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(proxy_hits.load(Ordering::SeqCst), 0, "the stream's agent took the environment's proxy");
    assert_eq!(target_hits.load(Ordering::SeqCst), 0, "the stream's agent followed a redirect");

    child("plain", &endpoint, &proxy);
    assert!(settled(&proxy_hits, true), "the child's environment names a proxy");
}
