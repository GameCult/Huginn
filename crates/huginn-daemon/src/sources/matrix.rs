//! Synapse's client API as the reader user sees it: Matrix JSON in, `RoomEvent`
//! and `RoomFacts` out. JSON for Matrix is spoken here and nowhere else. This
//! client reads with GET only and holds no policy: it filters nothing, decides
//! no room's or sender's scope, and names no platform. What is in scope is
//! `room_scope`'s; the token is read at one place, the `Authorization` header.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::chat::{EventKind, EventPage, RoomEvent, RoomHistory};
use super::room_scope::RoomFacts;

const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// A bridge's state event types: either marks a room as relaying another platform.
const BRIDGE_TYPES: [&str; 2] = ["m.bridge", "uk.half-shot.bridge"];

pub struct HttpMatrix {
    agent: ureq::Agent,
    base: String,
    token: String,
}

impl std::fmt::Debug for HttpMatrix {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("HttpMatrix").field("base", &self.base).finish_non_exhaustive()
    }
}

/// A path segment with everything but unreserved characters escaped.
fn segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

impl HttpMatrix {
    pub fn new(base_url: &str, token: &str) -> Self {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(CALL_TIMEOUT)).http_status_as_error(false).build().into();
        Self { agent, base: base_url.trim_end_matches('/').to_owned(), token: token.to_owned() }
    }

    /// One GET of `path` (already escaped): the status and the JSON body. No message made here carries the token.
    fn get(&self, path: &str) -> Result<(u16, Value)> {
        let url = format!("{}{path}", self.base);
        let mut response = self
            .agent
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .call()
            .with_context(|| format!("no answer from {url}"))?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().with_context(|| format!("reading the answer from {url}"))?;
        Ok((status, serde_json::from_str(&body).unwrap_or(Value::Null)))
    }

    fn ok(&self, path: &str) -> Result<Value> {
        let (status, body) = self.get(path)?;
        if status != 200 {
            bail!("synapse answered {status} for {path}");
        }
        Ok(body)
    }
}

fn event_of(raw: &Value) -> Result<RoomEvent> {
    let text = |key: &str| raw[key].as_str().map(str::to_owned).with_context(|| format!("an event has no {key}"));
    let content = &raw["content"];
    let content_keys = content.as_object().map(|map| map.keys().cloned().collect()).unwrap_or_default();
    Ok(RoomEvent { event_id: text("event_id")?, sender: text("sender")?, ts_ms: raw["origin_server_ts"].as_u64().context("an event has no origin_server_ts")?, content_keys, kind: kind_of(raw) })
}

fn kind_of(raw: &Value) -> EventKind {
    let content = &raw["content"];
    if !raw["unsigned"]["redacted_because"].is_null() {
        return EventKind::Other;
    }
    match raw["type"].as_str() {
        Some("m.room.redaction") => match content["redacts"].as_str().or_else(|| raw["redacts"].as_str()) {
            Some(target) => EventKind::Redaction { target: target.to_owned() },
            None => EventKind::Other,
        },
        Some("m.room.message") if matches!(content["msgtype"].as_str(), Some("m.text" | "m.notice" | "m.emote")) => {
            let Some(body) = content["body"].as_str() else { return EventKind::Other };
            let relation = &content["m.relates_to"];
            if relation["rel_type"].as_str() == Some("m.replace") {
                return match (relation["event_id"].as_str(), content["m.new_content"]["body"].as_str()) {
                    (Some(target), Some(new)) => EventKind::Edit { target: target.to_owned(), body: new.to_owned() },
                    _ => EventKind::Other,
                };
            }
            EventKind::Message { body: body.to_owned() }
        }
        _ => EventKind::Other,
    }
}

impl RoomHistory for HttpMatrix {
    fn joined_rooms(&mut self) -> Result<Vec<String>> {
        let body = self.ok("/_matrix/client/v3/joined_rooms")?;
        let rooms = body["joined_rooms"].as_array().context("the joined rooms answer is not a list")?;
        Ok(rooms.iter().filter_map(|room| room.as_str().map(str::to_owned)).collect())
    }

    fn facts(&mut self, room_id: &str) -> Result<RoomFacts> {
        let body = self.ok(&format!("/_matrix/client/v3/rooms/{}/state", segment(room_id)))?;
        let state = body.as_array().context("the room state is not a list")?;
        let mut facts = RoomFacts { room_id: room_id.to_owned(), alias: None, alt_aliases: Vec::new(), name: None, encrypted: false, direct: false, bridge: None };
        for event in state {
            let content = &event["content"];
            match event["type"].as_str() {
                Some("m.room.canonical_alias") => {
                    facts.alias = content["alias"].as_str().map(str::to_owned);
                    facts.alt_aliases = content["alt_aliases"].as_array().map(|list| list.iter().filter_map(|alias| alias.as_str().map(str::to_owned)).collect()).unwrap_or_default();
                }
                Some("m.room.name") => facts.name = content["name"].as_str().map(str::to_owned),
                Some("m.room.encryption") => facts.encrypted = true,
                Some("m.room.member") if content["is_direct"].as_bool() == Some(true) => facts.direct = true,
                Some(kind) if BRIDGE_TYPES.contains(&kind) => {
                    facts.bridge.get_or_insert_with(|| kind.to_owned());
                }
                _ => {}
            }
        }
        Ok(facts)
    }

    fn events_after(&mut self, room_id: &str, after: Option<&str>, limit: u32) -> Result<Option<EventPage>> {
        let room = segment(room_id);
        let from = match after {
            None => String::new(),
            Some(event) => {
                let (status, body) = self.get(&format!("/_matrix/client/v3/rooms/{room}/context/{}?limit=0", segment(event)))?;
                match status {
                    200 => match body["end"].as_str() {
                        Some(end) => format!("&from={}", segment(end)),
                        None => bail!("synapse gave no end token for the context of {event} in {room_id}"),
                    },
                    404 => return Ok(None),
                    other => bail!("synapse answered {other} for the context of {event} in {room_id}"),
                }
            }
        };
        let body = self.ok(&format!("/_matrix/client/v3/rooms/{room}/messages?dir=f&limit={limit}{from}"))?;
        let chunk = body["chunk"].as_array().context("the messages answer has no chunk")?;
        let events = chunk.iter().map(event_of).collect::<Result<Vec<_>>>()?;
        let more = body["end"].as_str().is_some() && !events.is_empty();
        Ok(Some(EventPage { events, more }))
    }
}
