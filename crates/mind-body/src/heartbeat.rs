//! The heartbeat: the one process that decides which Persona takes a turn and
//! when (rulings `eureka-body:ruling:shared-heartbeat-joint-lever`,
//! `self-declared-urgency-etiquette`, `operator-mention-fills-initiative`).
//!
//! The Minds share one heartbeat and jointly hold the lever to it. Its tempo is
//! `tempo::tally` of every Mind's newest vote, a `Pulse` in her own mind. When a
//! beat falls (the beat has passed since the last one) exactly one Persona takes
//! a turn: `tempo::pick` over the idle, released Minds, by how long each has
//! waited, bent by the urgency she declared for herself, a member addressing her
//! and (a later cut) topics she subscribed to. Two things launch a Persona off
//! the beat: the operator tagging or naming her, once per message, and her own
//! Self run having ended since her last turn. Nothing is metered here: no
//! meter and no model call.
//!
//! The heartbeat owns `heartbeat.cc` and is its only writer; Persona units read
//! it. The brakes stay the operator's and are read on every poll. Busy and idle
//! are the mind's: an in-force Persona run is busy. The launch is
//! `open_and_launch`, the only opener. The heartbeat never acks a delivery; the
//! Persona turn drains them.
//!
//! A held swarm brake ends the poll before the mind or the mouth is touched.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use cultcache_rs::{CultCache, DatabaseEntry, SingleFileMessagePackBackingStore};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineKind, PipelineRef, ResolutionOutcome, RunOperator, RunTurn, Slug};
use huginn_mind::{PipelineDocumentSummary, PipelineFacts, PipelinePageItems};

use crate::chat_client::{Chat, ChatDelivery};
use crate::control::{ControlSource, Effective, FileSource, HeldReason, SwarmEffective, read_effective, read_envelope, set_mode_world_readable};
use crate::launch::{Clock, LaunchOutcome, LaunchRequest, Launcher, Ports, holders, open_and_launch};
use crate::queue::{MindPort, any_of, citing, headers, of_kind};
use crate::tempo::{self, Standing};

/// The admitting agent's name on every run the heartbeat opens.
pub const AGENT: &str = "mind-heartbeat";
/// How many handled operator mentions the state remembers.
const MENTIONS_KEPT: usize = 64;
/// How many of an instance's latest Self runs a poll looks at.
const RECENT: u32 = 8;
/// The most pages of pending chat one poll reads for one Persona.
const PENDING_PAGES: usize = 16;

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
}

impl Config {
    /// Refuses no Persona, or a Persona twice.
    pub fn new(personas: Vec<PersonaId>, operators: Vec<String>) -> Result<Self> {
        if personas.is_empty() {
            bail!("no persona is configured");
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
        })
    }
}

/// `heartbeat.cc`'s state: when the last beat fell, and the operator messages
/// already answered with a launch (once per message, bounded). A later cut adds
/// the cache of scored deliveries beside them.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.heartbeat.state.v1", schema = "HeartbeatState")]
pub struct HeartbeatState {
    #[cultcache(key = 0)]
    pub last_beat_at: DateTime<Utc>,
    /// `<instance>:<event id>` of each operator message that launched a Persona.
    #[cultcache(key = 1)]
    pub handled_mentions: Vec<String>,
}

const STATE_KEY: &str = "state";

/// One instance's line in a poll's trace.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstanceTrace {
    pub instance: String,
    /// Her newest tempo vote, in seconds per beat.
    pub vote_beat_s: Option<u32>,
    /// The urgency in force for her: declared and not yet consumed by a turn.
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
    /// The beat in force: the tally of the Minds' votes.
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
                "  {}: vote {:?} urgency {} addressed {} busy {} released {} idle {}s score {:?}",
                row.instance, row.vote_beat_s, row.urgency_permille, row.addressed, row.busy, row.released, row.idle_s, row.score
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
}

/// What a Persona unit may read of `heartbeat.cc`: the newest trace, which
/// carries the tempo and every Mind's vote and declared urgency. Read-only.
pub trait TempoView {
    fn latest(&self) -> Result<Option<PollTrace>>;
}

/// The store as a CultCache single-file store under the unit's state directory.
/// It registers the state and the trace and nothing else: a file still holding
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
        Ok(cache)
    }

    fn open(&self) -> Result<CultCache> {
        let mut cache = Self::registered()?;
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&self.path))?;
        cache.pull_all_backing_stores().with_context(|| self.refusal())?;
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
}

impl FileStore {
    /// The file read without a lock and without a write, as the control stores are.
    fn snapshot(&self) -> Result<CultCache> {
        let mut cache = Self::registered()?;
        for envelope in FileSource::new(&self.path).snapshot().with_context(|| self.refusal())? {
            cache.put_raw_envelope(envelope).with_context(|| self.refusal())?;
        }
        Ok(cache)
    }
}

impl TempoView for FileStore {
    fn latest(&self) -> Result<Option<PollTrace>> {
        let mut all = self.snapshot()?.get_all_with_keys::<PollTrace>()?;
        all.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(all.pop().map(|(_, trace)| trace))
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
    pub host: &'a str,
    pub store: &'a dyn Store,
}

/// What one poll did. Every outcome is an answer; only a mind, store or
/// mouth that cannot be reached is an error.
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

fn parse_time(text: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .map(|at| at.with_timezone(&Utc))
        .map_err(|_| anyhow!("the mind gave a time that is not a timestamp"))
}

/// When a run was last resolved and how: its resolution of the highest sequence.
fn latest_resolution(
    mind: &dyn MindPort,
    run: &PipelineRef,
) -> Result<Option<(DateTime<Utc>, ResolutionOutcome)>> {
    let closures = Selection {
        cites: Some(citing(run, Some("subject"))),
        ..of_kind(PipelineKind::Resolution)
    };
    let mut ending: Option<(u32, DateTime<Utc>, ResolutionOutcome)> = None;
    for header in headers(mind, &closures)? {
        if let PipelineFacts::Resolution {
            sequence, outcome, ..
        } = header.facts
            && ending
                .as_ref()
                .is_none_or(|(latest, ..)| sequence >= *latest)
        {
            ending = Some((
                sequence,
                parse_time(&header.admission.admitted_at)?,
                outcome,
            ));
        }
    }
    Ok(ending.map(|(_, at, outcome)| (at, outcome)))
}

/// When her newest Persona turn that actually ran was admitted: one bounded read
/// of the runs the mind indexes as `ran`. A turn withdrawn unrun never happened,
/// so it neither resets her wait nor counts as her last turn, and the cost of
/// finding the last real turn does not grow with how many were withdrawn.
fn last_turn(mind: &dyn MindPort, instance: &Slug) -> Result<Option<PipelineDocumentSummary>> {
    let turn = format!("{:?}", RunTurn::PersonaTurn);
    let operator = format!("{:?}", RunOperator::Mind);
    let runs = any_of(
        any_of(
            any_of(
                any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]),
                "turn",
                &[&turn],
            ),
            "operated_by",
            &[&operator],
        ),
        "ran",
        &["true"],
    );
    newest(mind, runs)
}

/// The newest document a selection holds, by admission ordinal: one query.
fn newest(mind: &dyn MindPort, selection: Selection) -> Result<Option<PipelineDocumentSummary>> {
    let asking = Selection { descending: true, limit: Some(1), ..selection };
    let PipelinePageItems::Headers(found) = mind.query(&asking)?.items else { bail!("the mind answered documents to a header selection") };
    Ok(found.into_iter().next())
}

/// Whether `text` holds `name` as a whole word, ignoring case: the characters on
/// either side are not letters or digits. An identity match on a name, not a
/// judgement of meaning.
fn names_whole_word(text: &str, name: &str) -> bool {
    let (text, name) = (text.to_lowercase(), name.to_lowercase());
    if name.is_empty() {
        return false;
    }
    text.match_indices(&name).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + name.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// How an operator delivery reaches a Persona: tagged in `m.mentions`, or named.
fn addresses(who: &PersonaId, delivery: &ChatDelivery) -> Option<&'static str> {
    if delivery.mentions.iter().any(|id| id == &who.matrix_id) {
        Some("tag")
    } else if names_whole_word(&delivery.text, &who.public_name)
        || names_whole_word(&delivery.text, &who.handle)
    {
        Some("name")
    } else {
        None
    }
}

/// The mouth pages a pending answer, so fewer deliveries may come than wait.
/// The heartbeat acks nothing: it asks again after the highest seq it has until
/// the answer is empty. A mouth that never empties is cut off at `PENDING_PAGES`.
fn read_pending(chat: &dyn Chat) -> Result<Vec<ChatDelivery>> {
    let mut all: Vec<ChatDelivery> = Vec::new();
    let mut after = 0;
    for _ in 0..PENDING_PAGES {
        let page = chat.pending(after)?;
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

fn label_of(run: &eureka_pipeline::PipelineRef) -> String {
    run.id.0.rsplit(':').next().unwrap_or_default().to_string()
}

/// Her standing vote: the newest pulse that votes or withdraws, by admission
/// ordinal. A vote stands until she casts another or withdraws it, however many
/// pulses of other kinds follow; a withdrawal leaves her silent.
fn standing_vote(mind: &dyn MindPort, instance: &Slug) -> Result<Option<u32>> {
    let pulses = any_of(any_of(of_kind(PipelineKind::Pulse), "root", &[&instance.0]), "voted", &["true"]);
    Ok(newest(mind, pulses)?.and_then(|header| match header.facts {
        PipelineFacts::Pulse { tempo_vote_beat_s, tempo_vote_withdrawn: false, .. } => tempo_vote_beat_s,
        _ => None,
    }))
}

/// The urgency she declared for herself and no turn has consumed: her newest
/// declaration, if it was admitted after her newest Persona turn, by admission
/// ordinal like the vote: the wall clock never decides what a turn consumed.
fn pending_urgency(mind: &dyn MindPort, instance: &Slug, last_turn: Option<&PipelineDocumentSummary>) -> Result<u16> {
    let pulses = any_of(any_of(of_kind(PipelineKind::Pulse), "root", &[&instance.0]), "urged", &["true"]);
    let Some(header) = newest(mind, pulses)? else { return Ok(0) };
    let PipelineFacts::Pulse { urgency_permille: Some(urgency), .. } = header.facts else { return Ok(0) };
    Ok(if last_turn.is_none_or(|turn| header.admission.ordinal > turn.admission.ordinal) { urgency } else { 0 })
}

/// When her Self runs last ended: the latest resolution among her recent Self runs.
fn last_self_end(mind: &dyn MindPort, instance: &Slug) -> Result<Option<DateTime<Utc>>> {
    let turn = format!("{:?}", RunTurn::SelfRun);
    let operator = format!("{:?}", RunOperator::Mind);
    let runs = any_of(any_of(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]), "turn", &[&turn]), "operated_by", &[&operator]);
    let selection = Selection { descending: true, limit: Some(RECENT), ..runs };
    let PipelinePageItems::Headers(runs) = mind.query(&selection)?.items else { bail!("the mind answered documents to a header selection") };
    let mut newest = None;
    for run in runs {
        if let Some((ended, _)) = latest_resolution(mind, &run.id)? {
            newest = newest.max(Some(ended));
        }
    }
    Ok(newest)
}

/// One Persona reads from her own Mind.
struct MindRead {
    busy: bool,
    /// When her newest Persona turn was admitted.
    last_turn: Option<DateTime<Utc>>,
    /// Her standing vote for a beat.
    vote: Option<u32>,
    /// The urgency she declared and no turn has consumed.
    urgency: u16,
    self_end: Option<DateTime<Utc>>,
}

fn read_mind(mind: &dyn MindPort, slug: &Slug) -> Result<MindRead> {
    let turn = last_turn(mind, slug)?;
    Ok(MindRead {
        busy: !holders(mind, slug, RunTurn::PersonaTurn)?.is_empty(),
        last_turn: turn.as_ref().map(|header| parse_time(&header.admission.admitted_at)).transpose()?,
        vote: standing_vote(mind, slug)?,
        urgency: pending_urgency(mind, slug, turn.as_ref())?,
        self_end: last_self_end(mind, slug)?,
    })
}

struct Row<'a> {
    seat: &'a Seat<'a>,
    read: Option<MindRead>,
    released: bool,
    pending: Vec<ChatDelivery>,
    busy: bool,
    note: String,
}

impl Row<'_> {
    fn slug(&self) -> &Slug {
        &self.seat.who.slug
    }

    fn idle(&self) -> bool {
        self.released && !self.busy && self.read.is_some()
    }

    /// A member (not the operator) has a pending message that names or tags her.
    fn addressed_by_member(&self, config: &Config) -> bool {
        self.pending.iter().any(|delivery| !config.operators.contains(&delivery.sender) && addresses(&self.seat.who, delivery).is_some())
    }
}

enum Launched {
    Run(String),
    Busy,
    Refused(String),
}

/// The one way the heartbeat starts a Persona turn: the same brake read and the
/// same opener for a beat, an operator mention and a Self run that ended.
fn launch(organs: &Organs, row: &Row) -> Result<Launched> {
    let Effective::Released(grant) = organs.control.effective(row.slug()) else { return Ok(Launched::Refused("the brake is held".into())) };
    let ports = Ports { mind: row.seat.mind, launcher: organs.launcher, clock: organs.clock, host: organs.host };
    let request = LaunchRequest { turn: RunTurn::PersonaTurn, claims: vec![], agent: AGENT.into(), grant };
    Ok(match open_and_launch(&ports, request) {
        Ok(LaunchOutcome::Launched { run }) => Launched::Run(label_of(&run)),
        Ok(LaunchOutcome::Busy { .. }) => Launched::Busy,
        Ok(_) => Launched::Refused("launch refused".into()),
        Err(error) => Launched::Refused(format!("launch failed: {error}")),
    })
}

/// One poll.
pub fn poll(organs: &Organs, config: &Config) -> Result<Outcome> {
    if let SwarmEffective::Held(reason) = organs.control.envelope() {
        return Ok(Outcome::Held(reason));
    }
    let now = organs.clock.now();
    let stored = organs.store.load()?;
    let mut state = stored.clone().unwrap_or(HeartbeatState { last_beat_at: DateTime::<Utc>::UNIX_EPOCH, handled_mentions: vec![] });
    let mut eventful = false;

    let mut rows: Vec<Row> = Vec::new();
    for seat in organs.seats {
        let released = matches!(organs.control.effective(&seat.who.slug), Effective::Released(_));
        // One Mind daemon down skips her for this poll; the others are served.
        let (read, note) = match read_mind(seat.mind, &seat.who.slug) {
            Ok(read) => (Some(read), String::new()),
            Err(error) => {
                eventful = true;
                (None, format!("skipped: her mind could not be read: {error}"))
            }
        };
        let mut row = Row { seat, busy: read.as_ref().is_some_and(|read| read.busy), read, released, pending: vec![], note };
        // Pending chat, read-only, for a released Mind whose mind could be read.
        if row.released && row.read.is_some() {
            match read_pending(seat.chat) {
                Ok(deliveries) => row.pending = deliveries,
                Err(_) => row.note = "chat: pending failed".into(),
            }
        }
        rows.push(row);
    }

    // The tempo is the tally over every Mind of the roster, braked or not; a Mind with no vote or no readable mind counts as the default beat.
    let votes: Vec<Option<u32>> = rows.iter().map(|row| row.read.as_ref().and_then(|read| read.vote)).collect();
    let beat_s = tempo::tally(&votes);

    let mut launches: Vec<String> = Vec::new();
    let mut launched: Vec<Slug> = Vec::new();

    // Off the beat: the operator naming her, once per message, and her Self run
    // having ended since her last turn. Neither waits for the beat; both need her
    // idle and released.
    for index in 0..rows.len() {
        if !rows[index].idle() {
            continue;
        }
        let slug = rows[index].slug().clone();
        let mentioned = {
            let row = &rows[index];
            let mut pending: Vec<&ChatDelivery> = row.pending.iter().collect();
            pending.sort_by_key(|delivery| delivery.seq);
            pending
                .into_iter()
                .filter(|delivery| config.operators.contains(&delivery.sender) && addresses(&row.seat.who, delivery).is_some())
                .map(|delivery| format!("{}:{}", slug.0, delivery.event_id))
                .find(|key| !state.handled_mentions.contains(key))
        };
        let self_ended = rows[index].read.as_ref().is_some_and(|read| match (read.self_end, read.last_turn) {
            (Some(ended), Some(turn)) => ended > turn,
            (Some(_), None) => true,
            _ => false,
        });
        if mentioned.is_none() && !self_ended {
            continue;
        }
        match launch(organs, &rows[index])? {
            Launched::Run(label) => {
                if let Some(key) = mentioned {
                    state.handled_mentions.push(key);
                    let excess = state.handled_mentions.len().saturating_sub(MENTIONS_KEPT);
                    state.handled_mentions.drain(..excess);
                }
                rows[index].busy = true;
                launches.push(label);
                launched.push(slug);
            }
            Launched::Busy => rows[index].busy = true,
            Launched::Refused(why) => rows[index].note = why,
        }
    }

    // On the beat: when the beat has passed, one launch for the winner of the pick.
    let due = (now - state.last_beat_at).num_seconds() >= i64::from(beat_s);
    let standings: Vec<Standing> = rows
        .iter()
        .map(|row| {
            let read = row.read.as_ref();
            Standing {
                slug: row.slug().0.clone(),
                eligible: row.idle(),
                waited_s: read.and_then(|read| read.last_turn).map(|turn| u64::try_from((now - turn).num_seconds()).unwrap_or(0)),
                last_turn_ms: read.and_then(|read| read.last_turn).map(|turn| turn.timestamp_millis()),
                self_permille: read.map_or(0, |read| read.urgency),
                // The seam for topic subscriptions (a later cut): a Mind's nearest subscription scores her deliveries here.
                topic_permille: 0,
                addressed_by_member: row.addressed_by_member(config),
            }
        })
        .collect();
    // A winner whose unit cannot start loses the beat to the next of the pick, so
    // one failing Mind never starves the others; a launch ends the beat.
    let mut passed: Vec<Standing> = standings.clone();
    while due && let Some(winner) = tempo::pick(&passed, beat_s) {
        match launch(organs, &rows[winner])? {
            Launched::Run(label) => {
                state.last_beat_at = now;
                rows[winner].busy = true;
                launches.push(label);
                launched.push(rows[winner].slug().clone());
                break;
            }
            Launched::Busy => rows[winner].busy = true,
            Launched::Refused(why) => rows[winner].note = why,
        }
        passed[winner].eligible = false;
    }

    if stored.as_ref() != Some(&state) {
        organs.store.save(&state)?;
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
                    vote_beat_s: row.read.as_ref().and_then(|read| read.vote),
                    urgency_permille: standing.self_permille,
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
