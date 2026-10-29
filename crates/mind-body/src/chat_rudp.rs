//! The Persona organ's chat transport: CultNet RUDP to Bifrost's Matrix mouth.
//!
//! The mouth (Bifrost `tools/persona-matrix-mouth.mjs`, `persona-chat-documents.mjs`)
//! listens on loopback as a RUDP server. Its connection id is derived from the
//! chat credential, so a peer without the credential cannot open the session:
//! the first four bytes of the SHA-256 of the key file's text, trimmed, read big
//! endian (`connection_id`). Each request is a `cultnet.document_put_raw.v0`
//! message on the `schema` channel carrying one `chat_client` document as a
//! positional MessagePack tuple; the mouth answers a pending request with a
//! pending document and a send with a receipt, and answers an ack with nothing.
//! A request it rejects (another Persona's name, a malformed tuple) gets no
//! answer either, so every wait here is bounded by `reply_timeout`.
//!
//! The session opens on the first call and closes when `RudpChat` drops; a
//! start that dies between a send and its ack repeats the send with the same
//! request id, which the mouth treats as the same send (`chat_client`).

use std::cell::RefCell;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use cultnet_rs::{
    CultNetMessage, CultNetRawDocumentRecord, CultNetRawPayloadEncoding, CultNetRudpSocketTransportConnection, CultNetRudpSocketTransportOptions,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::chat_client::{
    ACK, Chat, ChatAck, ChatDelivery, ChatPending, ChatPendingRequest, ChatSendReceipt, ChatSendRequest, PENDING, PENDING_REQUEST, SEND_RECEIPT, SEND_REQUEST, decode,
    encode,
};

/// The tag every chat document carries; the mouth refuses any other.
const TAG: &str = "cultnet.transport.rudp.v0";
/// The source role the mouth sees on a request.
const SOURCE_ROLE: &str = "persona-organ";
/// How long one datagram read waits before the loop polls resends.
const POLL: Duration = Duration::from_millis(20);
/// How soon an unacknowledged packet is sent again.
const RESEND_DELAY_MS: u64 = 50;

/// The mouth's connection id for a chat credential: `sha256(key)` first four
/// bytes, big endian, where `key` is the credential file's text trimmed of
/// surrounding whitespace as the mouth reads it.
pub fn connection_id(key_file_text: &str) -> u32 {
    let digest = Sha256::digest(key_file_text.trim().as_bytes());
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]])
}

struct Link {
    transport: CultNetRudpSocketTransportConnection,
    runtime_id: String,
}

/// `Chat` over one RUDP session to the mouth of one Persona.
pub struct RudpChat {
    remote: SocketAddr,
    connection_id: u32,
    persona: String,
    reply_timeout: Duration,
    link: RefCell<Option<Link>>,
}

impl RudpChat {
    /// `remote` is the mouth's loopback address, `connection_id` the credential's
    /// (`connection_id`), `persona` the instance the mouth serves. Nothing is
    /// sent until the first call.
    pub fn new(remote: SocketAddr, connection_id: u32, persona: impl Into<String>, reply_timeout: Duration) -> Self {
        Self { remote, connection_id, persona: persona.into(), reply_timeout, link: RefCell::new(None) }
    }

    fn open(&self) -> Result<Link> {
        let bind: SocketAddr = if self.remote.is_ipv4() { ([0, 0, 0, 0], 0).into() } else { ([0_u16; 8], 0).into() };
        let socket = UdpSocket::bind(bind).context("the chat socket could not be opened")?;
        socket.set_read_timeout(Some(POLL))?;
        let runtime_id = format!("mind-persona-{}", self.persona);
        let mut options = CultNetRudpSocketTransportOptions::client(runtime_id.clone(), socket, self.remote, self.connection_id);
        options.resend_delay_ms = RESEND_DELAY_MS;
        let mut transport = CultNetRudpSocketTransportConnection::new(options)?;
        transport.connect(Vec::new())?;
        let deadline = Instant::now() + self.reply_timeout;
        while !transport.connected() {
            if Instant::now() >= deadline {
                bail!("the chat mouth did not accept the session");
            }
            transport.receive_once()?;
            transport.poll_resends()?;
        }
        Ok(Link { transport, runtime_id })
    }

    /// Puts one document and, when `answer` names a kind and record key, waits for
    /// the document the mouth answers it with. Documents of other kinds or keys
    /// (a late receipt of an earlier send) are skipped.
    fn exchange<T: Serialize>(&self, kind: &str, record_key: &str, document: &T, answer: Option<(&str, &str)>) -> Result<Option<Vec<u8>>> {
        let mut slot = self.link.borrow_mut();
        if slot.is_none() {
            *slot = Some(self.open()?);
        }
        let link = slot.as_mut().ok_or_else(|| anyhow!("the chat session is not open"))?;
        let message = CultNetMessage::DocumentPutRaw {
            message_id: format!("{kind}:{record_key}"),
            document: CultNetRawDocumentRecord {
                schema_id: kind.to_string(),
                record_key: record_key.to_string(),
                stored_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                payload_encoding: CultNetRawPayloadEncoding::Messagepack,
                payload: encode(document)?,
                source_runtime_id: Some(link.runtime_id.clone()),
                source_agent_id: Some(self.persona.clone()),
                source_role: Some(SOURCE_ROLE.to_string()),
                tags: Some(vec![TAG.to_string()]),
            },
        };
        link.transport.send_schema_message(&message)?;
        let deadline = Instant::now() + self.reply_timeout;
        let Some((answer_kind, answer_key)) = answer else {
            // Nothing is answered; the ack is done when the mouth has the packet.
            let left = deadline.saturating_duration_since(Instant::now());
            link.transport.flush_reliable(left)?;
            return Ok(None);
        };
        loop {
            if Instant::now() >= deadline {
                bail!("the chat mouth did not answer {kind}");
            }
            link.transport.poll_resends()?;
            if let Some(CultNetMessage::DocumentPutRaw { document, .. }) = link.transport.receive_schema_message_once()? {
                let tagged = document.tags.as_deref() == Some(&[TAG.to_string()][..]);
                if tagged && document.schema_id == answer_kind && document.record_key == answer_key {
                    return Ok(Some(document.payload));
                }
            }
        }
    }

    fn call<Q: Serialize, A: DeserializeOwned>(&self, kind: &str, key: &str, request: &Q, answer: (&str, &str)) -> Result<A> {
        let payload = self.exchange(kind, key, request, Some(answer))?.ok_or_else(|| anyhow!("no answer payload"))?;
        decode(&payload)
    }
}

impl Chat for RudpChat {
    fn pending(&self, after_seq: u64) -> Result<Vec<ChatDelivery>> {
        let request = ChatPendingRequest { persona: self.persona.clone(), after_seq };
        let pending: ChatPending = self.call(PENDING_REQUEST, &self.persona, &request, (PENDING, &self.persona))?;
        if pending.persona != self.persona {
            bail!("the mouth answered for another Persona");
        }
        Ok(pending.deliveries)
    }

    fn send(&self, request: &ChatSendRequest) -> Result<ChatSendReceipt> {
        let receipt: ChatSendReceipt = self.call(SEND_REQUEST, &request.request_id, request, (SEND_RECEIPT, &request.request_id))?;
        if receipt.request_id != request.request_id {
            bail!("the mouth answered another send");
        }
        Ok(receipt)
    }

    fn ack(&self, through_seq: u64) -> Result<()> {
        let ack = ChatAck { persona: self.persona.clone(), through_seq };
        self.exchange(ACK, &format!("{}:{through_seq}", self.persona), &ack, None).map(|_| ())
    }
}

impl Drop for RudpChat {
    fn drop(&mut self) {
        if let Some(mut link) = self.link.borrow_mut().take() {
            let _ = link.transport.disconnect(Vec::new());
        }
    }
}
