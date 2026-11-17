//! The one fake HTTP server the source tests talk to (the forge's and Matrix's):
//! it answers by path, holds open a path it has no answer for as a hung server
//! does, and records each request's method, path and `Authorization` header.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use serde_json::json;

/// What a fake server answers: a status, the `Location` header when there is one, and a body.
pub struct Reply {
    pub status: u16,
    pub location: Option<String>,
    pub body: String,
}

impl From<String> for Reply {
    fn from(body: String) -> Self {
        Self { status: 200, location: None, body }
    }
}

impl Reply {
    /// What the live forge answers an anonymous reader for a private repo.
    pub fn not_found() -> Self {
        Self { status: 404, location: None, body: json!({ "message": "The target couldn't be found." }).to_string() }
    }

    /// What the live forge answers for a renamed repo: a 301 to its public address, `/forge` prefix and all.
    pub fn moved_to(location: &str) -> Self {
        Self { status: 301, location: Some(location.to_owned()), body: String::new() }
    }

    pub fn status(status: u16, body: &str) -> Self {
        Self { status, location: None, body: body.to_owned() }
    }
}

/// One request as the server saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
}

/// Every request a fake server has seen, in order.
#[derive(Clone, Default)]
pub struct Asked(Arc<Mutex<Vec<Seen>>>);

impl Asked {
    pub fn seen(&self) -> Vec<Seen> {
        self.0.lock().unwrap().clone()
    }

    pub fn paths(&self) -> Vec<String> {
        self.0.lock().unwrap().iter().map(|seen| seen.path.clone()).collect()
    }
}

/// Answers each request by its path from `routes`; a path it has no answer for is held open and never answered.
pub fn forge_of<R: Into<Reply>>(routes: impl Fn(&str) -> Option<R> + Send + 'static) -> (String, Asked) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked = Asked::default();
    let record = asked.clone();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let mut words = request.split_whitespace();
            let method = words.next().unwrap_or_default().to_owned();
            let path = words.next().unwrap_or_default().to_owned();
            let authorization = request
                .lines()
                .find_map(|line| line.split_once(':').filter(|(name, _)| name.eq_ignore_ascii_case("authorization")))
                .map(|(_, value)| value.trim().to_owned());
            record.0.lock().unwrap().push(Seen { method, path: path.clone(), authorization });
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
