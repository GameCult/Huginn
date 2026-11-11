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
//! Self run having ended since her last turn. A beat's launch for a Mind whose
//! Self holds briefs (`inbox::derive`) is a Self run claiming them, in place of
//! her Persona turn: a session that ended with a brief open, or a brief admitted
//! after its flush, is never an orphan, and a Self start takes the beat like any
//! launch. A brief an open question holds (raised in it, or in a run that claims it:
//! the repetition breaker's trip holds the claims of the run it belongs to) is
//! neither claimed nor told until that question is answered or withdrawn; other
//! briefs are. Nothing is metered here:
//! no meter and no model call.
//!
//! The heartbeat owns `heartbeat.cc` and is its only writer; Persona units read
//! it. The brakes stay the operator's and are read on every poll. Busy and idle
//! are the slot's: a Persona run whose unit lives (or is within its start grace) is busy; a dead holder is not, and the next launch closes it. The launch is
//! `open_and_launch`, the only opener. The heartbeat never acks a delivery; the
//! Persona turn drains them.
//!
//! A held swarm brake ends the poll before the mind or the mouth is touched.

use std::fmt;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Timelike, Utc};
use cultcache_rs::{CultCache, DatabaseEntry, SingleFileMessagePackBackingStore};
use cultnet_rs::Selection;
use eureka_pipeline::{OrgRepo, PipelineKind, PipelineRef, ResolutionOutcome, RunOperator, RunTurn, Slug};
use huginn_mind::{PipelineDocumentSummary, PipelineFacts, PipelinePageItems};

use crate::address::{Addressee, Name, is_for, project_names};
use crate::chat_client::{Chat, ChatDelivery};
use crate::control::{ControlSource, Effective, FileSource, HeldReason, SwarmEffective, read_effective, read_envelope, set_mode_world_readable};
use crate::launch::{Clock, LaunchOutcome, LaunchRequest, Launcher, Ports, open_and_launch, slot};
use crate::queue::{MindPort, any_of, citing, claims_of, headers, in_force, of_kind, on_hold};
use crate::self_run::inbox;
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
    /// The nightly window and the Minds that sleep in it; none, no one sleeps.
    pub night: Option<Night>,
}

/// The heartbeat's night: a daily window in UTC minutes (it may wrap midnight) and the Minds that sleep in it.
/// The heartbeat alone decides who sleeps and when: one Sleep run per Mind per window, never made up if the window
/// passes without one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Night {
    pub from_min: u32,
    pub to_min: u32,
    pub minds: Vec<Slug>,
}

impl Night {
    /// The nightly sleep from the two settings that name it: both or neither, so a half-set deploy fails at start
    /// instead of quietly never sleeping, or sleeping no one.
    pub fn from_settings(window: Option<&str>, minds: Option<&str>) -> Result<Option<Self>> {
        match (window, minds) {
            (Some(window), Some(minds)) => Self::parse(window, minds).map(Some),
            (None, None) => Ok(None),
            _ => bail!("HEARTBEAT_SLEEP_UTC and HEARTBEAT_SLEEP_MINDS are set together or not at all"),
        }
    }

    /// `window` is `HH:MM-HH:MM` in UTC, `minds` the slugs comma-separated.
    pub fn parse(window: &str, minds: &str) -> Result<Self> {
        let minute = |text: &str| -> Result<u32> {
            let (hour, minute) = text.trim().split_once(':').ok_or_else(|| anyhow!("the night window is HH:MM-HH:MM"))?;
            let (hour, minute) = (hour.parse::<u32>()?, minute.parse::<u32>()?);
            if hour > 23 || minute > 59 {
                bail!("the night window is HH:MM-HH:MM");
            }
            Ok(hour * 60 + minute)
        };
        let (from, to) = window.split_once('-').ok_or_else(|| anyhow!("the night window is HH:MM-HH:MM"))?;
        let (from_min, to_min) = (minute(from)?, minute(to)?);
        if from_min == to_min {
            bail!("the night window is empty");
        }
        let minds: Vec<Slug> = minds.split(',').map(|slug| Slug(slug.trim().to_string())).filter(|slug| !slug.0.is_empty()).collect();
        if minds.is_empty() {
            bail!("no Mind is listed to sleep");
        }
        Ok(Self { from_min, to_min, minds })
    }

    /// When the window `now` is inside opened, or none outside it.
    fn opened(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let minute = now.hour() * 60 + now.minute();
        let inside = if self.from_min < self.to_min { (self.from_min..self.to_min).contains(&minute) } else { minute >= self.from_min || minute < self.to_min };
        if !inside {
            return None;
        }
        let day = if minute >= self.from_min { now.date_naive() } else { now.date_naive().pred_opt()? };
        Some(day.and_hms_opt(self.from_min / 60, self.from_min % 60, 0)?.and_utc())
    }
}

impl Config {
    /// Refuses no Persona, or a Persona twice.
    pub fn new(personas: Vec<PersonaId>, operators: Vec<String>, night: Option<Night>) -> Result<Self> {
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
            night,
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
    fn load_addressing(&self) -> Result<Option<Addressing>>;
    fn save_addressing(&self, addressing: &Addressing) -> Result<()>;
}

/// What a Persona unit may read of `heartbeat.cc`: the newest trace, which
/// carries the tempo and every Mind's vote and declared urgency, and the swarm's addressing. Read-only.
pub trait HeartbeatView {
    fn latest(&self) -> Result<Option<PollTrace>>;
    fn addressing(&self) -> Result<Option<Addressing>>;
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
    fn latest(&self) -> Result<Option<PollTrace>> {
        let mut all = self.snapshot()?.get_all_with_keys::<PollTrace>()?;
        all.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(all.pop().map(|(_, trace)| trace))
    }

    fn addressing(&self) -> Result<Option<Addressing>> {
        self.load_addressing()
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

/// When her newest Sleep run was admitted, whatever became of it: a night that failed still counts.
fn last_sleep(mind: &dyn MindPort, instance: &Slug) -> Result<Option<DateTime<Utc>>> {
    let operator = format!("{:?}", RunOperator::Mind);
    let runs = any_of(any_of(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]), "turn", &["Sleep"]), "operated_by", &[&operator]);
    newest(mind, runs)?.map(|header| parse_time(&header.admission.admitted_at)).transpose()
}

/// The newest document a selection holds, by admission ordinal: one query.
fn newest(mind: &dyn MindPort, selection: Selection) -> Result<Option<PipelineDocumentSummary>> {
    let asking = Selection { descending: true, limit: Some(1), ..selection };
    let PipelinePageItems::Headers(found) = mind.query(&asking)?.items else { bail!("the mind answered documents to a header selection") };
    Ok(found.into_iter().next())
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

/// Whether a delivery wakes `who` off the beat: the operator or another Mind of
/// the roster addresses her. A Mind naming herself does not; a member does not.
fn wakes(config: &Config, who: &Addressee, delivery: &ChatDelivery) -> bool {
    let from_operator = config.operators.contains(&delivery.sender);
    let from_other_mind = delivery.sender != who.matrix_id() && config.personas.iter().any(|mind| mind.matrix_id == delivery.sender);
    (from_operator || from_other_mind) && is_for(who, delivery)
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
    /// When her newest Sleep run was admitted.
    last_sleep: Option<DateTime<Utc>>,
}

fn read_mind(organs: &Organs, mind: &dyn MindPort, slug: &Slug) -> Result<MindRead> {
    let turn = last_turn(mind, slug)?;
    Ok(MindRead {
        // Busy is the slot's own reading of life: a holder whose unit is not active
        // (past its start grace) is dead, not busy, so the next launch reaches
        // `slot` step 0 and closes it there.
        busy: slot(&ports(organs, mind), slug, RunTurn::PersonaTurn, organs.clock.now())?.live.is_some(),
        last_turn: turn.as_ref().map(|header| parse_time(&header.admission.admitted_at)).transpose()?,
        vote: standing_vote(mind, slug)?,
        urgency: pending_urgency(mind, slug, turn.as_ref())?,
        self_end: last_self_end(mind, slug)?,
        last_sleep: last_sleep(mind, slug)?,
    })
}

struct Row<'a> {
    seat: &'a Seat<'a>,
    read: Option<MindRead>,
    released: bool,
    pending: Vec<ChatDelivery>,
    /// Whom a delivery may address in her: her roster names and her projects' names as the last poll that read her pending chat left them.
    addressee: Addressee,
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
        self.pending.iter().any(|delivery| !config.operators.contains(&delivery.sender) && is_for(&self.addressee, delivery))
    }
}

enum Launched {
    Run(String),
    Busy,
    Refused(String),
}

fn ports<'a>(organs: &Organs<'a>, mind: &'a dyn MindPort) -> Ports<'a> {
    Ports {
        mind,
        launcher: organs.launcher,
        clock: organs.clock,
        host: organs.host,
    }
}

/// The one way the heartbeat starts a run: the same brake read and the same
/// opener for a Persona turn (a beat, an operator mention, a Self run that ended)
/// and for the Self run a beat gives to the briefs her Self holds.
fn launch(
    organs: &Organs,
    row: &Row,
    turn: RunTurn,
    claims: Vec<PipelineRef>,
) -> Result<Launched> {
    let Effective::Released(grant) = organs.control.effective(row.slug()) else {
        return Ok(Launched::Refused("the brake is held".into()));
    };
    let request = LaunchRequest {
        turn,
        claims,
        agent: AGENT.into(),
        grant,
    };
    let ports = ports(organs, row.seat.mind);
    Ok(match open_and_launch(&ports, request) {
        Ok(LaunchOutcome::Launched { run }) => Launched::Run(label_of(&run)),
        Ok(LaunchOutcome::Busy { .. }) => Launched::Busy,
        Ok(LaunchOutcome::Tripped { run, question }) => {
            let held = claims_of(row.seat.mind, &run).map_or(0, |claims| claims.len());
            Launched::Refused(format!("tripped: {} holds the {held} claims of {}", question.id.0, run.id.0))
        }
        Ok(_) => Launched::Refused("launch refused".into()),
        Err(error) => Launched::Refused(format!("launch failed: {error}")),
    })
}

/// The most briefs one Self run claims.
const BRIEFS_PER_SELF_RUN: usize = 64;

/// What a Self run would claim for this Mind, read at one instant.
struct Claimable {
    /// The briefs in force that no live Self run holds and no open question holds.
    claims: Vec<PipelineRef>,
    /// How many briefs an open question holds.
    held: usize,
    /// Self runs in force whose unit is gone: the launch closes them.
    dead: Vec<PipelineRef>,
}

/// The briefs a Self run would claim for this Mind: those in force that no live
/// Self run holds and no open question holds. A Self run claims up to
/// `BRIEFS_PER_SELF_RUN` briefs and every run it ends empty counts against all
/// of them, so one cause trips them together; a trip holds the claims of its
/// run, not the Mind, until the operator answers or withdraws its question, and
/// the count starts afresh. Whether a holder lives is `launch::slot`'s answer
/// (a dead one is closed by the launch, so it does not stop the successor), and
/// what is held is `queue::on_hold`'s. The read here only chooses the claims;
/// the launch's gate reads the hold again at admission and decides.
fn claimable(organs: &Organs, row: &Row) -> Result<Claimable> {
    let mind = row.seat.mind;
    let found = slot(
        &ports(organs, mind),
        row.slug(),
        RunTurn::SelfRun,
        organs.clock.now(),
    )?;
    if found.live.is_some() {
        return Ok(Claimable {
            claims: Vec::new(),
            held: 0,
            dead: Vec::new(),
        });
    }
    let (free, held) = inbox::derive_counting_held(mind, &on_hold(mind)?, &HashSet::new())?;
    let claims = free
        .into_iter()
        .map(|brief| PipelineRef { kind: PipelineKind::FollowUp, id: brief.id.as_str().into() })
        .take(BRIEFS_PER_SELF_RUN)
        .collect();
    Ok(Claimable {
        claims,
        held,
        dead: found.dead,
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
    // The table as the last poll left it; unreadable counts as none, is noted in the trace, and is not overwritten.
    let (stored_addressing, addressing_unread) = match organs.store.load_addressing() {
        Ok(found) => (found.unwrap_or_default(), None),
        Err(error) => (Addressing::default(), Some(format!("addressing unread: {error}"))),
    };

    let mut rows: Vec<Row> = Vec::new();
    for seat in organs.seats {
        let released = matches!(organs.control.effective(&seat.who.slug), Effective::Released(_));
        // One Mind daemon down skips her for this poll; the others are served.
        let (read, note) = match read_mind(organs, seat.mind, &seat.who.slug) {
            Ok(read) => (Some(read), String::new()),
            Err(error) => {
                eventful = true;
                (None, format!("skipped: her mind could not be read: {error}"))
            }
        };
        // Her last-read names when the table holds her, else her roster names alone.
        let roster = Addressee::new(&seat.who.matrix_id, [seat.who.public_name.as_str(), seat.who.handle.as_str()]);
        let kept = stored_addressing.minds.iter().find(|stored| stored.matrix_id() == seat.who.matrix_id).cloned().unwrap_or_else(|| roster.clone());
        let mut row = Row { seat, busy: read.as_ref().is_some_and(|read| read.busy), read, released, pending: vec![], addressee: kept, note };
        // Pending chat, read-only, for a released Mind whose mind could be read.
        if row.released && row.read.is_some() {
            match read_pending(seat.chat) {
                Ok(deliveries) => row.pending = deliveries,
                Err(_) => row.note = "chat: pending failed".into(),
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

    // The tempo is the tally over the votes cast by the Minds of the roster, braked or not; a Mind with no vote or no readable mind abstains and is not counted.
    let votes: Vec<Option<u32>> = rows.iter().map(|row| row.read.as_ref().and_then(|read| read.vote)).collect();
    let beat_s = tempo::tally(&votes);

    let mut launches: Vec<String> = Vec::new();
    let mut launched: Vec<Slug> = Vec::new();

    // Off the beat: the operator or another Mind of the roster naming her, once per message, and her Self run
    // having ended since her last turn. Neither waits for the beat; both need her
    // idle and released.
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
                .filter(|delivery| wakes(config, &row.addressee, delivery))
                .map(|delivery| format!("{}:{}", slug.0, delivery.event_id))
                .find(|key| !state.handled_mentions.contains(key))
        };
        let self_ended = row.read.as_ref().is_some_and(|read| match (read.self_end, read.last_turn) {
            (Some(ended), Some(turn)) => ended > turn,
            (Some(_), None) => true,
            _ => false,
        });
        if mentioned.is_none() && !self_ended {
            continue;
        }
        match launch(organs, row, RunTurn::PersonaTurn, vec![])? {
            Launched::Run(label) => {
                if let Some(key) = mentioned {
                    state.handled_mentions.push(key);
                    let excess = state.handled_mentions.len().saturating_sub(MENTIONS_KEPT);
                    state.handled_mentions.drain(..excess);
                }
                row.busy = true;
                launches.push(label);
                launched.push(slug);
            }
            Launched::Busy => row.busy = true,
            Launched::Refused(why) => row.note = why,
        }
    }

    // The night: inside the window, one Sleep run for a listed Mind who is idle and released and has not slept
    // since the window opened (a failed night counts). The operator's mention went first above: she is busy then.
    // A window that passes without a night is not made up.
    if let Some(night) = &config.night
        && let Some(opened) = night.opened(now)
    {
        for row in &mut rows {
            if !row.idle() || !night.minds.contains(row.slug()) || row.read.as_ref().is_some_and(|read| read.last_sleep.is_some_and(|slept| slept >= opened)) {
                continue;
            }
            match launch(organs, row, RunTurn::Sleep, vec![])? {
                Launched::Run(label) => {
                    row.note = format!("asleep {label}");
                    row.busy = true;
                    launched.push(row.slug().clone());
                    launches.push(label);
                    break;
                }
                Launched::Busy => row.busy = true,
                Launched::Refused(why) => row.note = why,
            }
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
        // Her Self holds briefs: the beat is a Self run for them; otherwise a Persona turn.
        let Claimable {
            claims,
            held,
            dead,
        } = claimable(organs, &rows[winner]).unwrap_or_else(|error| {
            rows[winner].note = format!("briefs: {error}");
            Claimable {
                claims: Vec::new(),
                held: 0,
                dead: Vec::new(),
            }
        });
        let mut notes: Vec<String> = Vec::new();
        if held > 0 {
            notes.push(format!("{held} briefs held by open questions"));
        }
        if !claims.is_empty() && !dead.is_empty() {
            let ids: Vec<&str> = dead.iter().map(|run| run.id.0.as_str()).collect();
            notes.push(format!(
                "closing the Self run whose unit is gone: {}",
                ids.join(", ")
            ));
        }
        if !notes.is_empty() {
            rows[winner].note = notes.join("; ");
        }
        let turn = if claims.is_empty() {
            RunTurn::PersonaTurn
        } else {
            RunTurn::SelfRun
        };
        match launch(organs, &rows[winner], turn, claims)? {
            Launched::Run(label) => {
                state.last_beat_at = now;
                rows[winner].busy = turn == RunTurn::PersonaTurn;
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
