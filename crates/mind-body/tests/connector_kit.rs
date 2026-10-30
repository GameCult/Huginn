//! The connector transport against the kit's real server: `serve` over a real
//! replay store and caller admission, with a fake provider behind it. What is
//! faked is the model; the contract, the sealing, the admission, the ledger and
//! the spend readout are the kit's.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use mind_body::heartbeat::Models;
use rust_decimal::Decimal;
use mind_body::connector::{KitConnector, endpoint, key};
use mind_body::persona::{Connector, TurnRequest};
use model_connector::{
    CallerAdmission, ModelPrice, PlanQuota, PlanQuotaSnapshot, PriceTable, ProviderBackend, ProviderCall, ProviderRequest, Rates, ServeLimits, SpendCeiling, SpendWindowMs,
    StopReason, TransportEvent, TransportEventPayload, TransportOutcome, TransportResult, Usage, serve,
};

const CALLER: &str = "eureka";
const MODEL: &str = "claude-test-model";
const KEY: &str = "connector-key-for-test";
const HEARTBEAT: &str = "heartbeat";
const HEARTBEAT_KEY: &str = "the-heartbeats-own-key";

enum Behaviour {
    Reply(&'static str),
    Stop(StopReason),
    Fail,
}

struct FakeProvider {
    behaviour: Behaviour,
    seen: Mutex<Vec<ProviderRequest>>,
}

impl ProviderBackend for FakeProvider {
    fn ready(&self) -> Result<(), &'static str> {
        Ok(())
    }

    fn provider(&self) -> &str {
        "fake-provider"
    }

    fn plan_quota(&self) -> PlanQuotaSnapshot {
        PlanQuotaSnapshot {
            windows: vec![
                PlanQuota { window: "five_hour".into(), utilization_permille: Some(423), resets_at_unix_ms: 9, status: "allowed".into(), observed_at_unix_ms: 5 },
                PlanQuota { window: "seven_day".into(), utilization_permille: None, resets_at_unix_ms: 1, status: "expired".into(), observed_at_unix_ms: 2 },
            ],
        }
    }

    fn execute(&self, call: &ProviderCall) -> TransportResult {
        self.seen.lock().unwrap().push(call.request().clone());
        let usage = Usage { input_tokens: 10, cache_read_tokens: 0, cache_write_tokens: 0, output_tokens: 3, reasoning_tokens: None };
        match &self.behaviour {
            Behaviour::Reply(text) => call.transported(
                vec![TransportEvent { sequence: 0, payload: TransportEventPayload::TextDelta { text: (*text).to_string() } }],
                TransportOutcome::Completed { provider_response_id: None, stop_reason: StopReason::EndTurn, usage },
            ),
            Behaviour::Stop(stop_reason) => call.transported(
                vec![TransportEvent { sequence: 0, payload: TransportEventPayload::TextDelta { text: "cut".into() } }],
                TransportOutcome::Completed { provider_response_id: None, stop_reason: *stop_reason, usage },
            ),
            Behaviour::Fail => call.transported(
                vec![],
                TransportOutcome::Failed { kind: model_connector::FailureKind::Unavailable, message: "down".into(), usage: Some(usage) },
            ),
        }
    }
}

struct Daemon {
    endpoint: String,
    provider: Arc<FakeProvider>,
    _store: tempfile::TempDir,
}

fn daemon(behaviour: Behaviour) -> Daemon {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let store = tempfile::tempdir().unwrap();
    let replay = store.path().join("replay.cc");
    let rate = Rates { input: 1_000_000, cache_write_short: 1_000_000, cache_write_long: 1_000_000, cache_read: 1_000_000, output: 1_000_000 };
    let prices = PriceTable::from([(MODEL.to_string(), ModelPrice { rates: rate, long_context: None, overhead_tokens: 0 })]);
    let admission =
        CallerAdmission::new(CALLER, key(KEY), 1, [MODEL.to_string()], 2, 64 * 1024, 8192, SpendCeiling(1 << 40), SpendWindowMs(3_600_000)).unwrap();
    let observer =
        CallerAdmission::new(HEARTBEAT, key(HEARTBEAT_KEY), 1, [MODEL.to_string()], 2, 64 * 1024, 8192, SpendCeiling(1 << 40), SpendWindowMs(3_600_000)).unwrap();
    let limits = ServeLimits { max_frame_bytes: 256 * 1024, max_connections: 8, socket_timeout_ms: 5_000, max_expiry_skew_ms: 300_000 };
    let provider = Arc::new(FakeProvider { behaviour, seen: Mutex::default() });
    let backend = provider.clone();
    std::thread::spawn(move || {
        let _ = serve(listener, limits, vec![admission, observer], prices, &replay, backend);
    });
    Daemon { endpoint, provider, _store: store }
}

fn turn_request() -> TurnRequest {
    TurnRequest {
        caller: CALLER.into(),
        request_id: "run1-t0".into(),
        max_cost_micro_usd: 5_000_000,
        instructions: "You are Eureka.".into(),
        input: "Rooms you may speak in: operator.".into(),
        output_schema_json: r#"{"type":"object"}"#.into(),
    }
}

fn connector(daemon: &Daemon, model: &str) -> KitConnector {
    KitConnector::new(endpoint(&daemon.endpoint).unwrap(), &format!("{KEY}\n"), model, CALLER).unwrap()
}

#[test]
fn a_turn_is_one_tool_less_schema_constrained_call_and_its_answer_is_the_assistant_text() {
    let daemon = daemon(Behaviour::Reply(r#"{"say":[],"start_self_run":null}"#));
    let text = connector(&daemon, MODEL).turn(&turn_request()).unwrap();
    assert_eq!(text, r#"{"say":[],"start_self_run":null}"#);
    let seen = daemon.provider.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let request = &seen[0];
    assert_eq!((request.request_id.as_str(), request.conversation_id.as_str(), request.model.as_str()), ("run1-t0", CALLER, MODEL));
    assert_eq!(request.instructions, "You are Eureka.");
    assert_eq!(request.input, vec![model_connector::InputItem::UserText { text: "Rooms you may speak in: operator.".into() }]);
    assert_eq!(request.output_schema_json.as_deref(), Some(r#"{"type":"object"}"#));
    assert!(request.tools.is_empty());
}

#[test]
fn the_spend_readout_is_the_kits_for_the_caller_after_a_billed_turn() {
    let daemon = daemon(Behaviour::Reply("{}"));
    let connector = connector(&daemon, MODEL);
    connector.turn(&turn_request()).unwrap();
    let readout = connector.spend_readout().unwrap();
    let row = readout.callers.iter().find(|row| row.caller_id == CALLER).expect("the caller's row");
    assert_eq!(row.spent_micro_usd, 13, "10 input and 3 output tokens at 1 USD per million each");
    assert_eq!(row.ceiling_micro_usd, 1 << 40);
    let windows: Vec<_> = readout.plan_quota.iter().map(|plan| (plan.window.as_str(), plan.utilization_permille, plan.status.as_str())).collect();
    assert_eq!(windows, [("five_hour", Some(423), "allowed"), ("seven_day", None, "expired")]);
}

#[test]
fn a_turn_the_connector_refuses_or_the_provider_fails_or_stops_early_is_an_error() {
    let refused = daemon(Behaviour::Reply("{}"));
    assert!(connector(&refused, "a-model-she-is-not-allowed").turn(&turn_request()).is_err());
    assert!(refused.provider.seen.lock().unwrap().is_empty(), "a refused call reaches no provider");
    assert!(connector(&daemon(Behaviour::Fail), MODEL).turn(&turn_request()).is_err());
    assert!(connector(&daemon(Behaviour::Stop(StopReason::MaxOutputTokens)), MODEL).turn(&turn_request()).is_err(), "a cut-off answer is not an answer");
}

#[test]
fn a_wrong_connection_key_gets_nothing() {
    let daemon = daemon(Behaviour::Reply("{}"));
    let stranger = KitConnector::new(endpoint(&daemon.endpoint).unwrap(), "not-the-key", MODEL, CALLER).unwrap();
    assert!(stranger.turn(&turn_request()).is_err());
    assert!(stranger.spend_readout().is_err());
    assert!(daemon.provider.seen.lock().unwrap().is_empty());
}

#[test]
fn endpoints_are_the_connectors_loopback_tcp_address_and_a_rudp_url_is_refused() {
    assert_eq!(endpoint("127.0.0.1:4110").unwrap().port(), 4110);
    assert_eq!(endpoint("tcp://127.0.0.1:4110").unwrap().port(), 4110);
    for bad in ["rudp://127.0.0.1:4110", "localhost:4110", "127.0.0.1", ""] {
        assert!(endpoint(bad).is_err(), "{bad}");
    }
    assert_eq!(key("secret\r\n"), "secret");
    assert_eq!(key("a b\n\n"), "a b");
}

#[test]
fn the_heartbeat_reads_a_personas_spend_as_itself_and_picks_her_row() {
    let daemon = daemon(Behaviour::Reply("{}"));
    connector(&daemon, MODEL).turn(&turn_request()).unwrap();
    let heartbeat = KitConnector::new(endpoint(&daemon.endpoint).unwrap(), HEARTBEAT_KEY, MODEL, HEARTBEAT).unwrap();
    let spent = Models::spent(&heartbeat, CALLER).expect("a persona's row, asked under the heartbeat's own key");
    assert_eq!(spent.usd, Decimal::new(13, 6));
    assert_eq!(Models::spent(&heartbeat, HEARTBEAT).unwrap().usd, Decimal::ZERO, "its own row is there too");
    assert!(Models::spent(&heartbeat, "nobody").is_err(), "a caller with no row is an error, not zero");
}
