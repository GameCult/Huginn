//! The stream organ: her stream of thought, offline. Once per tick it projects her own
//! state through Ghostlight's Projector, lets a base model continue the projection a few ways,
//! keeps the continuation least like anything she has thought before, and admits it as one
//! `thought` with its whole trace. `mind-stream@<instance>` (agent `mind-stream`) alone decides
//! whether a thought exists and what it says.
//!
//! Grounding: no selection the seed makes names kind `thought`. A thought's words reach a later
//! seed only through something she wrote (a memory, a reply, a run). Cadence is derived from her
//! newest thought's admission time and nothing else is kept: no state file, no embedder (the
//! daemon scores nearness), no second prompt builder.
//!
//! The stream never admits anything but thoughts, never puts her persona state, never sends chat
//! and never starts a run. Delivery to a Persona is another organ's.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration as Span, Utc};
use eureka_pipeline::{
    Label, Line, Para, PipelineDocument, PipelineKind, PipelineRef, PipelineThought, ResolutionOutcome, Short, Slug, THOUGHT_CANDIDATES_MAX,
    THOUGHT_NEAREST_NONE_PERMILLE,
};
use eureka_state::HuginnClient;
use ghostlight_persona_projection::{ProjectorPrompt, build_projector_prompt};
use huginn_mind::{HuginnMindResponse, PipelineAdmissionOutcome, PipelineDocumentView, PipelinePageItems};
use serde_json::{Value, json};

use crate::control::Effective;
use crate::persona::{Responsibilities, outcome_words, quoted, recent_runs, render_responsibilities, repo_notice_line};
use crate::queue::{MindPort, any_of, headers, in_force, of_kind};
use crate::recall::{self, Cue};

pub const AGENT: &str = "mind-stream";

/// No tick sooner than this after her newest thought, whatever has happened.
pub const STREAM_MIN_GAP_S: i64 = 900;
/// How often the stream process polls: the unit in which a poll "crosses" a `STREAM_MIN_GAP_S` mark.
pub const STREAM_POLL_S: i64 = 60;
/// A tick is due after this long even when nothing has happened.
pub const STREAM_IDLE_GAP_S: i64 = 3600;
/// The most bytes of context and of stimulus the Projector is shown.
const SECTION_MAX_BYTES: usize = 3800;
/// An in-force memory this old may come unbidden when nothing is new.
const UNBIDDEN_AFTER_DAYS: i64 = 7;

const PROJECTOR_WORDS: usize = 180;
const CONTINUATIONS: usize = THOUGHT_CANDIDATES_MAX;
const TEMPERATURE_PERMILLE: u16 = 1000;
const MIN_P_PERMILLE: u16 = 50;
const MAX_TOKENS: u16 = 160;
const NUM_THREAD: u32 = 4;

/// The Ghostlight revision `build_projector_prompt` comes from; a test holds it to the manifest's pin.
const GHOSTLIGHT_REV: &str = "c43eb54962ed4a09cc737ecac73a16b0d7ce62b4";
/// How the continuer frame writes the date.
const FRAME_DATE_FORMAT: &str = "%-d %B %Y";

/// The setting the Projector renders for: where she is between conversations.
const STREAM_GUIDANCE: &str = "This person is a Mind of the GameCult swarm in the quiet between conversations. What she holds is her repos, her fellow Minds and the people she works with. Render what is on her mind in this quiet.";

/// What each part of the context may take. Their sum and the separators stay under
/// `SECTION_MAX_BYTES`, so no section can crowd another out.
const BUDGETS: [(&str, usize); 7] = [
    ("values", 350),
    ("pressures", 450),
    ("bonds", 500),
    ("runs", 250),
    ("responsibilities", 800),
    ("recall", 800),
    ("replies", 600),
];

const REPLIES_SHOWN: u32 = 3;
const RECENT_READ: u32 = 30;
const READS_MAX: usize = 64;
const LINE_MAX_BYTES: usize = 1000;
const PARA_MAX_BYTES: usize = 4000;

// ---------------------------------------------------------------------------------------------
// The model port.

/// How a continuation is sampled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sampling {
    pub temperature_permille: u16,
    pub min_p_permille: u16,
    pub max_tokens: u16,
}

impl Sampling {
    const STREAM: Self = Self { temperature_permille: TEMPERATURE_PERMILLE, min_p_permille: MIN_P_PERMILLE, max_tokens: MAX_TOKENS };
}

/// The two models of a tick. Faults are errors: the tick ends, the process does not.
pub trait StreamModel {
    /// The Projector: one chat prompt in, one projection out.
    fn project(&self, prompt: &str) -> Result<String>;
    /// The continuer: `n` raw continuations of `prompt`, no chat template.
    fn continue_raw(&self, prompt: &str, sampling: &Sampling, n: usize) -> Result<Vec<String>>;
}

/// Ollama on this host. Loopback only, as Ghostlight's local binding is.
pub struct OllamaStream {
    base: String,
    projector_model: String,
    base_model: String,
    agent: ureq::Agent,
}

impl OllamaStream {
    /// `endpoint` is `http://<loopback host>:<port>`; any other address is refused here.
    pub fn open(endpoint: &str, projector_model: &str, base_model: &str) -> Result<Self> {
        // Parsed as the HTTP client parses it, so the host checked is the host connected to.
        let uri: ureq::http::Uri = endpoint.parse().map_err(|_| anyhow!("the stream endpoint is not a URL"))?;
        let authority = uri.authority().filter(|_| uri.scheme_str() == Some("http")).ok_or_else(|| anyhow!("the stream endpoint must be http://<loopback>:<port>"))?;
        if authority.as_str().contains('@') {
            bail!("the stream endpoint carries userinfo");
        }
        if uri.path() != "/" || uri.query().is_some() {
            bail!("the stream endpoint is an origin, not a path");
        }
        let host = authority.host().trim_start_matches('[').trim_end_matches(']');
        let loopback = host.eq_ignore_ascii_case("localhost") || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if !loopback {
            bail!("the stream endpoint is not a loopback address");
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(900)))
            .timeout_connect(Some(Duration::from_secs(3)))
            .http_status_as_error(false)
            // The host checked above is the host connected to: no proxy from the environment, no redirect to elsewhere.
            .proxy(None)
            .max_redirects(0)
            .build()
            .into();
        Ok(Self { base: format!("http://{authority}"), projector_model: projector_model.to_owned(), base_model: base_model.to_owned(), agent })
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let mut response = self.agent.post(&url).header("content-type", "application/json").send(body.to_string()).with_context(|| format!("no answer from {url}"))?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().with_context(|| format!("reading the answer from {url}"))?;
        if status != 200 {
            bail!("the model host answered {status} to {path}: {}", text.chars().take(300).collect::<String>());
        }
        serde_json::from_str(&text).with_context(|| format!("the answer to {path} is not JSON"))
    }
}

impl StreamModel for OllamaStream {
    fn project(&self, prompt: &str) -> Result<String> {
        let answer = self.post("/api/chat", &json!({ "model": self.projector_model, "stream": false, "messages": [{ "role": "user", "content": prompt }] }))?;
        answer["message"]["content"].as_str().map(str::to_owned).ok_or_else(|| anyhow!("the Projector's answer has no message content"))
    }

    fn continue_raw(&self, prompt: &str, sampling: &Sampling, n: usize) -> Result<Vec<String>> {
        let body = json!({
            "model": self.base_model,
            "prompt": prompt,
            "raw": true,
            "stream": false,
            "options": {
                "temperature": f64::from(sampling.temperature_permille) / 1000.0,
                "min_p": f64::from(sampling.min_p_permille) / 1000.0,
                "num_predict": sampling.max_tokens,
                "stop": ["\n\n"],
                "num_thread": NUM_THREAD,
            },
        });
        (0..n)
            .map(|_| self.post("/api/generate", &body)?["response"].as_str().map(str::to_owned).ok_or_else(|| anyhow!("a continuation's answer has no response")))
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Her persona state, read and never put.

pub struct PersonaRead {
    pub state: Value,
    pub receipt_id: String,
}

pub trait PersonaSource {
    fn persona(&self) -> Result<PersonaRead>;
}

/// Her persona state from the daemon, as the Persona organ reads it.
pub struct HuginnPersonaSource {
    client: HuginnClient,
}

impl HuginnPersonaSource {
    pub fn new(client: HuginnClient) -> Self {
        Self { client }
    }
}

impl PersonaSource for HuginnPersonaSource {
    fn persona(&self) -> Result<PersonaRead> {
        match self.client.persona_get().map_err(|_| anyhow!("the mind could not be asked for the persona"))? {
            HuginnMindResponse::Persona(Some(view)) => Ok(PersonaRead { state: view.value, receipt_id: view.receipt_id }),
            HuginnMindResponse::Persona(None) => bail!("the mind holds no persona state"),
            _ => bail!("the mind answered the persona request with something else"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The seed.

/// What the Projector is shown, and the documents it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seed {
    pub identity: String,
    pub context: String,
    pub stimulus: String,
    /// Whether anything new was admitted since her newest thought (the unbidden memory and the
    /// nothing-new line are not news).
    pub news: bool,
    pub reads: Vec<PipelineRef>,
}

impl Seed {
    fn projector_prompt(&self) -> String {
        build_projector_prompt(&ProjectorPrompt {
            identity: &self.identity,
            typed_context: &self.context,
            visible_stimulus: &self.stimulus,
            domain_guidance: STREAM_GUIDANCE,
            word_budget: PROJECTOR_WORDS,
        })
    }
}

fn cap(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let cut = &text[..end];
    match cut.rfind('\n') {
        Some(at) if at > 0 => cut[..at].to_owned(),
        _ => cut.to_owned(),
    }
}

fn text_of<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter().try_fold(value, |at, key| at.get(*key)).and_then(Value::as_str).unwrap_or_default().trim()
}

fn items<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    path.iter().try_fold(value, |at, key| at.get(*key)).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn identity_of(state: &Value) -> String {
    let mut parts = vec![text_of(state, &["publicName"]).to_owned()];
    for text in [text_of(state, &["publicDescription"]), text_of(state, &["presentation", "pronouns"]), text_of(state, &["presentation", "voiceSummary"])] {
        if !text.is_empty() {
            parts.push(text.to_owned());
        }
    }
    cap(&parts.join(". "), LINE_MAX_BYTES)
}

fn values_section(state: &Value) -> String {
    let mut values: Vec<&Value> = items(state, &["values"]).iter().collect();
    values.sort_by(|a, b| b["priority"].as_f64().unwrap_or(0.0).total_cmp(&a["priority"].as_f64().unwrap_or(0.0)));
    let lines: Vec<String> = values.iter().map(|value| format!("- {}: {}", text_of(value, &["label"]), text_of(value, &["summary"]))).collect();
    section("What she values:", lines)
}

fn pressures_section(state: &Value) -> String {
    let lines: Vec<String> = items(state, &["agencyPressure", "pressures"]).iter().map(|pressure| format!("- {}", text_of(pressure, &["summary"]))).collect();
    section("What presses on her:", lines)
}

fn bond_line(bond: &Value) -> String {
    let who = [text_of(bond, &["object", "label"]), text_of(bond, &["object", "id"])].into_iter().find(|name| !name.is_empty()).unwrap_or("someone");
    format!("- {who} ({}): {}", text_of(bond, &["relationshipKind"]), text_of(bond, &["summary"]))
}

fn bonds_section(state: &Value) -> String {
    section("Who she is bound to:", items(state, &["affect", "socialBonds"]).iter().map(bond_line).collect())
}

fn section(heading: &str, lines: Vec<String>) -> String {
    if lines.is_empty() { String::new() } else { format!("{heading}\n{}", lines.join("\n")) }
}

/// The people her bonds name, by the ids her memories say they are `about`.
fn bonded_people(state: &Value) -> Vec<String> {
    items(state, &["affect", "socialBonds"])
        .iter()
        .filter(|bond| text_of(bond, &["object", "kind"]) == "person")
        .map(|bond| text_of(bond, &["object", "id"]).to_owned())
        .filter(|id| !id.is_empty())
        .collect()
}

fn documents(mind: &dyn MindPort, mut selection: cultnet_rs::Selection, limit: u32) -> Result<Vec<PipelineDocumentView>> {
    selection.projection = "document".into();
    selection.descending = true;
    selection.limit = Some(limit);
    match mind.query(&selection)?.items {
        PipelinePageItems::Documents(views) => Ok(views),
        PipelinePageItems::Headers(_) => bail!("the mind answered headers to a document selection"),
    }
}

fn admitted_at(view: &PipelineDocumentView) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&view.admission.admitted_at).ok().map(|at| at.with_timezone(&Utc))
}

fn mine(kind: PipelineKind, instance: &Slug) -> cultnet_rs::Selection {
    any_of(of_kind(kind), "root", &[&instance.0])
}

/// Her Self's newest replies, whether or not her Persona has heard them.
fn replies(mind: &dyn MindPort, instance: &Slug) -> Result<Vec<(PipelineRef, String)>> {
    let mut found: Vec<_> = documents(mind, mine(PipelineKind::Reply, instance), REPLIES_SHOWN)?
        .into_iter()
        .filter_map(|view| match view.document {
            PipelineDocument::Reply(reply) => Some((view.admission.ordinal, view.id, reply.text.0)),
            _ => None,
        })
        .collect();
    found.sort_by_key(|(ordinal, ..)| std::cmp::Reverse(*ordinal));
    Ok(found.into_iter().take(REPLIES_SHOWN as usize).map(|(_, id, text)| (id, text)).collect())
}

/// What her mind admitted since `since`, newest first: one line and one citation each.
fn mind_news(mind: &dyn MindPort, instance: &Slug, since: Option<DateTime<Utc>>) -> Result<Vec<(String, Option<PipelineRef>)>> {
    let after = |view: &PipelineDocumentView| since.is_none_or(|since| admitted_at(view).is_some_and(|at| at > since));
    let mut found: Vec<(u64, String, PipelineRef)> = Vec::new();
    for view in documents(mind, in_force(mine(PipelineKind::Memory, instance)), RECENT_READ)?.into_iter().filter(&after) {
        if let PipelineDocument::Memory(memory) = &view.document {
            found.push((view.admission.ordinal, format!("- You noted ({:?}): {}", memory.sort, quoted(&memory.text.0)), view.id.clone()));
        }
    }
    for view in documents(mind, mine(PipelineKind::Reply, instance), RECENT_READ)?.into_iter().filter(&after) {
        if let PipelineDocument::Reply(reply) = &view.document {
            found.push((view.admission.ordinal, format!("- Your Self said: {}", quoted(&reply.text.0)), view.id.clone()));
        }
    }
    for view in documents(mind, mine(PipelineKind::Resolution, instance), RECENT_READ)?.into_iter().filter(&after) {
        if let PipelineDocument::Resolution(resolution) = &view.document
            && resolution.subject.kind == PipelineKind::Run
        {
            let outcome: &ResolutionOutcome = &resolution.outcome;
            found.push((view.admission.ordinal, format!("- Run {} closed: {}", resolution.subject.id.0, outcome_words(outcome)), view.id.clone()));
        }
    }
    found.sort_by_key(|(ordinal, ..)| std::cmp::Reverse(*ordinal));
    Ok(found.into_iter().map(|(_, line, id)| (line, Some(id))).collect())
}

/// Her stewarded repos' activity by others is the forge's word, not a document; a daemon with no forge adds nothing.
/// Only what happened after `since` is news, and the notice is stamped with when it happened.
fn repo_news(mind: &dyn MindPort, since: Option<DateTime<Utc>>, responsibilities: &Responsibilities, now: DateTime<Utc>) -> Option<(String, Option<PipelineRef>)> {
    if responsibilities.repos.is_empty() {
        return None;
    }
    let mut activity = mind.stewarded_activity(&responsibilities.repos, None).ok()?;
    if let Some(since) = since {
        let since_ms = u64::try_from(since.timestamp_millis()).unwrap_or(0);
        // A capped repo's unread entries are older than the oldest it showed. Once the read shows an event of the repo at or before
        // `since` they are older than `since` too; until then the cap is disclosed, whatever other repos showed.
        let decided: Vec<String> = activity.events.iter().filter(|event| event.at <= since_ms).map(|event| event.repo.identity()).collect();
        activity.capped.retain(|repo| !decided.contains(&repo.identity()));
        activity.events.retain(|event| event.at > since_ms);
    }
    let happened = activity.events.iter().map(|event| event.at).max().unwrap_or_else(|| u64::try_from(now.timestamp_millis()).unwrap_or(0));
    repo_notice_line(&activity, happened).map(|line| (format!("- {line}"), None))
}

/// One in-force memory of hers older than `UNBIDDEN_AFTER_DAYS`, picked uniformly by `pick`.
fn unbidden(mind: &dyn MindPort, instance: &Slug, now: DateTime<Utc>, pick: u64) -> Result<Option<(String, PipelineRef)>> {
    let mut old: Vec<_> = headers(mind, &in_force(mine(PipelineKind::Memory, instance)))?
        .into_iter()
        .filter(|header| DateTime::parse_from_rfc3339(&header.admission.admitted_at).is_ok_and(|at| now - at.with_timezone(&Utc) > Span::days(UNBIDDEN_AFTER_DAYS)))
        .map(|header| (header.admission.ordinal, header.id))
        .collect();
    if old.is_empty() {
        return Ok(None);
    }
    old.sort_by_key(|(ordinal, _)| *ordinal);
    let (_, id) = old.swap_remove(usize::try_from(pick % old.len() as u64).unwrap_or(0));
    let one = cultnet_rs::Selection { keys: Some(vec![id.id.0.clone()]), projection: "document".into(), ..of_kind(PipelineKind::Memory) };
    let PipelinePageItems::Documents(views) = mind.query(&one)?.items else { bail!("the mind answered headers to a document selection") };
    Ok(views.into_iter().find_map(|view| match view.document {
        PipelineDocument::Memory(memory) => Some((format!("Out of nowhere you recall: {}", quoted(&memory.text.0)), id.clone())),
        _ => None,
    }))
}

/// The seed, rebuilt from her mind and nothing else. `since` is her newest thought's admission and `fresh` what `news` found.
fn build_seed(mind: &dyn MindPort, persona: &PersonaRead, since: Option<DateTime<Utc>>, now: DateTime<Utc>, responsibilities: &Responsibilities, fresh: Vec<(String, Option<PipelineRef>)>) -> Result<Seed> {
    let instance = mind.instance().clone();
    let state = &persona.state;
    let mut reads: Vec<PipelineRef> = Vec::new();
    let mut cite = |id: PipelineRef| {
        if !reads.contains(&id) {
            reads.push(id);
        }
    };

    let runs = recent_runs(mind, &instance)?;
    let people = bonded_people(state);
    let cue = Cue { senders: people.iter().map(String::as_str).collect(), texts: Vec::new(), names: BTreeMap::new(), listed: &responsibilities.listed, now };
    let recalled = recall::select(mind, &cue)?;
    let heard = replies(mind, &instance)?;
    let heard_section = section("What your Self said lately:", heard.iter().map(|(_, text)| format!("- {}", quoted(text))).collect());

    let sections = [
        values_section(state),
        pressures_section(state),
        bonds_section(state),
        section("Your latest runs:", runs.iter().map(|run| format!("- {run}")).collect()),
        responsibilities.text.clone(),
        recall::render(&recalled),
        heard_section,
    ];
    let kept: Vec<String> = sections.iter().zip(BUDGETS).map(|(text, (_, budget))| cap(text, budget)).collect();
    // The reads are what the Projector was shown after the bounds: the recall lines and replies the capped sections kept.
    // Each shown line is one citation: a memory reading like another takes its own line, not a share of the first's.
    let mut shown: Vec<&str> = kept[5].lines().collect();
    for entry in recalled.entries() {
        if let Some(at) = shown.iter().position(|line| *line == entry.line.trim_end()) {
            shown.remove(at);
            cite(PipelineRef { kind: PipelineKind::Memory, id: Short(entry.id.clone()) });
        }
    }
    let replies_shown = kept[6].lines().count().saturating_sub(1);
    heard.iter().take(replies_shown).for_each(|(id, _)| cite(id.clone()));
    let context = kept.iter().filter(|text| !text.trim().is_empty()).cloned().collect::<Vec<_>>().join("\n");

    let has_news = !fresh.is_empty();
    let stimulus = if has_news {
        let body: Vec<&str> = fresh.iter().map(|(line, _)| line.as_str()).collect();
        let kept = cap(&format!("Since your last thought:\n{}", body.join("\n")), SECTION_MAX_BYTES);
        // Each line is one line (`quoted` escapes newlines), so the lines the bound kept are what the thought read.
        let shown = kept.lines().count().saturating_sub(1);
        fresh.into_iter().take(shown).filter_map(|(_, id)| id).for_each(&mut cite);
        kept
    } else if let Some((line, id)) = unbidden(mind, &instance, now, u64::try_from(now.timestamp()).unwrap_or(0))? {
        cite(id);
        line
    } else {
        let at = since.map_or_else(|| "you began".to_owned(), |at| at.format("%-d %B %H:%M UTC").to_string());
        format!("Nothing has happened since {at}.")
    };
    reads.truncate(READS_MAX);
    Ok(Seed { identity: identity_of(state), context: cap(&context, SECTION_MAX_BYTES), stimulus: cap(&stimulus, SECTION_MAX_BYTES), news: has_news, reads })
}

// ---------------------------------------------------------------------------------------------
// Continuation, selection and the tick.

/// The frame the base model continues: her notebook heading, the date, the projection verbatim
/// and an open paragraph. It instructs nothing.
fn continuer_frame(name: &str, date: &str, projection: &str) -> String {
    format!("{name}'s notebook\n\n{date}\n\n{projection}\n\n")
}

/// The fixed parts of both prompts, recorded in every thought: the frame is the function's own output over placeholders.
fn prompt_version() -> String {
    format!(
        "ghostlight={GHOSTLIGHT_REV}; words={PROJECTOR_WORDS}; guidance={STREAM_GUIDANCE:?}; frame={:?}",
        continuer_frame("<name>", &format!("<date {FRAME_DATE_FORMAT}>"), "<projection>")
    )
}

/// Each candidate's nearness to her earlier thoughts, in thousandths of the daemon's top score;
/// 1000 where she has none.
fn nearness(mind: &dyn MindPort, candidates: &[String]) -> Result<Vec<u16>> {
    let members = in_force(mine(PipelineKind::Thought, mind.instance()));
    let any = mind.query(&cultnet_rs::Selection { limit: Some(1), ..members.clone() })?;
    if any.matched == 0 {
        return Ok(vec![THOUGHT_NEAREST_NONE_PERMILLE; candidates.len()]);
    }
    candidates
        .iter()
        .map(|text| {
            let page = mind.query_near(&members, text, 1)?;
            let top = page.scores.as_ref().and_then(|scores| scores.first().copied()).ok_or_else(|| anyhow!("the index scored no earlier thought"))?;
            Ok((f64::from(top) * 1000.0).round().clamp(0.0, 1000.0) as u16)
        })
        .collect()
}

/// The least near candidate, the first on a tie.
fn least_near(nearest: &[u16]) -> usize {
    nearest.iter().enumerate().min_by_key(|(at, near)| (**near, *at)).map_or(0, |(at, _)| at)
}

/// How a tick ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// A brake is held: nothing was read, asked or admitted.
    Held(crate::control::HeldReason),
    /// Not due by the cadence.
    NotDue,
    /// Every continuation was blank; nothing was admitted. The loop waits `STREAM_MIN_GAP_S`.
    Blank,
    /// The thought; admitted unless the tick was a dry run.
    Thought { thought: Box<PipelineThought>, admitted: bool },
}

/// Whether a tick is a real one or the operator's look at what one would say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Live,
    /// Both models run and nothing is admitted; the cadence does not hold it back.
    DryRun,
}

pub struct Organs<'a> {
    pub mind: &'a dyn MindPort,
    pub persona: &'a dyn PersonaSource,
    pub model: &'a dyn StreamModel,
    pub projector_model: &'a str,
    pub base_model: &'a str,
}

/// Whether a tick is due: `STREAM_MIN_GAP_S` has passed and there is news, or `STREAM_IDLE_GAP_S` has.
fn due(since: Option<DateTime<Utc>>, now: DateTime<Utc>, news: bool) -> bool {
    match since {
        None => true,
        Some(at) => {
            let gap = (now - at).num_seconds();
            gap >= STREAM_IDLE_GAP_S || (gap >= STREAM_MIN_GAP_S && news)
        }
    }
}

/// When her newest thought was ticked: its label's instant, never later than its admission. The models take minutes between the
/// two, and what the mind or the forge admitted meanwhile is news to the next thought, which the admission time would hide.
fn newest_thought(mind: &dyn MindPort) -> Result<Option<DateTime<Utc>>> {
    let newest = cultnet_rs::Selection { descending: true, limit: Some(1), ..mine(PipelineKind::Thought, mind.instance()) };
    let PipelinePageItems::Headers(found) = mind.query(&newest)?.items else { bail!("the mind answered documents to a header selection") };
    Ok(found
        .iter()
        .filter_map(|header| {
            let admitted = DateTime::parse_from_rfc3339(&header.admission.admitted_at).ok()?.with_timezone(&Utc);
            let label = header.id.id.0.rsplit(':').next().and_then(|label| label.strip_prefix('t')).and_then(|label| label.parse::<i64>().ok()).and_then(|label| DateTime::from_timestamp(label, 0));
            Some(label.map_or(admitted, |at| at.min(admitted)))
        })
        .max())
}

fn bounded(text: &str, max: usize) -> String {
    cap(text.trim(), max)
}

/// One tick. A fault is an `Err`; the caller prints it and the process goes on.
pub fn tick(organs: &Organs, brake: &dyn Fn() -> Effective, mode: Mode, now: DateTime<Utc>) -> Result<Outcome> {
    if let Effective::Held { reason } = brake() {
        return Ok(Outcome::Held(reason));
    }
    let mind = organs.mind;
    let since = newest_thought(mind)?;
    if mode == Mode::Live && since.is_some_and(|at| (now - at).num_seconds() < STREAM_MIN_GAP_S) {
        return Ok(Outcome::NotDue);
    }
    // Due-ness is decided on the cheap reads; the seed (recall, runs, replies) is built only for a tick that may run.
    let responsibilities = render_responsibilities(mind, mind.instance(), &BTreeMap::new())?;
    let mut fresh = mind_news(mind, mind.instance(), since)?;
    // The forge is read when a tick will run anyway, and on a quiet stretch only by the poll that crosses each minimum-gap mark.
    let quiet = mode == Mode::Live && !due(since, now, !fresh.is_empty());
    if quiet && since.is_some_and(|at| (now - at).num_seconds() % STREAM_MIN_GAP_S >= STREAM_POLL_S) {
        return Ok(Outcome::NotDue);
    }
    fresh.extend(repo_news(mind, since, &responsibilities, now));
    if quiet && fresh.is_empty() {
        return Ok(Outcome::NotDue);
    }
    let persona = organs.persona.persona()?;
    let seed = build_seed(mind, &persona, since, now, &responsibilities, fresh)?;

    let began = std::time::Instant::now();
    let projection = bounded(&organs.model.project(&seed.projector_prompt())?, PARA_MAX_BYTES);
    let projector_ms = u32::try_from(began.elapsed().as_millis()).unwrap_or(u32::MAX);
    if projection.is_empty() {
        return Ok(Outcome::Blank);
    }
    let frame = continuer_frame(text_of(&persona.state, &["publicName"]), &now.format(FRAME_DATE_FORMAT).to_string(), &projection);
    let began = std::time::Instant::now();
    let candidates: Vec<String> = organs
        .model
        .continue_raw(&frame, &Sampling::STREAM, CONTINUATIONS)?
        .iter()
        .map(|text| bounded(text, LINE_MAX_BYTES))
        .filter(|text| !text.is_empty())
        .take(THOUGHT_CANDIDATES_MAX)
        .collect();
    let continuer_ms = u32::try_from(began.elapsed().as_millis()).unwrap_or(u32::MAX);
    if candidates.is_empty() {
        return Ok(Outcome::Blank);
    }
    let nearest_permille = nearness(mind, &candidates)?;
    let chosen = least_near(&nearest_permille);
    let thought = PipelineThought {
        instance: mind.instance().clone(),
        label: Label::from(format!("t{}", now.timestamp()).as_str()),
        text: Para::from(candidates[chosen].as_str()),
        reads: seed.reads.clone(),
        persona_receipt: Short(persona.receipt_id.clone()),
        identity: Line::from(seed.identity.as_str()),
        context: Para::from(seed.context.as_str()),
        stimulus: Para::from(seed.stimulus.as_str()),
        projector_model: Short(organs.projector_model.to_owned()),
        projection: Para::from(projection.as_str()),
        base_model: Short(organs.base_model.to_owned()),
        temperature_permille: Sampling::STREAM.temperature_permille,
        min_p_permille: Sampling::STREAM.min_p_permille,
        max_tokens: Sampling::STREAM.max_tokens,
        candidates: candidates.iter().map(|text| Line::from(text.as_str())).collect(),
        nearest_permille,
        chosen: u8::try_from(chosen).unwrap_or(0),
        projector_ms,
        continuer_ms,
        prompt_version: Line::from(prompt_version().as_str()),
    };
    if mode == Mode::DryRun {
        return Ok(Outcome::Thought { thought: Box::new(thought), admitted: false });
    }
    let label = thought.label.0.clone();
    match mind.admit(AGENT, &format!("stream-{label}"), vec![PipelineDocument::Thought(thought.clone())])? {
        PipelineAdmissionOutcome::Committed { .. } => Ok(Outcome::Thought { thought: Box::new(thought), admitted: true }),
        other => Err(anyhow!("the mind did not admit the thought: {other:?}")),
    }
}

#[cfg(test)]
mod tests;
