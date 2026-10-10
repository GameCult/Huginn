//! The campaign progress view as named subcommands. Each recipe is exactly the
//! selection `references/campaign-state.md` gives (Eureka 278b881, "Recipes"),
//! built per call and never cached. Each one's `--help` prints its selection.

use clap::{Args, Subcommand};
use huginn_mind::SemanticQuery;
use huginn_mind::eureka_pipeline::{Line, PipelineKind};
use serde_json::{Value, json};

use crate::door;
use crate::trouble::Trouble;

/// The campaign whose `gap-` follow-ups are the substrate's known gaps.
const GAP_HOME: &str = "eureka-substrate";
/// How many neighbours `precedent` asks the index for.
const PRECEDENT_TOP_K: u32 = 10;
/// Severities that block a cut.
const BLOCKING: [&str; 2] = ["Blocker", "High"];

#[derive(Args, Clone, Debug, Default)]
pub struct Paging {
    /// The `next` cursor of the previous page.
    #[arg(long)]
    pub cursor: Option<String>,
    /// Print each item as `view` does, not as a header line.
    #[arg(long)]
    pub documents: bool,
}

#[derive(Subcommand, Debug)]
pub enum Recipe {
    /// Every campaign.
    Campaigns(Paging),
    /// The target in force.
    Target {
        campaign: String,
        #[command(flatten)]
        page: Paging,
    },
    /// Rulings in force, operator directions included.
    Rulings {
        #[arg(required = true)]
        campaigns: Vec<String>,
        #[command(flatten)]
        page: Paging,
    },
    /// Open questions and follow-ups.
    Open {
        #[arg(required = true)]
        campaigns: Vec<String>,
        #[command(flatten)]
        page: Paging,
    },
    /// Open findings admitted by Soul.
    Findings {
        campaign: String,
        /// Only Blocker and High.
        #[arg(long)]
        blocking: bool,
        #[command(flatten)]
        page: Paging,
    },
    /// Specs with no report.
    Unreported {
        campaign: String,
        #[command(flatten)]
        page: Paging,
    },
    /// Open questions raised in a spec: the spec waits on the operator.
    Blocked {
        spec_id: String,
        #[command(flatten)]
        page: Paging,
    },
    /// Reports with no verdict: Soul's queue.
    Unjudged {
        campaign: String,
        #[command(flatten)]
        page: Paging,
    },
    /// A subject's resolutions, newest first.
    History {
        id: String,
        #[command(flatten)]
        page: Paging,
    },
    /// One cut's spec, reports, verdicts, findings and resolutions.
    Cut {
        campaign: String,
        label: String,
        #[command(flatten)]
        page: Paging,
    },
    /// Every spec and report, whole.
    Ledger {
        campaign: String,
        #[command(flatten)]
        page: Paging,
    },
    /// Rulings, resolutions and findings nearest a question in plain words.
    Precedent {
        campaign: String,
        text: String,
        #[command(flatten)]
        page: Paging,
    },
    /// The substrate's known gaps.
    Gaps(Paging),
}

/// What a recipe asks the mind: the selection, the semantic part when it has
/// one, and the caller's paging flags.
pub struct Ask {
    pub selection: Value,
    pub semantic: Option<SemanticQuery>,
    pub page: Paging,
}

impl Recipe {
    pub fn ask(self) -> Result<Ask, Trouble> {
        let (selection, semantic, page) = match self {
            Self::Campaigns(page) => (campaigns(), None, page),
            Self::Target { campaign, page } => (target(&campaign), None, page),
            Self::Rulings { campaigns, page } => (rulings(&campaigns), None, page),
            Self::Open { campaigns, page } => (open(&campaigns), None, page),
            Self::Findings { campaign, blocking, page } => (findings(&campaign, blocking), None, page),
            Self::Unreported { campaign, page } => (unreported(&campaign), None, page),
            Self::Blocked { spec_id, page } => (blocked(&spec_id), None, page),
            Self::Unjudged { campaign, page } => (unjudged(&campaign), None, page),
            Self::History { id, page } => (history(door::kind_of(&id)?.type_id(), &id), None, page),
            Self::Cut { campaign, label, page } => (cut(&campaign, &label), None, page),
            Self::Ledger { campaign, page } => (ledger(&campaign), None, page),
            Self::Precedent { campaign, text, page } => {
                (precedent(&campaign), Some(SemanticQuery { text: Line(text), top_k: PRECEDENT_TOP_K }), page)
            }
            Self::Gaps(page) => (gaps(), None, page),
        };
        Ok(Ask { selection, semantic, page })
    }
}

fn schema(kind: PipelineKind) -> &'static str {
    kind.type_id()
}

fn root(campaigns: &[String]) -> Value {
    json!({ "index": "root", "op": "any_of", "values": campaigns })
}

fn in_force() -> Value {
    json!({ "index": "in_force", "op": "any_of", "values": ["true"] })
}

fn severity_blocking() -> Value {
    json!({ "index": "severity", "op": "any_of", "values": BLOCKING })
}

fn campaigns() -> Value {
    json!({ "schemas": [schema(PipelineKind::Campaign)] })
}

fn target(campaign: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::Target)],
        "fields": [root(&[campaign.to_string()]), in_force()],
        "projection": "document",
    })
}

fn rulings(campaigns: &[String]) -> Value {
    json!({ "schemas": [schema(PipelineKind::Ruling)], "fields": [root(campaigns), in_force()] })
}

fn open(campaigns: &[String]) -> Value {
    json!({
        "schemas": [schema(PipelineKind::Question), schema(PipelineKind::FollowUp)],
        "fields": [root(campaigns), in_force()],
    })
}

fn findings(campaign: &str, blocking: bool) -> Value {
    let mut fields = vec![
        root(&[campaign.to_string()]),
        in_force(),
        json!({ "index": "faculty", "op": "any_of", "values": ["Soul"] }),
    ];
    if blocking {
        fields.push(severity_blocking());
    }
    json!({ "schemas": [schema(PipelineKind::Finding)], "fields": fields })
}

fn unreported(campaign: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::CutSpec)],
        "fields": [root(&[campaign.to_string()]), in_force()],
        "cited": { "role": "cut_spec", "exists": false },
    })
}

fn blocked(spec_id: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::Question)],
        "fields": [in_force()],
        "cites": { "target": { "schemaId": schema(PipelineKind::CutSpec), "recordKey": spec_id }, "role": "raised_in" },
    })
}

fn unjudged(campaign: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::CutReport)],
        "fields": [root(&[campaign.to_string()])],
        "cited": { "role": "cut_report", "exists": false },
    })
}

fn history(schema_id: &str, id: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::Resolution)],
        "cites": { "target": { "schemaId": schema_id, "recordKey": id }, "role": "subject" },
        "descending": true,
    })
}

fn cut(campaign: &str, label: &str) -> Value {
    json!({
        "schemas": [
            schema(PipelineKind::CutSpec),
            schema(PipelineKind::CutReport),
            schema(PipelineKind::Verdict),
            schema(PipelineKind::Finding),
            schema(PipelineKind::Resolution),
        ],
        "fields": [root(&[campaign.to_string()]), { "index": "cut", "op": "any_of", "values": [label] }],
    })
}

fn ledger(campaign: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::CutSpec), schema(PipelineKind::CutReport)],
        "fields": [root(&[campaign.to_string()])],
        "projection": "document",
    })
}

fn precedent(campaign: &str) -> Value {
    json!({
        "schemas": [schema(PipelineKind::Ruling), schema(PipelineKind::Resolution), schema(PipelineKind::Finding)],
        "fields": [root(&[campaign.to_string()])],
    })
}

fn gaps() -> Value {
    json!({
        "schemas": [schema(PipelineKind::FollowUp)],
        "fields": [root(&[GAP_HOME.to_string()]), in_force()],
    })
}

/// Each recipe's `--help` addendum: its selection with placeholders in the
/// arguments' places. The same builders as execution, so help cannot drift
/// from what runs.
pub fn help() -> Vec<(&'static str, String)> {
    let c = || "<c>".to_string();
    let selection = |value: Value| format!("selection:\n{}", serde_json::to_string_pretty(&value).unwrap_or_default());
    let findings_help =
        format!("{}\n--blocking adds the field predicate:\n{}", selection(findings(&c(), false)), severity_blocking());
    let precedent_help = format!(
        "{}\nsemantic: {{ \"text\": \"<text>\", \"top_k\": {PRECEDENT_TOP_K} }}",
        selection(precedent(&c()))
    );
    vec![
        ("campaigns", selection(campaigns())),
        ("target", selection(target(&c()))),
        ("rulings", selection(rulings(&[c()]))),
        ("open", selection(open(&[c()]))),
        ("findings", findings_help),
        ("unreported", selection(unreported(&c()))),
        ("blocked", selection(blocked("<id>"))),
        ("unjudged", selection(unjudged(&c()))),
        ("history", selection(history("epiphany.pipeline.<kind>.v2", "<id>"))),
        ("cut", selection(cut(&c(), "<label>"))),
        ("ledger", selection(ledger(&c()))),
        ("precedent", precedent_help),
        ("gaps", selection(gaps())),
    ]
}
