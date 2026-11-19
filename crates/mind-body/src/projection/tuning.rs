//! `persona-tuning.toml`: every knob of the Select stage and the Projector, read at each wake.
//!
//! The file is optional. Absent, the code's defaults hold and the source reads `defaults`; present, every key it
//! names overrides one default and any key the code does not know refuses the file, so a mistyped experiment fails
//! loudly instead of running on the default it meant to change. A value out of its range refuses the file too.
//!
//! The SHA-256 carried with every packet is of the effective tuning, serialized whole with its defaults: a file that
//! spells out the defaults hashes as no file does, and two builds whose defaults differ hash differently.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Where the tuning of an instance lives.
pub const TUNING_ROOT: &str = "/etc/gamecult/minds";
pub const TUNING_FILE: &str = "persona-tuning.toml";

/// The setting the Projector renders for. No task: in the Persona-only shape nothing is work she owes.
const GUIDANCE: &str = "This person is a Mind of the GameCult swarm. She lives in its chat rooms among the operator, the other Minds and whoever drops by, and no task is waiting on her in this moment. Render the moment as she lives it: who is here and how they sit with her, what she carries, what she wants.";

/// The context room the fixed slots need beside the identity: the hour and the mood lines always fit.
const FIXED_ROOM: usize = 256;

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

impl Default for SelectTuning {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            inclusion_score: 0.5,
            context_bytes: 2500,
            stimulus_bytes: 1500,
            heard_bytes: 1500,
            identity_bytes: 300,
            bond_summary_bytes: 300,
            item_bytes: 400,
            memory_tau_h: 24.0,
            freshness_base: 0.5,
            freshness_window: 5,
            people_full: 2,
            beat_tail_lines: 6,
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
        positive("select.temperature", select.temperature)?;
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
            ("projector.words", self.projector.words),
            ("projector.beat_words", self.projector.beat_words),
        ] {
            if bytes == 0 {
                bail!("{name} must be above 0");
            }
        }
        if select.context_bytes < select.identity_bytes + FIXED_ROOM {
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
