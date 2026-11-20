//! `persona-tuning.toml`: every knob of the Select stage, the Projector and the Persona stage, read at each wake.
//!
//! The file is optional. Absent, the code's defaults hold and the source reads `defaults`; present, every key it
//! names overrides one default and any key the code does not know refuses the file, so a mistyped experiment fails
//! loudly instead of running on the default it meant to change. A value out of its range refuses the file too.
//!
//! The SHA-256 carried with every packet is of the effective tuning, serialized whole with its defaults: a file that
//! spells out the defaults hashes as no file does, and two builds whose defaults differ hash differently.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Where the tuning of an instance lives.
pub const TUNING_ROOT: &str = "/etc/gamecult/minds";
pub const TUNING_FILE: &str = "persona-tuning.toml";

/// The Persona stage's setting (map section 2): she continues the Projector's paragraph, her interior, with speech only
/// where her thought moves her to it, quoted (map section 5.5, invariant 5). It is not a reply to the room.
const PERSONA_GUIDANCE: &str = "Continue her inner stream where the paragraph leaves off: her own next thoughts, in the first person, in her own words. This is not a message to the room. If she means to say something aloud, she thinks it as she would, what and to whom; whether anything is said, and what, is decided after her. In a busy room most moments pass with nothing said.";

/// The setting the Projector renders for. No task: in the Persona-only shape nothing is work she owes.
const GUIDANCE: &str = "This person is a Mind of the GameCult swarm. She lives in its chat rooms among the operator, the other Minds and whoever drops by. Render the moment as she lives it: who is here and how they sit with her, what she carries, what she wants. What was said reaches her word for word after this stream, so never retell or summarise it: render what it stirs in her and what she makes of the people who said it. Render the conversation as it stands. Do not weigh options or list what she might do or say; what she does is hers to decide. Leave out whatever the context does not carry: an absence is not part of her moment. Use each person's pronouns as given.";

/// The Interpreter's speech step (map section 4, C3b): its scope and rules beside Ghostlight's membrane. It has no tools;
/// what the membrane calls the gap tool is the `gaps` list of the JSON.
const INTERPRETER_GUIDANCE: &str = "This step decides only what she says aloud in chat this turn, from her prose. No tool is available in this step: report every translation and every gap in the JSON object the contract describes, and return that object alone. A line is speech only when her prose shows her choosing to say it now, to the room or to someone in it. Words she recalls, quotes, or heard from someone else are never her speech, even when her prose repeats them. Thinking about saying something, deciding to stay quiet, or saying that she says nothing, is not speech. Write each line as she would type it, in her voice, without @handles or quotation marks around it, and copy into source_quote the exact words of her prose the line rests on, character for character. Name in mentions, by Matrix id from the people listed, only someone she addresses directly; in reply_to, the Matrix id of the person whose message she answers, when she answers one. The room is one of the rooms listed. An empty say list is the usual answer in a busy room.";

/// The context room the fixed slots need beside the identity: the hour and the mood lines always fit.
const FIXED_ROOM: usize = 256;
/// The temperatures a selection may run at: colder is deterministic in all but name, hotter is noise.
const TEMPERATURE_RANGE: std::ops::RangeInclusive<f64> = 0.01..=10.0;
/// The ceiling of the messages to her: at least a long message whole, and far under the connector's 256 KiB frame, which
/// also carries the rest of the prompt.
const ADDRESSED_RANGE: std::ops::RangeInclusive<usize> = 1024..=64 * 1024;
/// The most bytes any one part of the prompt may take: with the identity, the guidance and the messages to her
/// (`ADDRESSED_RANGE`), the Projector's prompt stays far under the connector's 256 KiB frame.
const PART_BYTES_MAX: usize = 32 * 1024;
/// The most words a lived stream may be given: a thin moment gets a thin stream, and no budget needs a book.
const WORDS_MAX: usize = 2000;
/// The most room lines the group state reads: a recent window, not the room's history.
const GROUP_LINES_MAX: usize = 40;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tuning {
    pub projector: ProjectorTuning,
    pub persona: PersonaTuning,
    pub interpreter: InterpreterTuning,
    pub select: SelectTuning,
    pub lens: LensWeights,
    /// Each person's pronouns by Matrix id, for everyone the packet names: the persona schema has no pronoun field for
    /// another person. A file's table replaces this one whole; anyone not listed is they/them.
    pub pronouns: BTreeMap<String, String>,
}

/// The swarm's people as of 2026-11-20 (operator's word): she for the operator and five Minds, he for Heimdall and Deru,
/// any for Void, rendered they/them.
const PRONOUNS: [(&str, &str); 10] = [
    ("@meta:gamecult.org", "she/her"),
    ("@libby:gamecult.org", "she/her"),
    ("@kiko:gamecult.org", "she/her"),
    ("@aqua:gamecult.org", "she/her"),
    ("@weksa:gamecult.org", "she/her"),
    ("@nibu:gamecult.org", "she/her"),
    ("@eureka:gamecult.org", "she/her"),
    ("@heimdall:gamecult.org", "he/him"),
    ("@void:gamecult.org", "they/them (any pronouns)"),
    ("@deru:gamecult.org", "he/him"),
];

impl Default for Tuning {
    fn default() -> Self {
        Self {
            projector: ProjectorTuning::default(),
            persona: PersonaTuning::default(),
            interpreter: InterpreterTuning::default(),
            select: SelectTuning::default(),
            lens: LensWeights::default(),
            pronouns: PRONOUNS.iter().map(|(id, pronouns)| ((*id).to_owned(), (*pronouns).to_owned())).collect(),
        }
    }
}

/// The Projector's call: its model, its domain guidance and its word budgets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectorTuning {
    pub model: String,
    pub guidance: String,
    /// The lived stream's words on a chat, follow-up or reflection turn.
    pub words: usize,
    /// On a beat: a thin moment gets a thin stream.
    pub beat_words: usize,
}

/// The Persona stage's call: its model, its voice and setting guidance and its word budget (map section 3.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PersonaTuning {
    pub model: String,
    pub guidance: String,
    pub words: usize,
}

/// The selection: its temperature and inclusion, its budgets, its half-life, the freshness decay and the pool caps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SelectTuning {
    /// The softmax temperature that orders the candidates.
    pub temperature: f64,
    /// The score at which a candidate is included 63% of the time: inclusion is `1 - exp(-score / inclusion_score)`.
    pub inclusion_score: f64,
    /// Identity plus typed context, in bytes.
    pub context_bytes: usize,
    pub stimulus_bytes: usize,
    /// What was said to her, verbatim, for the Persona stage.
    pub heard_bytes: usize,
    /// The ceiling of the messages to her, in the stimulus and in the heard lines alike: they pass the stimulus and heard
    /// caps whole, newest first, up to this, and the older ones are counted.
    pub addressed_bytes: usize,
    pub identity_bytes: usize,
    /// One bond line's summary, in bytes.
    pub bond_summary_bytes: usize,
    /// One selected item's text, in bytes.
    pub item_bytes: usize,
    /// The recency time constant of her memories and short-term items, in hours.
    pub memory_tau_h: f64,
    /// freshness = base ^ (times shown in the last `freshness_window` projections); 0 never repeats an item in the window.
    pub freshness_base: f64,
    pub freshness_window: usize,
    /// People present shown with a full bond line; the rest are named.
    pub people_full: usize,
    /// The room lines a beat shows.
    pub beat_tail_lines: usize,
    /// The group-state slot: who has been talking to whom, how long since a human spoke, what is addressed to her or
    /// open to anyone, and how long the run of Mind lines is (prior art: group state in the prompt is what lets a
    /// participant judge whether to speak).
    pub group_state: bool,
    /// The room lines the group state reads.
    pub group_lines: usize,
    /// The group-state slot's bytes.
    pub group_bytes: usize,
    pub caps: PoolCaps,
}

/// The most items each pool may give a packet. A maximum, never a target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PoolCaps {
    pub memories_about_present: usize,
    pub other_memories: usize,
    pub needs: usize,
    pub incubation: usize,
    pub stance: usize,
    pub value: usize,
    pub said: usize,
}

/// The weighted draw of the lens a turn begins from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LensWeights {
    pub body_and_hour: f64,
    pub person_present: f64,
    pub unresolved_want: f64,
    pub memory_surfacing: f64,
    pub wondering: f64,
}

impl Default for ProjectorTuning {
    fn default() -> Self {
        Self { model: "claude-sonnet-5-5".into(), guidance: GUIDANCE.into(), words: 150, beat_words: 90 }
    }
}

/// The Interpreter's speech call: its model and its guidance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InterpreterTuning {
    pub model: String,
    pub guidance: String,
}

impl Default for InterpreterTuning {
    fn default() -> Self {
        Self { model: "claude-sonnet-5-5".into(), guidance: INTERPRETER_GUIDANCE.into() }
    }
}

impl Default for PersonaTuning {
    fn default() -> Self {
        Self { model: "claude-sonnet-5-5".into(), guidance: PERSONA_GUIDANCE.into(), words: 180 }
    }
}

impl Default for SelectTuning {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            inclusion_score: 0.5,
            context_bytes: 2500,
            stimulus_bytes: 1500,
            heard_bytes: 1500,
            addressed_bytes: 8 * 1024,
            identity_bytes: 300,
            bond_summary_bytes: 300,
            item_bytes: 400,
            memory_tau_h: 24.0,
            freshness_base: 0.5,
            freshness_window: 5,
            people_full: 2,
            beat_tail_lines: 6,
            group_state: true,
            group_lines: 12,
            group_bytes: 360,
            caps: PoolCaps::default(),
        }
    }
}

impl Default for PoolCaps {
    fn default() -> Self {
        Self { memories_about_present: 2, other_memories: 2, needs: 1, incubation: 1, stance: 1, value: 1, said: 1 }
    }
}

impl Default for LensWeights {
    fn default() -> Self {
        Self { body_and_hour: 1.0, person_present: 1.0, unresolved_want: 1.0, memory_surfacing: 1.0, wondering: 1.0 }
    }
}

impl Tuning {
    /// Refuses a value the stages cannot run on, naming the knob.
    pub(crate) fn validate(&self) -> Result<()> {
        let select = &self.select;
        let positive = |name: &str, value: f64| if value.is_finite() && value > 0.0 { Ok(()) } else { Err(anyhow::anyhow!("{name} must be a finite number above 0, not {value}")) };
        if !(select.temperature.is_finite() && TEMPERATURE_RANGE.contains(&select.temperature)) {
            bail!("select.temperature must be from {} to {}, not {}", TEMPERATURE_RANGE.start(), TEMPERATURE_RANGE.end(), select.temperature);
        }
        if self.projector.model.trim().is_empty() {
            bail!("projector.model must name a model");
        }
        if self.interpreter.model.trim().is_empty() {
            bail!("interpreter.model must name a model");
        }
        if self.persona.model.trim().is_empty() {
            bail!("persona.model must name a model");
        }
        if !ADDRESSED_RANGE.contains(&select.addressed_bytes) {
            bail!("select.addressed_bytes must be from {} to {}, not {}", ADDRESSED_RANGE.start(), ADDRESSED_RANGE.end(), select.addressed_bytes);
        }
        let caps = &select.caps;
        if [caps.memories_about_present, caps.other_memories, caps.needs, caps.incubation, caps.stance, caps.value, caps.said].iter().all(|cap| *cap == 0) {
            bail!("select.caps are all 0: a packet could hold nothing of her");
        }
        positive("select.inclusion_score", select.inclusion_score)?;
        positive("select.memory_tau_h", select.memory_tau_h)?;
        if !(select.freshness_base.is_finite() && (0.0..=1.0).contains(&select.freshness_base)) {
            bail!("select.freshness_base must be from 0 to 1, not {}", select.freshness_base);
        }
        for (name, bytes) in [
            ("select.context_bytes", select.context_bytes),
            ("select.stimulus_bytes", select.stimulus_bytes),
            ("select.heard_bytes", select.heard_bytes),
            ("select.identity_bytes", select.identity_bytes),
            ("select.bond_summary_bytes", select.bond_summary_bytes),
            ("select.item_bytes", select.item_bytes),
            ("select.group_lines", select.group_lines),
            ("select.group_bytes", select.group_bytes),
            ("projector.words", self.projector.words),
            ("projector.beat_words", self.projector.beat_words),
            ("persona.words", self.persona.words),
        ] {
            if bytes == 0 {
                bail!("{name} must be above 0");
            }
        }
        for (name, bytes) in [
            ("select.context_bytes", select.context_bytes),
            ("select.stimulus_bytes", select.stimulus_bytes),
            ("select.heard_bytes", select.heard_bytes),
            ("select.identity_bytes", select.identity_bytes),
            ("select.bond_summary_bytes", select.bond_summary_bytes),
            ("select.item_bytes", select.item_bytes),
            ("select.group_bytes", select.group_bytes),
            ("projector.guidance", self.projector.guidance.len()),
            ("persona.guidance", self.persona.guidance.len()),
            ("interpreter.guidance", self.interpreter.guidance.len()),
        ] {
            if bytes > PART_BYTES_MAX {
                bail!("{name} must be at most {PART_BYTES_MAX} bytes, not {bytes}: the prompt must fit the connector's frame");
            }
        }
        if select.group_lines > GROUP_LINES_MAX {
            bail!("select.group_lines must be at most {GROUP_LINES_MAX}, not {}", select.group_lines);
        }
        for (name, words) in [("projector.words", self.projector.words), ("projector.beat_words", self.projector.beat_words), ("persona.words", self.persona.words)] {
            if words > WORDS_MAX {
                bail!("{name} must be at most {WORDS_MAX}, not {words}");
            }
        }
        if select.identity_bytes.checked_add(FIXED_ROOM).is_none_or(|needed| select.context_bytes < needed) {
            bail!("select.context_bytes must leave {FIXED_ROOM} bytes beside select.identity_bytes for the hour and the mood");
        }
        let lens = &self.lens;
        let weights = [lens.body_and_hour, lens.person_present, lens.unresolved_want, lens.memory_surfacing, lens.wondering];
        if weights.iter().any(|weight| !weight.is_finite() || *weight < 0.0) || lens.body_and_hour <= 0.0 {
            bail!("lens weights must be finite and not negative, and lens.body_and_hour, the one lens every turn has, above 0");
        }
        Ok(())
    }

    /// The SHA-256 of the effective tuning, serialized whole.
    pub(crate) fn sha256(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// The tuning a wake ran under, and where it came from.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Loaded {
    pub tuning: Tuning,
    /// `file` or `defaults`.
    pub source: &'static str,
    /// The effective tuning's SHA-256, hex.
    pub sha256: String,
}

/// The tuning at `path`, or the defaults when no file is there. A file that cannot be read or parsed, that names a key
/// the code does not know, or that sets a value out of its range is an error: the wake does not run on a tuning nobody
/// meant.
pub fn load(path: &Path) -> Result<Loaded> {
    let (tuning, source) = match std::fs::read(path) {
        Ok(bytes) => {
            let text = std::str::from_utf8(&bytes).with_context(|| format!("the tuning {} is not UTF-8", path.display()))?;
            (toml::from_str::<Tuning>(text).with_context(|| format!("the tuning {} is not a tuning", path.display()))?, "file")
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Tuning::default(), "defaults"),
        Err(error) => return Err(error).with_context(|| format!("the tuning {} could not be read", path.display())),
    };
    tuning.validate().with_context(|| format!("the tuning {} is out of range", path.display()))?;
    let sha256 = tuning.sha256();
    Ok(Loaded { tuning, source, sha256 })
}
