//! The lease wire (v2: no reservation, no settle) and the client against a fake
//! connector that speaks it over a real unix socket with real SCM_RIGHTS.

use super::*;

const RESET: u64 = 1_793_307_000_000;

fn request() -> LeaseRequest {
    LeaseRequest { expires_at_unix_ms: RESET }
}

const LEASE_ID: &str = "lease:0123456789abcdef0123456789abcdef";

fn window() -> PlanQuota {
    PlanQuota { window: "five_hour".into(), utilization_permille: Some(80), resets_at_unix_ms: RESET, status: "allowed".into(), observed_at_unix_ms: RESET - 1000 }
}

fn refusal_message() -> LeaseMessage {
    LeaseMessage::Refusal(LeaseRefused { kind: LeaseRefusalKind::ProviderRejected, detail: "the provider refuses until its reset".into() })
}

fn request_frame() -> Vec<u8> {
    encode_frame(&LeaseMessage::Request(request())).unwrap()
}

fn grant_frame() -> Vec<u8> {
    encode_frame(&LeaseMessage::Grant(LeaseGranted { lease_id: LEASE_ID.into(), plan_quota: vec![window()] })).unwrap()
}

fn refusal_frame() -> Vec<u8> {
    encode_frame(&refusal_message()).unwrap()
}

/// The connector names on the wire, spelled out: a renamed field or kind is a
/// different protocol, so the frames are read back as plain JSON values.
#[test]
fn the_wire_names_are_the_connector_lease_v2_names() {
    let body = |frame: &[u8]| -> serde_json::Value { rmp_serde::from_slice(&frame[4..]).unwrap() };
    assert_eq!(body(&request_frame()), serde_json::json!({"kind": "claude_lease_request", "value": {"expires_at_unix_ms": RESET}}));
    assert_eq!(
        body(&grant_frame()),
        serde_json::json!({"kind": "claude_lease_grant", "value": {"lease_id": LEASE_ID, "plan_quota": [{
            "window": "five_hour", "utilization_permille": 80, "resets_at_unix_ms": RESET, "status": "allowed", "observed_at_unix_ms": RESET - 1000
        }]}})
    );
    assert_eq!(body(&refusal_frame())["kind"], "claude_lease_refusal");
    assert_eq!(body(&refusal_frame())["value"]["kind"], "ProviderRejected");
    for (name, kind) in [("UnknownCaller", LeaseRefusalKind::UnknownCaller), ("Expired", LeaseRefusalKind::Expired), ("Busy", LeaseRefusalKind::Busy)] {
        let frame = encode_frame(&LeaseMessage::Refusal(LeaseRefused { kind, detail: String::new() })).unwrap();
        assert_eq!(body(&frame)["value"]["kind"], name);
    }
}

#[test]
fn a_v1_settle_or_settled_frame_is_not_a_message() {
    for kind in ["claude_lease_settle", "claude_lease_settled"] {
        let body = rmp_serde::to_vec_named(&serde_json::json!({"kind": kind, "value": {}})).unwrap();
        assert!(decode_frame_body(&body).is_err(), "{kind}");
    }
}

#[test]
fn a_frame_is_one_whole_message_within_the_bound() {
    let grant = grant_frame();
    let mut trailing = grant.clone();
    trailing.push(0);
    assert!(decode_frame(&trailing).is_none());
    assert!(decode_frame(&grant[..grant.len() - 1]).is_none());
    assert!(decode_frame(&grant).is_some());
    assert!(decode_frame(&[0, 0, 0, 0]).is_none());
    assert!(decode_frame(&[0, 0]).is_none());
    let mut oversize = vec![0_u8; 4 + MAX_LEASE_FRAME_BYTES + 1];
    oversize[..4].copy_from_slice(&u32::try_from(MAX_LEASE_FRAME_BYTES + 1).unwrap().to_be_bytes());
    assert!(decode_frame(&oversize).is_none());
    let too_big = LeaseMessage::Refusal(LeaseRefused { kind: LeaseRefusalKind::Busy, detail: "x".repeat(MAX_LEASE_FRAME_BYTES) });
    assert!(encode_frame(&too_big).is_err(), "the client never sends a frame the connector would refuse");
}

#[test]
fn the_frame_bound_is_the_connector_65536_bytes() {
    assert_eq!(MAX_LEASE_FRAME_BYTES, 65_536);
}

/// A refusal whose MessagePack body is exactly `body_len` bytes.
fn refusal_with_body_of(body_len: usize) -> LeaseMessage {
    let refusal = |filler: usize| LeaseMessage::Refusal(LeaseRefused { kind: LeaseRefusalKind::Busy, detail: "x".repeat(filler) });
    let overhead = rmp_serde::to_vec_named(&refusal(0)).unwrap().len();
    // The string header grows with the filler, so walk to the length instead of computing it.
    let message = (body_len.saturating_sub(overhead + 8)..body_len).map(refusal).find(|message| rmp_serde::to_vec_named(message).unwrap().len() == body_len);
    message.expect("a refusal of exactly that length exists")
}

#[test]
fn a_frame_of_exactly_the_bound_is_sent_and_one_byte_more_is_not() {
    let exact = encode_frame(&refusal_with_body_of(MAX_LEASE_FRAME_BYTES)).unwrap();
    assert_eq!(exact.len(), 4 + MAX_LEASE_FRAME_BYTES);
    assert_eq!(decode_frame(&exact).map(|_| ()), Some(()), "and it reads back");
    assert!(encode_frame(&refusal_with_body_of(MAX_LEASE_FRAME_BYTES + 1)).is_err());
}

#[cfg(unix)]
mod socket_wire {
    use std::io::{IoSlice, Read, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::{AsFd, BorrowedFd};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::thread::JoinHandle;

    use rustix::io::{FdFlags, fcntl_getfd};
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
    use tempfile::TempDir;

    use super::*;

    /// A fake connector: it accepts one connection and runs `script` on it.
    fn connector<T: Send + 'static>(script: impl FnOnce(UnixStream) -> T + Send + 'static) -> (TempDir, SocketLender, JoinHandle<T>) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("lease.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let served = std::thread::spawn(move || script(listener.accept().unwrap().0));
        (dir, SocketLender::new(path), served)
    }

    fn read_frame(stream: &mut UnixStream) -> Vec<u8> {
        let mut length = [0_u8; 4];
        stream.read_exact(&mut length).unwrap();
        let mut frame = length.to_vec();
        frame.resize(4 + usize::try_from(u32::from_be_bytes(length)).unwrap(), 0);
        stream.read_exact(&mut frame[4..]).unwrap();
        frame
    }

    fn send_with(stream: &UnixStream, frame: &[u8], fds: &[BorrowedFd<'_>]) {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() {
            assert!(control.push(SendAncillaryMessage::ScmRights(fds)));
        }
        sendmsg(stream, &[IoSlice::new(frame)], &mut control, SendFlags::empty()).unwrap();
    }

    /// The read end of a pipe holding `text`, write end closed, as the connector lends it.
    fn pipe_holding(text: &str) -> std::os::fd::OwnedFd {
        let (read, mut write) = std::io::pipe().unwrap();
        write.write_all(text.as_bytes()).unwrap();
        read.into()
    }

    #[test]
    fn a_lease_round_trips_with_the_token_on_a_close_on_exec_descriptor_and_the_plan_windows() {
        let (_dir, lender, served) = connector(|mut stream| {
            assert_eq!(read_frame(&mut stream), request_frame(), "the request is the connector bytes");
            send_with(&stream, &grant_frame(), &[pipe_holding("tok-canary-1").as_fd()]);
            // The lease lasts while the connection is open: it ends when the holder drops it.
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).unwrap();
            rest
        });
        let Lent::Granted(lease) = lender.lend(&request()).unwrap() else { panic!("refused") };
        assert_eq!(lease.plan_quota(), [window()]);
        // Close-on-exec: nothing the actuator starts inherits the lease own descriptor.
        assert!(fcntl_getfd(&lease.token().0).unwrap().contains(FdFlags::CLOEXEC));
        // The duplicate for the child survives exec, sits above stdio and holds the token once.
        let inheritable = lease.token().inheritable().unwrap();
        assert!(inheritable.number() >= 3 && !fcntl_getfd(&inheritable.0).unwrap().contains(FdFlags::CLOEXEC));
        let mut text = String::new();
        std::fs::File::from(rustix::io::dup(&inheritable.0).unwrap()).read_to_string(&mut text).unwrap();
        assert_eq!(text, "tok-canary-1");
        drop(lease);
        assert!(served.join().unwrap().is_empty(), "a lease sends nothing after the request: no settle exists");
    }

    #[test]
    fn a_refusal_is_an_answer_with_its_kind_and_no_descriptor() {
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            stream.write_all(&refusal_frame()).unwrap();
        });
        let Lent::Refused(refused) = lender.lend(&request()).unwrap() else { panic!("granted") };
        assert_eq!(refused.kind, LeaseRefusalKind::ProviderRejected);
        served.join().unwrap();
    }

    /// What a connector sends for a frame length: the length, then `body`.
    fn framed(length: u32, body: &[u8]) -> Vec<u8> {
        let mut frame = length.to_be_bytes().to_vec();
        frame.extend_from_slice(body);
        frame
    }

    #[test]
    fn the_receive_bound_refuses_zero_and_one_past_and_takes_exactly_the_bound() {
        let outside = |frame: Vec<u8>| {
            let (_dir, lender, served) = connector(move |mut stream| {
                read_frame(&mut stream);
                // Enough body that only the length can be what is wrong with the frame.
                // The client may refuse on the length and hang up before the body is written: that is no failure of the connector.
                let _ = stream.write_all(&frame);
            });
            let error = lender.lend(&request()).err().map(|error| format!("{error:#}"));
            served.join().unwrap();
            error
        };
        let past = u32::try_from(MAX_LEASE_FRAME_BYTES + 1).unwrap();
        assert!(outside(framed(0, &[0x80; 8])).unwrap().contains("outside its bound"));
        assert!(outside(framed(past, &vec![0x80; MAX_LEASE_FRAME_BYTES + 1])).unwrap().contains("outside its bound"));

        let exact = encode_frame(&refusal_with_body_of(MAX_LEASE_FRAME_BYTES)).unwrap();
        let (_dir, lender, served) = connector(move |mut stream| {
            read_frame(&mut stream);
            stream.write_all(&exact).unwrap();
        });
        let Lent::Refused(refused) = lender.lend(&request()).unwrap() else { panic!("granted") };
        assert_eq!(refused.kind, LeaseRefusalKind::Busy);
        served.join().unwrap();
    }

    #[test]
    fn a_grant_without_exactly_one_descriptor_is_an_error() {
        for fds in [0, 2] {
            let (_dir, lender, served) = connector(move |mut stream| {
                read_frame(&mut stream);
                let held: Vec<_> = (0..fds).map(|_| pipe_holding("tok")).collect();
                let borrowed: Vec<_> = held.iter().map(AsFd::as_fd).collect();
                send_with(&stream, &grant_frame(), &borrowed);
            });
            assert!(lender.lend(&request()).is_err(), "{fds} descriptors");
            served.join().unwrap();
        }
    }

    #[test]
    fn a_connector_that_is_not_there_or_answers_wrongly_is_an_error_not_a_refusal() {
        let dir = TempDir::new().unwrap();
        assert!(SocketLender::new(dir.path().join("none.sock")).lend(&request()).is_err());
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            stream.write_all(&request_frame()).unwrap();
        });
        assert!(lender.lend(&request()).is_err());
        served.join().unwrap();
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            // A frame length past the bound.
            stream.write_all(&u32::MAX.to_be_bytes()).unwrap();
        });
        assert!(lender.lend(&request()).is_err());
        served.join().unwrap();
    }

    #[test]
    fn a_refusal_that_carries_a_descriptor_is_an_error() {
        // A refusal lends nothing: a descriptor with one is a connector fault, and is closed, not used.
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            send_with(&stream, &refusal_frame(), &[pipe_holding("tok-canary-3").as_fd()]);
        });
        assert!(lender.lend(&request()).is_err());
        served.join().unwrap();
    }
}
