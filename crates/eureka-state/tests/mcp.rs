//! The `eureka-state` binary over stdio, as Claude Code drives it: real
//! processes, real JSON-RPC, a real daemon in-process behind it (or a scripted
//! server where the daemon cannot be made to misbehave).

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::net::UdpSocket;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use huginn_mind::envelope::{OperationFailure, encode_failure, encode_response};
use huginn_mind::HuginnMindResponse;
use huginn_mind::epiphany_pipeline::{PipelineKind, PipelineRef, Short};
use serde_json::{Value, json};

mod common;
use common::*;

const WAIT: Duration = Duration::from_secs(60);

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next: u64,
    /// The server's answer to `initialize`.
    init: Value,
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Mcp {
    /// The binary with exactly the environment given, initialised.
    fn start(env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_eureka-state"));
        command
            .env_remove("EUREKA_INSTANCE")
            .env_remove("HUGINN_ENDPOINT")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("the binary starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = sender.send(line);
            }
        });
        let mut mcp = Self { child, stdin, lines, next: 1, init: Value::Null };
        let init = mcp.rpc(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
        );
        assert!(init.get("result").is_some(), "{init}");
        mcp.init = init["result"].clone();
        mcp.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }

    /// Configured for `instance` against a daemon at `addr`.
    fn at(instance: &str, addr: std::net::SocketAddr) -> Self {
        Self::start(&[("EUREKA_INSTANCE", instance), ("HUGINN_ENDPOINT", &format!("rudp://{addr}"))])
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// One request and its response, whole (`result` or `error`).
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let line = self.lines.recv_timeout(WAIT).expect("the server answers");
            let message: Value = serde_json::from_str(&line).expect("stdout carries only JSON-RPC");
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    /// A tool call's result: whether it is an error, and its JSON body.
    fn call(&mut self, tool: &str, arguments: Value) -> (bool, Value) {
        let response = self.rpc("tools/call", json!({ "name": tool, "arguments": arguments }));
        let result = &response["result"];
        assert!(result.is_object(), "a tool result, not a protocol error: {response}");
        let text = result["content"][0]["text"].as_str().expect("text content");
        (result["isError"] == json!(true), serde_json::from_str(text).expect("a JSON body"))
    }
}

/// A loopback UDP port nothing listens on.
fn closed_port() -> std::net::SocketAddr {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

fn faculty_args(documents: Vec<Value>) -> Value {
    json!({ "faculty": "Hands", "agent": "claude", "session": "session-7", "documents": documents })
}

fn campaign_json() -> Value {
    serde_json::to_value(campaign()).unwrap()
}

/// Every `$ref` in a schema, and the `$defs` it can resolve against.
fn refs(schema: &Value, found: &mut Vec<String>) {
    match schema {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref" {
                    found.push(value.as_str().unwrap().to_string());
                } else {
                    refs(value, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| refs(item, found)),
        _ => {}
    }
}

/// Four tools, each input schema a self-contained document: every `$ref` is
/// internal and resolves, and the selection's definitions are bundled in.
#[test]
fn tools_list_offers_four_tools_with_self_contained_schemas() {
    let mut mcp = Mcp::at(INSTANCE, closed_port());
    let listed = mcp.rpc("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().expect("tools");
    let names: BTreeSet<&str> = tools.iter().map(|tool| tool["name"].as_str().unwrap()).collect();
    assert_eq!(names, BTreeSet::from(["whoami", "admit", "view", "query"]));
    for tool in tools {
        let schema = &tool["inputSchema"];
        let mut found = Vec::new();
        refs(schema, &mut found);
        for reference in &found {
            let name = reference.strip_prefix("#/$defs/").unwrap_or_else(|| panic!("{}: remote $ref {reference}", tool["name"]));
            assert!(schema["$defs"].get(name).is_some(), "{}: {reference} does not resolve", tool["name"]);
        }
        assert!(schema.get("$id").is_none(), "{}: a remote identity", tool["name"]);
    }
    let query = tools.iter().find(|tool| tool["name"] == "query").unwrap();
    for bundled in ["fieldPredicate", "citation", "incoming", "recordRef"] {
        assert!(query["inputSchema"]["$defs"].get(bundled).is_some(), "{bundled} is not bundled");
    }
    assert_eq!(query["inputSchema"]["properties"]["selection"]["properties"]["limit"]["maximum"], json!(200));
}

/// The vendored selection schema is CultLib's, at the revision `cultnet-rs`
/// is built from.
#[test]
fn selection_schema_matches_cultlib() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let metadata = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--offline", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(metadata.status.success(), "{}", String::from_utf8_lossy(&metadata.stderr));
    let metadata: Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let manifest = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "cultnet-rs")
        .expect("cultnet-rs is a dependency")["manifest_path"]
        .as_str()
        .unwrap();
    let crate_dir = std::path::Path::new(manifest).parent().unwrap();
    let published = crate_dir.join("../../contracts/cultnet/cultnet.selection.schema.json");
    let theirs: Value = serde_json::from_str(&std::fs::read_to_string(published).unwrap()).unwrap();
    let ours: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schemas/cultnet.selection.schema.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ours, theirs);
}

/// Every tool against a live daemon, and what each returns.
#[test]
fn every_tool_round_trips_against_a_live_daemon() {
    let (root, daemon) = mind(vec![vec![stewardship()]]);
    let server = serve(root, daemon);
    let mut mcp = Mcp::at(INSTANCE, server.addr);

    let (error, whoami) = mcp.call("whoami", json!({}));
    assert!(!error);
    assert_eq!(whoami["instance"], json!(INSTANCE));
    assert_eq!(whoami["endpoint"], json!(format!("rudp://{}", server.addr)));
    assert_eq!(whoami["reachable"], json!(true));
    assert_eq!(whoami["status"]["instance"], json!(INSTANCE));
    assert_eq!(whoami["status"]["documents"], json!(2));
    assert!(whoami.get("error").is_none(), "{whoami}");

    let (error, admitted) = mcp.call("admit", faculty_args(vec![campaign_json()]));
    assert!(!error);
    let writes = admitted["Committed"]["writes"].as_array().expect("committed").clone();
    assert_eq!(writes.len(), 1, "{admitted}");

    let (error, viewed) = mcp.call("view", json!({ "id": writes[0] }));
    assert!(!error);
    assert_eq!(viewed["id"], writes[0]);
    assert_eq!(viewed["admission"]["provenance"]["session"], json!("session-7"));

    let (error, nothing) = mcp.call("view", json!({ "id": absent() }));
    assert!(!error);
    assert_eq!(nothing, Value::Null);

    let (error, page) = mcp.call("query", json!({ "selection": { "projection": "document" } }));
    assert!(!error);
    assert_eq!(page["matched"], json!(3), "{page}");
    assert_eq!(page["items"]["Documents"].as_array().unwrap().len(), 3);

    let (_, counted) = mcp.call("whoami", json!({}));
    assert_eq!(counted["status"]["receipts"], json!(3));
}

/// Declared identity is configuration, not a per-call argument. Input that
/// names another instance, or another tool, changes neither: the batch is
/// admitted for the configured instance, and the receipt says `eureka-state`.
#[test]
fn admit_ignores_any_instance_in_input() {
    let (root, daemon) = mind(vec![vec![stewardship()]]);
    let server = serve(root, daemon);
    let mut mcp = Mcp::at(INSTANCE, server.addr);

    let mut arguments = faculty_args(vec![campaign_json()]);
    arguments["instance"] = json!("thought-cage");
    arguments["tool"] = json!("someone-else");
    arguments["provenance"] = json!({ "faculty": "Operator", "agent": "x", "session": "y", "tool": "someone-else" });
    let (error, admitted) = mcp.call("admit", arguments);
    assert!(!error);
    let writes = admitted["Committed"]["writes"].as_array().unwrap_or_else(|| panic!("not committed: {admitted}"));

    let (_, viewed) = mcp.call("view", json!({ "id": writes[0] }));
    let provenance = &viewed["admission"]["provenance"];
    assert_eq!(provenance["tool"], json!("eureka-state"));
    assert_eq!(provenance["faculty"], json!("Hands"));
    assert_eq!(provenance["agent"], json!("claude"));
    assert_eq!(provenance["session"], json!("session-7"));
}

/// A mind's refusal is an answer: a successful result carrying the typed
/// refusal. Here the configured instance is not the daemon's.
#[test]
fn a_refusal_is_a_successful_result() {
    let (root, daemon) = mind(vec![vec![stewardship()]]);
    let server = serve(root, daemon);
    let mut mcp = Mcp::at("thought-cage", server.addr);

    let (error, viewed) = mcp.call("view", json!({ "id": absent() }));
    assert!(!error);
    assert!(viewed.get("Refused").is_some(), "{viewed}");
    let (error, queried) = mcp.call("query", json!({ "selection": {} }));
    assert!(!error);
    assert!(queried.get("Refused").is_some(), "{queried}");
    let (error, admitted) = mcp.call("admit", faculty_args(vec![campaign_json()]));
    assert!(!error);
    assert!(admitted.get("Refused").is_some(), "{admitted}");
}

/// A daemon that cannot be reached is `isError` with the endpoint, and
/// `whoami` reports it as `reachable: false` without a protocol error.
#[test]
fn a_closed_port_is_reported_and_is_not_a_protocol_error() {
    let addr = closed_port();
    let mut mcp = Mcp::at(INSTANCE, addr);

    let whoami = mcp.rpc("tools/call", json!({ "name": "whoami", "arguments": {} }));
    assert!(whoami.get("error").is_none(), "{whoami}");
    assert_ne!(whoami["result"]["isError"], json!(true));
    let body: Value = serde_json::from_str(whoami["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(body["reachable"], json!(false));
    assert_eq!(body["status"], Value::Null);
    assert_eq!(body["error"]["error"], json!("Unavailable"));

    let (error, view) = mcp.call("view", json!({ "id": absent() }));
    assert!(error);
    assert_eq!(view["error"], json!("Unavailable"));
    assert_eq!(view["endpoint"], json!(format!("rudp://{addr}")));
}

/// A daemon rejecting the envelope is a permanent mismatch, not a down
/// daemon: `isError` with its own code, distinct from `Unavailable`.
#[test]
fn a_rejected_request_is_a_typed_error_distinct_from_unavailable() {
    let server = answering(encode_failure(
        "eureka-state-call",
        "view",
        &OperationFailure { code: "wrong-service".into(), message: "not huginn.mind".into() },
        "scripted",
    ));
    let mut mcp = Mcp::at(INSTANCE, server.addr);
    let (error, body) = mcp.call("view", json!({ "id": absent() }));
    assert!(error);
    assert_eq!(body["error"], json!("Rejected"));
    assert_eq!(body["code"], json!("wrong-service"));
    assert_eq!(body["detail"], json!("not huginn.mind"));
    assert_eq!(body["endpoint"], json!(format!("rudp://{}", server.addr)));
}

/// A `whoami` answered with another operation's payload is not a mind's
/// status: it is reported, not called reachable.
#[test]
fn whoami_does_not_take_another_answer_for_a_status() {
    let server = answering(encode_response("eureka-state-call", "whoami", &HuginnMindResponse::View(None), "scripted").unwrap());
    let mut mcp = Mcp::at(INSTANCE, server.addr);
    let (error, body) = mcp.call("whoami", json!({}));
    assert!(!error);
    assert_eq!(body["reachable"], json!(false));
    assert_eq!(body["status"], Value::Null);
    assert!(body["error"]["detail"].as_str().unwrap().contains("unexpected answer"), "{body}");
}

/// Bad configuration does not stop the server. `whoami` says what is wrong and
/// every other tool refuses with the same typed body.
#[test]
fn bad_configuration_is_served_and_reported() {
    let cases: [(&[(&str, &str)], &str); 5] = [
        (&[("HUGINN_ENDPOINT", "rudp://127.0.0.1:1")], "EUREKA_INSTANCE is not set"),
        (&[("EUREKA_INSTANCE", "Not A Slug"), ("HUGINN_ENDPOINT", "rudp://127.0.0.1:1")], "EUREKA_INSTANCE is not a valid"),
        (&[("EUREKA_INSTANCE", "eureka")], "HUGINN_ENDPOINT is not set"),
        (&[("EUREKA_INSTANCE", "eureka"), ("HUGINN_ENDPOINT", "127.0.0.1:1")], "HUGINN_ENDPOINT is not rudp://"),
        (&[("EUREKA_INSTANCE", "eureka"), ("HUGINN_ENDPOINT", "rudp://not-an-address")], "HUGINN_ENDPOINT is not rudp://"),
    ];
    for (env, expected) in cases {
        let mut mcp = Mcp::start(env);
        let (error, whoami) = mcp.call("whoami", json!({}));
        assert!(!error, "{expected}");
        assert_eq!(whoami["reachable"], json!(false), "{expected}");
        assert_eq!(whoami["error"]["error"], json!("Misconfigured"), "{expected}");
        assert!(whoami["error"]["detail"].as_str().unwrap().contains(expected), "{expected}: {whoami}");
        for (tool, arguments) in [
            ("view", json!({ "id": absent() })),
            ("query", json!({ "selection": {} })),
            ("admit", faculty_args(vec![])),
        ] {
            let (error, refusal) = mcp.call(tool, arguments);
            assert!(error, "{tool}: {expected}");
            assert_eq!(refusal["error"], json!("Misconfigured"), "{tool}");
        }
    }
}

/// Input that does not fit the tool's schema is refused before any call is
/// made. rmcp 2.2 reports it as a tool result with `isError`, not as a
/// JSON-RPC `invalid_params`, so the model sees what to correct. The server
/// keeps serving.
#[test]
fn malformed_input_is_refused_before_any_call() {
    let server = answering(encode_response("eureka-state-call", "admit", &HuginnMindResponse::View(None), "scripted").unwrap());
    let mut mcp = Mcp::at(INSTANCE, server.addr);
    let response = mcp.rpc("tools/call", json!({ "name": "admit", "arguments": { "faculty": "Hands" } }));
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["isError"], json!(true));
    assert!(response["result"]["content"][0]["text"].as_str().unwrap().contains("missing field"), "{response}");
    let (error, _) = mcp.call("whoami", json!({}));
    assert!(!error);
}

/// A well-formed id of a document no mind here holds.
fn absent() -> Value {
    serde_json::to_value(PipelineRef { kind: PipelineKind::CutSpec, id: Short("eureka-state:cut_spec:cut-99.r1".into()) })
        .unwrap()
}

/// A client only lists tools from a server that says it has them.
#[test]
fn initialize_advertises_tools() {
    let mcp = Mcp::at(INSTANCE, closed_port());
    assert!(mcp.init["capabilities"]["tools"].is_object(), "{}", mcp.init);
}
