//! The wire against the connector's golden frames, and the client against a
//! fake connector that speaks them over a real unix socket with real SCM_RIGHTS.

use super::*;

const REQUEST: &[u8] = include_bytes!("lease_v1/request.bin");
const GRANT: &[u8] = include_bytes!("lease_v1/grant.bin");
const REFUSAL: &[u8] = include_bytes!("lease_v1/refusal.bin");
const SETTLE: &[u8] = include_bytes!("lease_v1/settle.bin");
const SETTLED: &[u8] = include_bytes!("lease_v1/settled.bin");

const RESET: u64 = 1_793_307_000_000;

fn request() -> LeaseRequest {
    LeaseRequest { expires_at_unix_ms: RESET }
}

const LEASE_ID: &str = "lease:0123456789abcdef0123456789abcdef";

fn settlement() -> Settlement {
    Settlement {
        usage: vec![
            ModelUsage { model: "claude-opus-5-1".into(), usage: TokenUsage { input_tokens: 1200, cache_read_tokens: 30_000, cache_write_tokens: 500, output_tokens: 800, reasoning_tokens: Some(100) } },
            ModelUsage { model: "claude-haiku-5-1".into(), usage: TokenUsage { input_tokens: 400, cache_read_tokens: 0, cache_write_tokens: 0, output_tokens: 60, reasoning_tokens: None } },
        ],
    }
}

fn settle_message() -> LeaseMessage {
    LeaseMessage::Settle(LeaseSettle { lease_id: LEASE_ID.into(), usage: settlement().usage })
}

#[test]
fn lease_fixtures_decode() {
    let refusal = LeaseRefused { kind: LeaseRefusalKind::SpendCeiling, detail: "the window cannot hold this reservation".into() };
    let cases: Vec<(&str, &[u8], LeaseMessage)> = vec![
        ("request", REQUEST, LeaseMessage::Request(request())),
        ("grant", GRANT, LeaseMessage::Grant(LeaseGranted { lease_id: LEASE_ID.into(), reserved_micro_usd: 600_000 })),
        ("refusal", REFUSAL, LeaseMessage::Refusal(refusal)),
        ("settle", SETTLE, settle_message()),
        ("settled", SETTLED, LeaseMessage::Settled(LeaseSettled { billed_micro_usd: 123_456, overrun_micro_usd: 23_456 })),
    ];
    for (name, bytes, message) in cases {
        assert_eq!(decode_frame(bytes).as_ref(), Some(&message), "{name} decodes");
        assert_eq!(encode_frame(&message).unwrap(), bytes, "{name} re-encodes to the connector's bytes");
    }
}

#[test]
fn a_frame_is_one_whole_message_within_the_bound() {
    let mut trailing = GRANT.to_vec();
    trailing.push(0);
    assert!(decode_frame(&trailing).is_none());
    assert!(decode_frame(&GRANT[..GRANT.len() - 1]).is_none());
    assert!(decode_frame(&[0, 0, 0, 0]).is_none());
    assert!(decode_frame(&[0, 0]).is_none());
    let mut oversize = vec![0_u8; 4 + MAX_LEASE_FRAME_BYTES + 1];
    oversize[..4].copy_from_slice(&u32::try_from(MAX_LEASE_FRAME_BYTES + 1).unwrap().to_be_bytes());
    assert!(decode_frame(&oversize).is_none());
    let too_big = LeaseMessage::Refusal(LeaseRefused { kind: LeaseRefusalKind::Busy, detail: "x".repeat(MAX_LEASE_FRAME_BYTES) });
    assert!(encode_frame(&too_big).is_err(), "the client never sends a frame the connector would refuse");
}

#[test]
fn the_frame_bound_is_the_connectors_65536_bytes() {
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
    fn a_lease_round_trips_with_the_token_on_a_close_on_exec_descriptor_and_the_settle_in_the_connectors_bytes() {
        let (_dir, lender, served) = connector(|mut stream| {
            assert_eq!(read_frame(&mut stream), REQUEST, "the request is the connector's golden bytes");
            send_with(&stream, GRANT, &[pipe_holding("tok-canary-1").as_fd()]);
            assert_eq!(read_frame(&mut stream), SETTLE, "the settle is the connector's golden bytes");
            stream.write_all(SETTLED).unwrap();
        });
        let Lent::Granted(lease) = lender.lend(&request()).unwrap() else { panic!("refused") };
        // Close-on-exec: nothing the actuator starts inherits the lease's own descriptor.
        assert!(fcntl_getfd(&lease.token().0).unwrap().contains(FdFlags::CLOEXEC));
        // The duplicate for the child survives exec, sits above stdio and holds the token once.
        let inheritable = lease.token().inheritable().unwrap();
        assert!(inheritable.number() >= 3 && !fcntl_getfd(&inheritable.0).unwrap().contains(FdFlags::CLOEXEC));
        let mut text = String::new();
        std::fs::File::from(rustix::io::dup(&inheritable.0).unwrap()).read_to_string(&mut text).unwrap();
        assert_eq!(text, "tok-canary-1");
        assert_eq!(lease.settle(settlement()).unwrap(), 123_456);
        served.join().unwrap();
    }

    #[test]
    fn a_refusal_is_an_answer_with_its_kind_and_no_descriptor() {
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            stream.write_all(REFUSAL).unwrap();
        });
        let Lent::Refused(refused) = lender.lend(&request()).unwrap() else { panic!("granted") };
        assert_eq!(refused.kind, LeaseRefusalKind::SpendCeiling);
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
                send_with(&stream, GRANT, &borrowed);
            });
            assert!(lender.lend(&request()).is_err(), "{fds} descriptors");
            served.join().unwrap();
        }
    }

    #[test]
    fn a_lease_dropped_without_settling_sends_no_settle_and_closes_the_connection() {
        // A run killed by the brake never settles: the connector sees the connection end with no settle,
        // which it bills as the whole reservation.
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            send_with(&stream, GRANT, &[pipe_holding("tok-canary-2").as_fd()]);
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).unwrap();
            rest
        });
        let Lent::Granted(lease) = lender.lend(&request()).unwrap() else { panic!("refused") };
        drop(lease);
        assert!(served.join().unwrap().is_empty(), "nothing was sent after the request");
    }

    #[test]
    fn a_connector_that_is_not_there_or_answers_wrongly_is_an_error_not_a_refusal() {
        let dir = TempDir::new().unwrap();
        assert!(SocketLender::new(dir.path().join("none.sock")).lend(&request()).is_err());
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            stream.write_all(SETTLED).unwrap();
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
    fn a_refusal_or_settled_answer_that_carries_a_descriptor_is_an_error() {
        // A refusal lends nothing: a descriptor with one is a connector fault, and is closed, not used.
        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            send_with(&stream, REFUSAL, &[pipe_holding("tok-canary-3").as_fd()]);
        });
        assert!(lender.lend(&request()).is_err());
        served.join().unwrap();

        let (_dir, lender, served) = connector(|mut stream| {
            read_frame(&mut stream);
            send_with(&stream, GRANT, &[pipe_holding("tok-canary-4").as_fd()]);
            read_frame(&mut stream);
            send_with(&stream, SETTLED, &[pipe_holding("tok-canary-5").as_fd()]);
        });
        let Lent::Granted(lease) = lender.lend(&request()).unwrap() else { panic!("refused") };
        assert!(lease.settle(settlement()).is_err());
        served.join().unwrap();
    }
}
