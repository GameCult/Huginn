//! `persona-tuning.toml`: every knob of the Select stage and the Projector, read at each wake.
//!
//! The file is optional. Absent, the code's defaults hold and the source reads `defaults`; present, every key it
//! names overrides one default and any key the code does not know refuses the file, so a mistyped experiment fails
//! loudly instead of running on the default it meant to change. The file's SHA-256 is carried with every packet,
//! so a reader of the trace can tell which tuning a turn ran under.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Where the tuning of an instance lives.
pub const TUNING_ROOT: &str = "/etc/gamecult/minds";
pub const TUNING_FILE: &str = "persona-tuning.toml";

/// The setting the Projector renders for. No task: in the Persona-only shape nothing is work she owes.
const GUIDANCE: &str = "This person is a Mind of the GameCult swarm. She lives in its chat rooms among the operator, the other Minds and whoever drops by, and no task is waiting on her in this moment. Render the moment as she lives it: who is here and how they sit with her, what she carries, what she wants.";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tuning {
    pub projector: ProjectorTuning,
    pub select: SelectTuning,
    pub lens: LensWeights,
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

/// The selection: its temperature, its budgets, its half-lives, the freshness decay and the pool caps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SelectTuning {
    /// The softmax temperature of the sampled remainder.
    pub temperature: f64,
    /// Identity plus typed context, in bytes.
    pub context_bytes: usize,
    pub stimulus_bytes: usize,
    /// What was said to her, verbatim, for the Persona stage.
    pub heard_bytes: usize,
    pub identity_bytes: usize,
    /// One bond line's summary, in bytes.
    pub bond_summary_bytes: usize,
    /// One selected item's text, in bytes.
    pub item_bytes: usize,
    /// The recency time constant of her own memories and short-term items, in hours.
    pub memory_tau_h: f64,
    /// The recency time constant of what her Mind recalls, in hours.
    pub recall_tau_h: f64,
    /// freshness = base ^ (times shown in the last `freshness_window` projections).
    pub freshness_base: f64,
    pub freshness_window: usize,
    /// People present shown with a full bond line; the rest are named.
    pub people_full: usize,
    /// The room lines a beat shows.
    pub beat_tail_lines: usize,
    /// The sorts of her Mind's memories recall may offer, as the mind spells them. Her Mind is mostly the mission's
    /// record: Committed, Did and Asked are work and stay out unless an experiment names them.
    pub recall_sorts: Vec<String>,
    pub caps: PoolCaps,
}

/// Every memory sort the mind knows.
pub(crate) const MEMORY_SORTS: [&str; 7] = ["Asked", "Committed", "Decided", "Learned", "Did", "Idea", "Rapport"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PoolCaps {
    pub memories_about_present: usize,
    pub other_memories: usize,
    pub needs: usize,
    pub incubation: usize,
    pub stance: usize,
    pub recall: usize,
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

impl Default for SelectTuning {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            context_bytes: 2500,
            stimulus_bytes: 1500,
            heard_bytes: 1500,
            identity_bytes: 300,
            bond_summary_bytes: 300,
            item_bytes: 400,
            memory_tau_h: 24.0,
            recall_tau_h: 72.0,
            freshness_base: 0.5,
            freshness_window: 5,
            people_full: 2,
            beat_tail_lines: 6,
            recall_sorts: ["Learned", "Idea", "Decided", "Rapport"].map(String::from).to_vec(),
            caps: PoolCaps::default(),
        }
    }
}

impl Default for PoolCaps {
    fn default() -> Self {
        // One recalled item a packet: her own persona state is what she is made of; her Mind only seasons it.
        Self { memories_about_present: 2, other_memories: 2, needs: 1, incubation: 1, stance: 1, recall: 1, value: 1, said: 1 }
    }
}

impl Default for LensWeights {
    fn default() -> Self {
        Self { body_and_hour: 1.0, person_present: 1.0, unresolved_want: 1.0, memory_surfacing: 1.0, wondering: 1.0 }
    }
}

/// The tuning a wake ran under, and which file it was.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Loaded {
    pub tuning: Tuning,
    /// `file` or `defaults`.
    pub source: &'static str,
    /// The file's SHA-256, hex; for the defaults, the SHA-256 of no bytes.
    pub sha256: String,
}

/// The tuning at `path`, or the defaults when no file is there. A file that cannot be read or parsed, or that names a
/// key the code does not know, is an error: the wake does not run on a tuning nobody wrote.
pub fn load(path: &Path) -> Result<Loaded> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Loaded { tuning: Tuning::default(), source: "defaults", sha256: hex(&Sha256::digest(b"")) }),
        Err(error) => return Err(error).with_context(|| format!("the tuning {} could not be read", path.display())),
    };
    let text = std::str::from_utf8(&bytes).with_context(|| format!("the tuning {} is not UTF-8", path.display()))?;
    let tuning: Tuning = toml::from_str(text).with_context(|| format!("the tuning {} is not a tuning", path.display()))?;
    if let Some(unknown) = tuning.select.recall_sorts.iter().find(|sort| !MEMORY_SORTS.contains(&sort.as_str())) {
        bail!("the tuning {} names {unknown:?} in recall_sorts, which is not a memory sort ({})", path.display(), MEMORY_SORTS.join(", "));
    }
    Ok(Loaded { tuning, source: "file", sha256: hex(&Sha256::digest(&bytes)) })
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
