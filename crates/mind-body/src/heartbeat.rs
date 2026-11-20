//! The heartbeat: the one process that decides which Persona wakes and when
//! (rulings `eureka-body:ruling:persona-only-heartbeat`,
//! `operator-mention-fills-initiative`).
//!
//! The beat is the configured `Config::beat_s`. When a beat falls (the beat has
//! passed since the last one) exactly one Persona wakes: `tempo::pick` over the
//! idle, released Minds, by how long each has waited since the heartbeat last
//! woke her, bent by a member addressing her and (a later cut) topics she
//! subscribed to. The operator or another Mind tagging or naming her wakes her
//! off the beat, once per message. There is no night wake until reflection and
//! consolidation return (map C4). Nothing is metered here: no meter and no model call.
//!
//! A wake starts her unit and nothing else: the heartbeat writes no mind (ruling
//! `persona-only-heartbeat`: her Mind is written only by Self sessions). It reads
//! her mind only for the names her projects go by. Busy is systemd's own: a
//! Persona whose unit is active is busy, and a start of an active unit is a no-op.
//!
//! The heartbeat owns `heartbeat.cc` and is its only writer. The brakes stay the
//! operator's and are read on every poll. The heartbeat never acks a delivery.
//!
//! A held swarm brake ends the poll before the mind or the mouth is touched.

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use cultcache_rs::{CultCache, DatabaseEntry, SingleFileMessagePackBackingStore};
use eureka_pipeline::{OrgRepo, PipelineKind, RunTurn, Slug};
use huginn_mind::PipelineFacts;

use crate::address::{Addressee, Name, is_for, project_names};
use crate::chat_client::{Basis, Chat, ChatDelivery};
use crate::control::{ControlSource, Effective, FileSource, HeldReason, SwarmEffective, read_effective, read_envelope, set_mode_world_readable};
use crate::launch::{Clock, Launcher};
use crate::queue::{MindPort, headers, in_force, of_kind};
use crate::tempo::{self, Standing};

/// How many handled operator mentions the state remembers.
const MENTIONS_KEPT: usize = 64;
/// The most pages of pending chat one poll reads for one Persona.
const PENDING_PAGES: usize = 16;
/// The beat, in seconds, when `HEARTBEAT_BEAT_S` is not set.
pub const DEFAULT_BEAT_S: u32 = 1800;

/// Who a Persona is, for the two things the heartbeat matches on: the Matrix
/// user id its tags carry, and the names a message may call it by. Each Mind is
/// its own Huginn daemon, so the entry also carries the daemon the Persona's
/// control, runs and launches are read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaId {
    pub slug: Slug,
    pub public_name: String,
    pub handle: String,
    pub matrix_id: String,
    pub huginn: SocketAddr,
}

impl PersonaId {
    /// `slug,handle,Public Name,@user:server,rudp://<ip>:<port>`.
    pub fn parse(spec: &str) -> Result<Self> {
        const SHAPE: &str =
            "a persona is slug,handle,public name,matrix id,rudp://<ip>:<port> of her Huginn";
        let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
        let [slug, handle, public_name, matrix_id, huginn] = parts.as_slice() else {
            bail!(SHAPE)
        };
        if parts.iter().any(|part| part.is_empty()) {
            bail!(SHAPE);
        }
        let huginn = huginn
            .strip_prefix("rudp://")
            .and_then(|address| address.parse::<SocketAddr>().ok())
            .ok_or_else(|| anyhow!(SHAPE))?;
        let slug = Slug((*slug).to_string());
        slug.validate_slug()
            .map_err(|_| anyhow!("a persona's slug is not a slug"))?;
        Ok(Self {
            slug,
            public_name: (*public_name).to_string(),
            handle: (*handle).to_string(),
            matrix_id: (*matrix_id).to_string(),
            huginn,
        })
    }
}

/// What the heartbeat is told that is not in the mind or the swarm store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub personas: Vec<PersonaId>,
    /// Sender ids that are the operator.
    pub operators: Vec<String>,
    /// Seconds between beats: `HEARTBEAT_BEAT_S`, else `DEFAULT_BEAT_S`.
    pub beat_s: u32,
}

impl Config {
    /// Refuses no Persona, a Persona twice, a beat of zero seconds, or an operator id that is also a Persona's: one
    /// sender is either the operator or a Mind, never both, or the wake rule could not tell which it heard.
    pub fn new(personas: Vec<PersonaId>, operators: Vec<String>, beat_s: u32) -> Result<Self> {
        if personas.is_empty() {
            bail!("no persona is configured");
        }
        if operators.iter().any(|operator| personas.iter().any(|persona| persona.matrix_id == *operator)) {
            bail!("an operator id is also a persona's matrix id");
        }
        if beat_s == 0 {
            bail!("the beat is zero seconds");
        }
        for (index, persona) in personas.iter().enumerate() {
            if personas
                .iter()
                .skip(index + 1)
                .any(|other| other.slug == persona.slug)
            {
                bail!("a persona is configured twice");
            }
        }
        Ok(Self {
            personas,
            operators,
            beat_s,
        })
    }
}

/// `heartbeat.cc`'s state: when the last beat fell, the operator messages
/// already answered with a wake (once per message, bounded), and when the
/// heartbeat last woke each Persona.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.heartbeat.state.v1", schema = "HeartbeatState")]
pub struct HeartbeatState {
    #[cultcache(key = 0)]
    pub last_beat_at: DateTime<Utc>,
    /// `<instance>:<event id>` of each operator message that woke a Persona.
    #[cultcache(key = 1)]
    pub handled_mentions: Vec<String>,
    /// When the heartbeat last started each Persona's turn, by instance slug.
    #[cultcache(key = 2, default)]
    pub last_turn_at: BTreeMap<String, DateTime<Utc>>,
}

const STATE_KEY: &str = "state";

/// The swarm's addressing as the heartbeat last read it: each Mind's Matrix id and the names a delivery may call her (her own and her
/// projects'), in roster order. A cache of the stewardship records, written only by the heartbeat's poll; the Personas read it to rank
/// and show whom a delivery is for.
#[derive(Clone, Debug, Default, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.heartbeat.addressing.v1", schema = "HeartbeatAddressing")]
pub struct Addressing {
    #[cultcache(key = 0)]
    pub minds: Vec<Addressee>,
}

const ADDRESSING_KEY: &str = "addressing";

/// One instance's line in a poll's trace.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstanceTrace {
    pub instance: String,
    /// Retired: always none. Kept so the records `heartbeat.cc` holds from before the cut still decode (nothing migrates the file).
    pub vote_beat_s: Option<u32>,
    /// The urgency in force for her: 1000 when a member addresses her, else 0.
    pub urgency_permille: u16,
    /// A member's pending message addresses her.
    pub addressed: bool,
    pub busy: bool,
    pub released: bool,
    /// Seconds since her newest Persona turn; -1 when she has had none.
    pub idle_s: i64,
    /// `tempo::score` as text; empty when she was not eligible for the beat.
    pub score: String,
    pub note: String,
}

/// One poll that changed anything.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.heartbeat.poll_trace.v1", schema = "HeartbeatPollTrace")]
pub struct PollTrace {
    #[cultcache(key = 0)]
    pub at: DateTime<Utc>,
    /// The beat in force: `Config::beat_s`.
    #[cultcache(key = 1)]
    pub tempo_s: u32,
    #[cultcache(key = 2)]
    pub instances: Vec<InstanceTrace>,
    #[cultcache(key = 3)]
    pub launches: Vec<String>,
}

impl fmt::Display for PollTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "poll at {} tempo {}s", self.at.to_rfc3339_opts(SecondsFormat::Secs, true), self.tempo_s)?;
        for row in &self.instances {
            write!(
                f,
                "  {}: urgency {} addressed {} busy {} released {} idle {}s score {:?}",
                row.instance, row.urgency_permille, row.addressed, row.busy, row.released, row.idle_s, row.score
            )?;
            if !row.note.is_empty() {
                write!(f, " note {}", row.note)?;
            }
            writeln!(f)?;
        }
        for launch in &self.launches {
            writeln!(f, "  launched {launch}")?;
        }
        Ok(())
    }
}

/// `heartbeat.cc`: the state and the trace, one writer.
pub trait Store {
    fn load(&self) -> Result<Option<HeartbeatState>>;
    fn save(&self, state: &HeartbeatState) -> Result<()>;
    fn record(&self, trace: &PollTrace) -> Result<()>;
    fn load_addressing(&self) -> Result<Option<Addressing>>;
    fn save_addressing(&self, addressing: &Addressing) -> Result<()>;
}

/// What a Persona unit may read of `heartbeat.cc`: the swarm's addressing. Read-only.
pub trait HeartbeatView {
    fn addressing(&self) -> Result<Option<Addressing>>;
    /// When the store was last written, so a reader can tell a stale table; none when there is no store at all.
    fn written_at(&self) -> Option<DateTime<Utc>> {
        None
    }
}

/// The store as a CultCache single-file store under the unit's state directory.
/// It registers the state, the trace and the addressing and nothing else: a file still holding
/// any older document does not decode, and every use of it fails
/// naming the file. It is never migrated.
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn registered() -> Result<CultCache> {
        let mut cache = CultCache::new();
        cache.register_entry_type::<HeartbeatState>()?;
        cache.register_entry_type::<PollTrace>()?;
        cache.register_entry_type::<Addressing>()?;
        Ok(cache)
    }

    fn open(&self) -> Result<CultCache> {
        let mut cache = Self::registered()?;
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&self.path))?;
        cache.pull_all_backing_stores().with_context(|| self.refusal())?;
        // A pull now carries a record no registered type claims instead of refusing it; the writer still must not write beside one.
        if !cache.foreign_records().is_empty() {
            return Err(anyhow!(self.refusal()));
        }
        Ok(cache)
    }

    fn refusal(&self) -> String {
        format!("{} holds a document this heartbeat does not read (an old initiative store?): archive it, nothing migrates it", self.path.display())
    }

    /// The startup check: the file decodes, or does not exist.
    pub fn check(&self) -> Result<()> {
        self.load().map(|_| ())
    }

    fn settle(&self) -> Result<()> {
        set_mode_world_readable(&self.path)
    }
}

impl Store for FileStore {
    fn load(&self) -> Result<Option<HeartbeatState>> {
        self.snapshot()?.get::<HeartbeatState>(STATE_KEY)
    }

    fn save(&self, state: &HeartbeatState) -> Result<()> {
        self.open()?.put(STATE_KEY, state)?;
        self.settle()
    }

    fn record(&self, trace: &PollTrace) -> Result<()> {
        self.open()?.put(format!("{:020}", trace.at.timestamp_millis()), trace)?;
        self.settle()
    }

    fn load_addressing(&self) -> Result<Option<Addressing>> {
        self.snapshot()?.get::<Addressing>(ADDRESSING_KEY)
    }

    fn save_addressing(&self, addressing: &Addressing) -> Result<()> {
        self.open()?.put(ADDRESSING_KEY, addressing)?;
        self.settle()
    }
}

impl FileStore {
    /// The newest poll trace: what `mind-heartbeat timeline` prints.
    pub fn latest(&self) -> Result<Option<PollTrace>> {
        let mut all = self.snapshot()?.get_all_with_keys::<PollTrace>()?;
        all.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(all.pop().map(|(_, trace)| trace))
    }

    /// The file read without a lock and without a write, as the control stores are.
    fn snapshot(&self) -> Result<CultCache> {
        let mut cache = Self::registered()?;
        for envelope in FileSource::new(&self.path).snapshot().with_context(|| self.refusal())? {
            cache.put_raw_envelope(envelope).with_context(|| self.refusal())?;
        }
        Ok(cache)
    }
}

impl HeartbeatView for FileStore {
    fn addressing(&self) -> Result<Option<Addressing>> {
        self.load_addressing()
    }

    fn written_at(&self) -> Option<DateTime<Utc>> {
        std::fs::metadata(&self.path).and_then(|meta| meta.modified()).ok().map(DateTime::<Utc>::from)
    }
}

/// What the operator stores allow: the swarm brake, and each instance's own
/// reading (its brake and the swarm brake).
pub trait Control {
    fn envelope(&self) -> SwarmEffective;
    fn effective(&self, instance: &Slug) -> Effective;
}

/// The root-owned stores, through the one fail-closed readers.
pub struct FileControl;

impl Control for FileControl {
    fn envelope(&self) -> SwarmEffective {
        read_envelope()
    }

    fn effective(&self, instance: &Slug) -> Effective {
        read_effective(instance)
    }
}

/// One configured Persona organs: her mind and her mouth.
pub struct Seat<'a> {
    pub who: PersonaId,
    pub mind: &'a dyn MindPort,
    pub chat: &'a dyn Chat,
}

pub struct Organs<'a> {
    pub seats: &'a [Seat<'a>],
    pub control: &'a dyn Control,
    pub launcher: &'a dyn Launcher,
    pub clock: &'a dyn Clock,
    pub store: &'a dyn Store,
}

/// What one poll did. Every outcome is an answer; only a store that cannot be
/// reached is an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The swarm brake is held; nothing else was touched.
    Held(HeldReason),
    Polled {
        launched: Vec<Slug>,
    },
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held(reason) => write!(f, "held: {reason:?}"),
            Self::Polled { launched } if launched.is_empty() => f.write_str("polled: no launch"),
            Self::Polled { launched } => write!(f, "polled: launched {}", launched.iter().map(|slug| slug.0.as_str()).collect::<Vec<_>>().join(", ")),
        }
    }
}

/// Every repo she stewards in force and the chat names of those repos: one query on her mind.
pub(crate) fn stewardships_of(mind: &dyn MindPort) -> Result<(Vec<Name>, Vec<OrgRepo>)> {
    let (mut names, mut repos) = (Vec::new(), Vec::new());
    for header in headers(mind, &in_force(of_kind(PipelineKind::Stewardship)))? {
        if let PipelineFacts::Stewardship { repo, names: listed, .. } = &header.facts {
            names.extend(project_names(repo, listed));
            repos.push(repo.clone());
        }
    }
    Ok((names, repos))
}

/// A member (anyone not the operator) has a pending delivery that addresses her: the beat's urgency. The one owner of
/// that reading: the poll's rows and the simulator's beat both ask it, over the deliveries the mouth would give her.
pub(crate) fn addressed_by_member(operators: &[String], who: &Addressee, pending: &[ChatDelivery]) -> bool {
    pending.iter().any(|delivery| !operators.contains(&delivery.sender) && is_for(who, delivery))
}

/// Who sent a line, as the wake rule sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Speaker {
    Operator,
    /// Another Mind of the roster.
    Mind,
    /// A human who is not the operator: a member.
    Member,
    /// The Persona herself.
    Her,
}

impl Speaker {
    fn human(self) -> bool {
        matches!(self, Self::Operator | Self::Member)
    }
}

/// Which lines wake a Persona off the beat. `Live` is the heartbeat's rule today and the only one `poll` uses; the rest
/// are what desk question `po-mind-wakes-mind` weighs, run in `mind-persona project --simulate` until it is answered.
/// The beat is not here: every policy has it, and it wakes one Persona by `tempo::pick`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WakePolicy {
    /// The operator or another Mind addressing her (`is_for`). A member's line does not.
    Live,
    /// Ruling `persona-only-wake-humans-in-rooms`: a line addressing her, from anyone, and any human line in her rooms.
    Ruled,
    /// Ruling `persona-only-humans-pace`: a human's line wakes her in her own room (a direct message, or a room that is
    /// hers); in a public room only when it addresses her or continues an exchange she is in. A Mind's line wakes her
    /// only by an explicit mention (a Matrix mention of her or a reply to her, never a name in prose), in any room, and
    /// only while fewer than this many Mind lines in a row came before it across her rooms (`LineFacts::mind_run`): the
    /// backstop.
    HumansPace(usize),
    /// `Ruled`, with at most this many Mind lines in a row in a room before a human must speak again: a Mind's line past
    /// the cap wakes no one.
    NamesWithCap(usize),
    /// The swarm's old loop, a control: any line in her rooms but her own.
    AllMessages,
}

/// Why a line wakes her, strongest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WakeCause {
    AddressedByHuman,
    Human,
    AddressedByMind,
    /// A Mind's line that does not address her: only the control wakes on it.
    Mind,
}

/// What the room says about a line, beside the line: the facts a policy may weigh. The live poll knows none of them yet
/// and passes the default; `Live` reads none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineFacts {
    /// How many Mind lines in a row came before this one, across every room she sees, since a human last spoke in any of
    /// them: the backstop counts a Mind-only run per Mind, not per room, so a loop cannot dodge it by changing rooms.
    pub mind_run: usize,
    /// The line is in a room of hers: a direct message (the delivery says so) or a room configured as hers.
    pub own_room: bool,
    /// She has spoken in the room's current exchange: the lines before this one that follow each other without a long gap.
    pub in_exchange: bool,
}

/// Fail-safe: a caller that has not counted the Mind run is at the cap, so a Mind's line wakes no one; the room facts
/// are unknown, so none opens a wake.
impl Default for LineFacts {
    fn default() -> Self {
        Self { mind_run: usize::MAX, own_room: false, in_exchange: false }
    }
}

/// Whether a line in her rooms wakes `who` off the beat under `policy`, and why. The one owner of the wake rule: the
/// heartbeat's poll asks it with `Live`, the simulator with the policy under test.
pub(crate) fn wakes(policy: WakePolicy, who: &Addressee, delivery: &ChatDelivery, speaker: Speaker, facts: LineFacts) -> Option<WakeCause> {
    if speaker == Speaker::Her {
        return None;
    }
    let addressed = is_for(who, delivery);
    let human = speaker.human();
    let cause = match (human, addressed) {
        (true, true) => WakeCause::AddressedByHuman,
        (true, false) => WakeCause::Human,
        (false, true) => WakeCause::AddressedByMind,
        (false, false) => WakeCause::Mind,
    };
    let wakes = match policy {
        WakePolicy::Live => addressed && matches!(speaker, Speaker::Operator | Speaker::Mind),
        WakePolicy::Ruled => human || addressed,
        WakePolicy::HumansPace(cap) => {
            let explicit = delivery.mentions.iter().any(|id| id == who.matrix_id()) || matches!(delivery.basis, Some(Basis::Tag | Basis::Reply));
            if human { addressed || delivery.is_direct || facts.own_room || facts.in_exchange } else { explicit && facts.mind_run < cap }
        }
        WakePolicy::NamesWithCap(cap) => human || (addressed && facts.mind_run < cap),
        WakePolicy::AllMessages => true,
    };
    wakes.then_some(cause)
}

impl Config {
    /// Who sent a delivery, as `wakes` sees them.
    fn speaker(&self, who: &Addressee, delivery: &ChatDelivery) -> Speaker {
        if delivery.sender == who.matrix_id() {
            Speaker::Her
        } else if self.operators.contains(&delivery.sender) {
            Speaker::Operator
        } else if self.personas.iter().any(|mind| mind.matrix_id == delivery.sender) {
            Speaker::Mind
        } else {
            Speaker::Member
        }
    }
}

/// The mouth pages a pending answer, so fewer deliveries may come than wait.
/// The heartbeat acks nothing: it asks again after the highest seq it has until
/// the answer is empty. A mouth that never empties is cut off at `PENDING_PAGES`.
fn read_pending(chat: &dyn Chat) -> Result<Vec<ChatDelivery>> {
    let mut all: Vec<ChatDelivery> = Vec::new();
    let mut after = 0;
    for _ in 0..PENDING_PAGES {
        let page = chat.pending(after)?.deliveries;
        let Some(top) = page.iter().map(|delivery| delivery.seq).max() else {
            break;
        };
        if top <= after {
            break;
        }
        after = top;
        all.extend(page);
    }
    Ok(all)
}

struct Row<'a> {
    seat: &'a Seat<'a>,
    released: bool,
    /// Her unit is active.
    busy: bool,
    pending: Vec<ChatDelivery>,
    /// Whom a delivery may address in her: her roster names and her projects' names as the last poll that read her pending chat left them.
    addressee: Addressee,
    note: String,
}

impl Row<'_> {
    fn slug(&self) -> &Slug {
        &self.seat.who.slug
    }

    fn idle(&self) -> bool {
        self.released && !self.busy
    }

    /// A member (not the operator) has a pending message that names or tags her.
    fn addressed_by_member(&self, config: &Config) -> bool {
        addressed_by_member(&config.operators, &self.addressee, &self.pending)
    }
}

enum Woke {
    Started,
    Busy,
    Refused(String),
}

/// The one way the heartbeat wakes a Persona: her brake read again, then her
/// unit started unless it is active. No run is opened and nothing is admitted.
fn wake(organs: &Organs, row: &Row) -> Woke {
    let turn = RunTurn::PersonaTurn;
    if !matches!(organs.control.effective(row.slug()), Effective::Released(_)) {
        return Woke::Refused("the brake is held".into());
    }
    if organs.launcher.alive(row.slug(), turn) {
        return Woke::Busy;
    }
    match organs.launcher.start(row.slug(), turn) {
        Ok(()) => Woke::Started,
        Err(error) => Woke::Refused(format!("start failed: {error}")),
    }
}

/// One poll.
pub fn poll(organs: &Organs, config: &Config) -> Result<Outcome> {
    if let SwarmEffective::Held(reason) = organs.control.envelope() {
        return Ok(Outcome::Held(reason));
    }
    let now = organs.clock.now();
    let stored = organs.store.load()?;
    let mut state = stored.clone().unwrap_or(HeartbeatState { last_beat_at: DateTime::<Utc>::UNIX_EPOCH, handled_mentions: vec![], last_turn_at: BTreeMap::new() });
    let mut eventful = false;
    // The table as the last poll left it; unreadable counts as none, is noted in the trace, and is not overwritten.
    let (stored_addressing, addressing_unread) = match organs.store.load_addressing() {
        Ok(found) => (found.unwrap_or_default(), None),
        Err(error) => (Addressing::default(), Some(format!("addressing unread: {error}"))),
    };

    let mut rows: Vec<Row> = Vec::new();
    for seat in organs.seats {
        let released = matches!(organs.control.effective(&seat.who.slug), Effective::Released(_));
        let busy = organs.launcher.alive(&seat.who.slug, RunTurn::PersonaTurn);
        // Her last-read names when the table holds her, else her roster names alone.
        let roster = Addressee::new(&seat.who.matrix_id, [seat.who.public_name.as_str(), seat.who.handle.as_str()]);
        let kept = stored_addressing.minds.iter().find(|stored| stored.matrix_id() == seat.who.matrix_id).cloned().unwrap_or_else(|| roster.clone());
        let mut row = Row { seat, released, busy, pending: vec![], addressee: kept, note: String::new() };
        // Pending chat, read-only, for a released Mind.
        if row.released {
            match read_pending(seat.chat) {
                Ok(deliveries) => row.pending = deliveries,
                Err(_) => {
                    eventful = true;
                    row.note = "chat: pending failed".into();
                }
            }
            // Her projects' names address her too; read only when something is pending. A failed read keeps her last-read row.
            if !row.pending.is_empty() {
                match stewardships_of(seat.mind) {
                    Ok((names, repos)) => row.addressee = roster.with_stewardships(names, repos),
                    Err(error) => row.note = format!("{} stewardship names unread: {error}", row.note).trim().to_string(),
                }
            }
        }
        rows.push(row);
    }

    let beat_s = config.beat_s;
    let mut launches: Vec<String> = Vec::new();
    let mut launched: Vec<Slug> = Vec::new();

    // Off the beat: the operator or another Mind of the roster naming her, once per message. It does not wait for the beat;
    // it needs her idle and released.
    for row in &mut rows {
        if !row.idle() {
            continue;
        }
        let slug = row.slug().clone();
        let mentioned = {
            let mut pending: Vec<&ChatDelivery> = row.pending.iter().collect();
            pending.sort_by_key(|delivery| delivery.seq);
            pending
                .into_iter()
                .filter(|delivery| wakes(WakePolicy::Live, &row.addressee, delivery, config.speaker(&row.addressee, delivery), LineFacts::default()).is_some())
                .map(|delivery| format!("{}:{}", slug.0, delivery.event_id))
                .find(|key| !state.handled_mentions.contains(key))
        };
        let Some(key) = mentioned else { continue };
        match wake(organs, row) {
            Woke::Started => {
                state.handled_mentions.push(key);
                let excess = state.handled_mentions.len().saturating_sub(MENTIONS_KEPT);
                state.handled_mentions.drain(..excess);
                state.last_turn_at.insert(slug.0.clone(), now);
                row.busy = true;
                launches.push(slug.0.clone());
                launched.push(slug);
            }
            Woke::Busy => row.busy = true,
            Woke::Refused(why) => row.note = why,
        }
    }

    // On the beat: when the beat has passed, one wake for the winner of the pick.
    let due = (now - state.last_beat_at).num_seconds() >= i64::from(beat_s);
    let standings: Vec<Standing> = rows
        .iter()
        .map(|row| {
            let last = state.last_turn_at.get(&row.slug().0).copied();
            Standing {
                slug: row.slug().0.clone(),
                eligible: row.idle(),
                waited_s: last.map(|turn| u64::try_from((now - turn).num_seconds()).unwrap_or(0)),
                last_turn_ms: last.map(|turn| turn.timestamp_millis()),
                // The seam for topic subscriptions (a later cut): a Mind's nearest subscription scores her deliveries here.
                topic_permille: 0,
                addressed_by_member: row.addressed_by_member(config),
            }
        })
        .collect();
    // A winner whose unit cannot start loses the beat to the next of the pick, so
    // one failing Mind never starves the others; a wake ends the beat.
    let mut passed: Vec<Standing> = standings.clone();
    while due && let Some(winner) = tempo::pick(&passed, beat_s) {
        match wake(organs, &rows[winner]) {
            Woke::Started => {
                let slug = rows[winner].slug().clone();
                state.last_beat_at = now;
                state.last_turn_at.insert(slug.0.clone(), now);
                rows[winner].busy = true;
                launches.push(slug.0.clone());
                launched.push(slug);
                break;
            }
            Woke::Busy => rows[winner].busy = true,
            Woke::Refused(why) => rows[winner].note = why,
        }
        passed[winner].eligible = false;
    }

    if stored.as_ref() != Some(&state) {
        organs.store.save(&state)?;
    }
    let addressing = Addressing { minds: rows.iter().map(|row| row.addressee.clone()).collect() };
    match addressing_unread {
        None if addressing != stored_addressing => organs.store.save_addressing(&addressing)?,
        None => {}
        Some(note) => {
            eventful = true;
            if let Some(first) = rows.first_mut() {
                first.note = format!("{} {note}", first.note).trim().to_string();
            }
        }
    }
    if eventful || !launches.is_empty() || rows.iter().any(|row| !row.note.is_empty()) {
        let trace = PollTrace {
            at: now,
            tempo_s: beat_s,
            instances: rows
                .iter()
                .zip(&standings)
                .map(|(row, standing)| InstanceTrace {
                    instance: row.slug().0.clone(),
                    vote_beat_s: None,
                    urgency_permille: tempo::urgency_permille(standing),
                    addressed: standing.addressed_by_member,
                    busy: row.busy,
                    released: row.released,
                    idle_s: standing.waited_s.map_or(-1, |waited| i64::try_from(waited).unwrap_or(i64::MAX)),
                    score: if standing.eligible { format!("{:.3}", tempo::score(standing, beat_s)) } else { String::new() },
                    note: row.note.clone(),
                })
                .collect(),
            launches,
        };
        organs.store.record(&trace).context("the heartbeat trace could not be written")?;
    }
    Ok(Outcome::Polled { launched })
}

#[cfg(test)]
mod tests;
