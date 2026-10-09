//! Eureka pipeline documents: the typed shape of a Eureka campaign's state.
//!
//! This is a leaf type library, and deliberately nothing else. It owns document
//! shape, field bounds, formats, key derivation and the JSON schemas published
//! under `schemas/cultnet`; it owns no storage, no admission, no process and no
//! network. That is the whole reason it is a package: the organ that will admit
//! these documents depends on these types without depending on the harness
//! that used to hold them.
//!
//! Every kind is a plain `serde` + `JsonSchema` value inside a one-slot
//! `DatabaseEntry` wrapper, always prepared with `prepare_entry_named`, so the
//! stored payload is `[value]` and the value is the named map the published
//! schema describes. Keys are semantic: `pipeline_key` derives them from the
//! value, and the write validator refuses any envelope whose key differs.
//!
//! Every key is `<root>:<kind>:<local>`, three segments for every kind with
//! none excused: the root is a `Slug`, the kind is the literal kind name, and
//! the local is `Label`s joined by `.`, so no local part carries a dot and a
//! reader recovers the parts by splitting. A root's local is the constant
//! `self`; a resolution's local is its subject's kind and local. A
//! resolution's local ends in its per-subject sequence, `n<N>`, and a
//! stewardship's in its per-repo sequence, so a subject's resolutions and a
//! repo's assignments each share a prefix.
//!
//! **Validation follows the type.** `value_types!` is the single field list: it
//! emits the struct and its `Bounded` impl from the same tokens, so a field
//! cannot be declared without being validated. Format rules live in the field's
//! type (`Label`, `Slug`, `OrgRepo`, `Sha`, `Sha256Hex`, `Date`), never in a
//! hand-written impl that a later field can slip past. A `Vec` field must carry
//! a maximum, because `Vec<T>` has no `Bounded` impl of its own.
//!
//! The wrappers are crate-private: outside code registers, prepares and
//! decodes them through `register_pipeline_document_types`,
//! `PipelineDocument::prepare` and `PipelineDocument::decode`, and validates a
//! write through `validate_pipeline_write_envelope`.

// `anyhow` is the cache's own error type, which `prepare` and the registrar
// pass through; every refusal this crate decides is a `PipelineRefusal`.
use anyhow::Result;
use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Typed refusals of the pipeline documents: bounds, formats and key identity
/// (D2's document half), and `ForeignStore`, raised by `decode` for an
/// envelope of any other type. This is every refusal the leaf decides; a
/// store's identity and its schema epoch are not decided here, since this
/// crate owns no store, and belong to the organ that will admit these
/// documents.
///
/// Serialised externally tagged, serde's default for an enum: each variant is
/// a one-key map from the variant's name to its named fields, so `field` and
/// `value` are on the wire under their own names and a reader of a refusal
/// carried over a wire reads the same parts a local caller matches on. No
/// attribute buys that, which is why there is none: an internal or adjacent
/// tag would move the fields under a content key without making any of them
/// more visible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PipelineRefusal {
    FieldBound { field: String, limit: u32, actual: u32 },
    InvalidFormat { field: String, value: String },
    InvalidIdentity { kind: PipelineKind, key: String, expected: String },
    DuplicateRepo { field: String, repo: String },
    ForeignStore { r#type: String },
}

impl std::fmt::Display for PipelineRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "pipeline refusal: {self:?}")
    }
}

impl std::error::Error for PipelineRefusal {}

pub(crate) trait Bounded {
    fn validate(&self, field: &str) -> Result<(), PipelineRefusal>;
}

impl<T: Bounded> Bounded for Option<T> {
    fn validate(&self, field: &str) -> Result<(), PipelineRefusal> {
        self.as_ref().map_or(Ok(()), |value| value.validate(field))
    }
}

macro_rules! unbounded_scalars {
    ($($ty:ty),* $(,)?) => {$(
        impl Bounded for $ty {
            fn validate(&self, _field: &str) -> Result<(), PipelineRefusal> {
                Ok(())
            }
        }
    )*};
}

unbounded_scalars!(u32, u64, bool);

fn bound(field: &str, limit: usize, actual: usize) -> Result<(), PipelineRefusal> {
    if actual > limit {
        return Err(PipelineRefusal::FieldBound {
            field: field.into(),
            limit: limit as u32,
            actual: actual as u32,
        });
    }
    Ok(())
}

/// List maximums only. Minimum counts are admission rules with their own
/// refusals (Cut 3b), so a bounds pass never pre-empts them.
fn list<T: Bounded>(field: &str, items: &[T], max: usize) -> Result<(), PipelineRefusal> {
    bound(field, max, items.len())?;
    for (index, item) in items.iter().enumerate() {
        item.validate(&format!("{field}[{index}]"))?;
    }
    Ok(())
}

fn format_error(field: &str, value: &str) -> PipelineRefusal {
    PipelineRefusal::InvalidFormat {
        field: field.into(),
        value: value.into(),
    }
}

fn hex(field: &str, value: &str, lengths: std::ops::RangeInclusive<usize>) -> Result<(), PipelineRefusal> {
    let lower_hex = value.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    if !lengths.contains(&value.len()) || !lower_hex {
        return Err(format_error(field, value));
    }
    Ok(())
}

/// A key label: `[A-Za-z0-9_-]{1,64}`. Dots are excluded so that a composed key
/// segments unambiguously; see `local`.
fn label_text(field: &str, value: &str) -> Result<(), PipelineRefusal> {
    let valid = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    if !valid || value.is_empty() || value.len() > 64 {
        return Err(format_error(field, value));
    }
    Ok(())
}

/// A dotted key segment: label parts joined by `.`, at most 64 bytes. Every
/// part must be a label, so `..`, a leading or trailing dot, and an empty part
/// are all refused.
fn dotted_text(field: &str, value: &str) -> Result<(), PipelineRefusal> {
    dotted_within(field, value, 64)
}

/// `dotted_text`'s grammar with the bound as a parameter: the one place a
/// dotted segment is parsed, whoever owns the bound.
fn dotted_within(field: &str, value: &str, max: usize) -> Result<(), PipelineRefusal> {
    if value.is_empty() || value.len() > max {
        return Err(format_error(field, value));
    }
    for part in value.split('.') {
        label_text(field, part)?;
    }
    Ok(())
}

/// GitHub's own grammar for `owner/repo`, exactly one `/`: the owner is
/// `[A-Za-z0-9-]`, 1 to 39 bytes, with no leading or trailing hyphen; the repo
/// is `[A-Za-z0-9._-]`, 1 to 100 bytes, is never `.` or `..`, and does not end
/// in `.git`, ASCII-case-insensitively (`a/b.GIT` is refused exactly as
/// `a/b.git` is), so the suffix rule cannot be dodged by a case variant whose
/// `identity()` is refused. The repo's own byte alphabet excludes `/`, so a
/// second `/` inside it is already refused by the byte check; no separate
/// `contains('/')` check is needed.
fn org_repo_text(field: &str, value: &str) -> Result<(), PipelineRefusal> {
    let valid = value.split_once('/').is_some_and(|(owner, repo)| {
        (1..=39).contains(&owner.len())
            && owner.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !owner.starts_with('-')
            && !owner.ends_with('-')
            && (1..=100).contains(&repo.len())
            && repo.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            && repo != "."
            && repo != ".."
            && !(repo.len() >= 4 && repo[repo.len() - 4..].eq_ignore_ascii_case(".git"))
    });
    if valid {
        Ok(())
    } else {
        Err(format_error(field, value))
    }
}

macro_rules! bounded_text {
    ($($(#[$doc:meta])* $name:ident = $limit:literal, $check:expr;)*) => {$(
        #[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        #[schemars(extend("maxLength" = $limit))]
        $(#[$doc])*
        pub struct $name(pub String);
        impl Bounded for $name {
            fn validate(&self, field: &str) -> Result<(), PipelineRefusal> {
                let check: fn(&str, &str) -> Result<(), PipelineRefusal> = $check;
                check(field, &self.0)
            }
        }
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.into())
            }
        }
    )*};
}

fn within(limit: usize) -> impl Fn(&str, &str) -> Result<(), PipelineRefusal> {
    move |field: &str, value: &str| bound(field, limit, value.len())
}

/// A title holds at least one `char::is_alphanumeric()` character, no
/// `char::is_control()` character, no U+2028 LINE SEPARATOR or U+2029
/// PARAGRAPH SEPARATOR (both `Zl`/`Zp`, so `is_control()` does not reach
/// them, and a one-line field should carry no hard line break), and no bidi
/// control character: `U+061C`, `U+200E`, `U+200F`, `U+202A`-`U+202E`,
/// `U+2066`-`U+2069`. No denylist of individual zero-width or format
/// characters is maintained here; a character not covered by one of these
/// tests is allowed, so ZWJ and ZWNJ still work.
fn title_text(field: &str, value: &str) -> Result<(), PipelineRefusal> {
    let alphanumeric = value.chars().any(|character| character.is_alphanumeric());
    let control = value
        .chars()
        .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'));
    let bidi = value.chars().any(|character| {
        matches!(
            character,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
    });
    if !alphanumeric || control || bidi {
        return Err(format_error(field, value));
    }
    within(200)(field, value)
}

bounded_text! {
    /// Text of at most 200 UTF-8 bytes.
    Short = 200, |field, value| within(200)(field, value);
    /// A title: 1 to 200 UTF-8 bytes, enforced by admission; JSON Schema
    /// length counts characters. Holds at least one alphanumeric character,
    /// no control character, and no bidi control character (`U+061C`,
    /// `U+200E`, `U+200F`, `U+202A`-`U+202E`, `U+2066`-`U+2069`).
    #[schemars(extend("minLength" = 1))]
    Title = 200, title_text;
    /// Text of at most 1,000 UTF-8 bytes.
    Line = 1000, |field, value| within(1000)(field, value);
    /// Text of at most 4,000 UTF-8 bytes; longer narrative is cited by `DocRef`.
    Para = 4000, |field, value| within(4000)(field, value);
    /// A key label: `[A-Za-z0-9_-]{1,64}`.
    Label = 64, label_text;
    /// A campaign slug or key segment: label parts joined by `.`.
    Slug = 64, dotted_text;
    /// A git commit id: 7-40 lowercase hex characters.
    Sha = 40, |field, value| hex(field, value, 7..=40);
    /// A full git commit id: exactly 40 lowercase hex characters.
    FullSha = 40, |field, value| hex(field, value, 40..=40);
    /// A SHA-256 digest: exactly 64 lowercase hex characters.
    Sha256Hex = 64, |field, value| hex(field, value, 64..=64);
}

impl Sha {
    /// The type's identity rule: two spellings name one commit when one is a
    /// prefix of the other, so a 7-character abbreviation and the full id of
    /// the same commit are equal here though not `==`. Both are already 7-40
    /// lowercase hex. Two distinct commits that share a 7-character prefix are
    /// git's ambiguity, not the mind's.
    pub fn names_same_commit(&self, other: &Sha) -> bool {
        self.0.starts_with(&other.0) || other.0.starts_with(&self.0)
    }
}

impl Slug {
    /// The public door onto the slug grammar, for a caller outside this crate
    /// that holds a bare `Slug` and no way to reach `Bounded`, which is
    /// crate-private. On the pattern of `PipelineRef::validate_ref`: it
    /// delegates so the grammar keeps one owner, and it applies exactly the
    /// `dotted_text` check that every `Slug` field in this crate is held to,
    /// so a caller that validates a declared name this way and a document
    /// field of the same type refuse the same inputs. Named `validate_slug`
    /// rather than a bare `validate`, because an inherent `validate` would
    /// shadow `Bounded`'s for every in-crate caller holding a `Slug`.
    pub fn validate_slug(&self) -> Result<(), PipelineRefusal> {
        Bounded::validate(self, "slug")
    }
}

/// A GitHub repository as `Org/Repo`, at most 140 UTF-8 bytes (39 + 1 +
/// 100). The owner is 1 to 39 `[A-Za-z0-9-]` bytes with no leading or
/// trailing hyphen; the repo is 1 to 100 `[A-Za-z0-9._-]` bytes, is never
/// `.` or `..`, and does not end in `.git`, ASCII-case-insensitively.
/// Identity is case-insensitive: `GameCult/Epiphany` and `gamecult/epiphany`
/// are the same repository.
//
// The raw string is display-only. `OrgRepo` is not generated by
// `bounded_text!`: that macro derives `PartialEq`, `Eq`, `Hash`, `PartialOrd`
// and `Ord` on the raw string, with no per-member override, so a type built
// from it compares and keys on spelling. `OrgRepo` instead carries
// hand-written impls of those five traits, every one of them over
// `identity()` (the ASCII-lowercased value), so `==`, a `BTreeMap` key and a
// `HashMap` key are all case-insensitive by construction: `GameCult/Epiphany`
// and `gamecult/epiphany` compare equal, hash equal, order together and
// collapse to one map entry, and a caller cannot reach the case-sensitive
// behaviour by accident. `Hash` and `Eq` agree because both are defined over
// the same `identity()` string. Every other bounded type keeps the macro's
// derives untouched. This reasoning is a source comment, not a doc comment,
// so it does not enter the published schema description (RS-L closing fix 2):
// a consumer needs the grammar, the bound and the case-insensitivity, not the
// Rust-macro justification for how that got built.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("maxLength" = 140))]
pub struct OrgRepo(pub String);

impl Bounded for OrgRepo {
    fn validate(&self, field: &str) -> Result<(), PipelineRefusal> {
        org_repo_text(field, &self.0)
    }
}

impl From<&str> for OrgRepo {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl PartialEq for OrgRepo {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for OrgRepo {}

impl std::hash::Hash for OrgRepo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.identity().hash(state);
    }
}

impl PartialOrd for OrgRepo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrgRepo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.identity().cmp(&other.identity())
    }
}

impl OrgRepo {
    /// The public door onto the `org_repo` grammar, for a caller outside this
    /// crate that holds a bare `OrgRepo` (the organ's `repo` alias, RS-3) and
    /// no way to reach `Bounded`. On the pattern of `Slug::validate_slug`: it
    /// delegates to `org_repo_text` through `Bounded::validate`, so the
    /// grammar keeps one owner and a declared name checked this way refuses
    /// exactly what a document's own `OrgRepo` field refuses.
    pub fn validate_org_repo(&self) -> Result<(), PipelineRefusal> {
        Bounded::validate(self, "org_repo")
    }

    /// The one canonical key for an `OrgRepo`: the value, ASCII-lowercased.
    /// Every comparison, hash, ordering and lookup keyed by repository
    /// identity goes through this — `PartialEq`, `Hash` and `Ord` above are
    /// all defined in terms of it — so `GameCult/Epiphany` and
    /// `gamecult/epiphany` are the same repository everywhere, not only where
    /// a caller remembers to ask.
    pub fn identity(&self) -> String {
        self.0.to_ascii_lowercase()
    }
}

impl Label {
    /// The public door onto the label grammar, for a caller outside this
    /// crate that holds a bare `Label` (the organ's `cut` alias, RS-3) and no
    /// way to reach `Bounded`. On the pattern of `Slug::validate_slug`: it
    /// delegates to `label_text` through `Bounded::validate`, so the grammar
    /// keeps one owner and a declared name checked this way refuses exactly
    /// what a document's own `Label` field refuses.
    pub fn validate_label(&self) -> Result<(), PipelineRefusal> {
        Bounded::validate(self, "label")
    }
}

/// A calendar date, `YYYY-MM-DD`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("maxLength" = 10))]
pub struct Date(pub String);

impl Bounded for Date {
    fn validate(&self, field: &str) -> Result<(), PipelineRefusal> {
        match chrono::NaiveDate::parse_from_str(&self.0, "%Y-%m-%d") {
            Ok(_) if self.0.len() == 10 => Ok(()),
            _ => Err(format_error(field, &self.0)),
        }
    }
}

macro_rules! bounded_field {
    ($value:expr, $field:expr) => {
        Bounded::validate(&$value, $field)?
    };
    ($value:expr, $field:expr, $max:literal) => {
        list($field, &$value, $max)?
    };
}

/// The single field list: one invocation emits the value struct and its
/// `Bounded` impl, so a field can never be left out of validation.
macro_rules! value_types {
    ($($(#[$attr:meta])* pub struct $name:ident { $($field:ident: $ty:ty $([$max:literal])?),* $(,)? } $(=> $extra:path)?)*) => {$(
        $(#[$attr])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        pub struct $name {
            $(
                $(#[schemars(extend("maxItems" = $max))])?
                pub $field: $ty
            ),*
        }

        impl Bounded for $name {
            fn validate(&self, at: &str) -> Result<(), PipelineRefusal> {
                $(bounded_field!(self.$field, &format!("{at}.{}", stringify!($field)) $(, $max)?);)*
                $($extra(self, at)?;)?
                Ok(())
            }
        }
    )*};
}

macro_rules! unit_enums {
    ($($name:ident { $($variant:ident),* $(,)? })*) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        pub enum $name { $($variant),* }
        impl Bounded for $name {
            fn validate(&self, _field: &str) -> Result<(), PipelineRefusal> {
                Ok(())
            }
        }
    )*};
}

/// RS-L closing fix 4: `campaign.repos` refuses two entries naming the same
/// repository by identity, not by raw spelling, so `GameCult/Epiphany` and
/// `gamecult/epiphany` in one list refuse exactly as one entry twice would.
/// This is a cross-element invariant on `PipelineCampaign` alone, not a
/// per-type format rule any other `value_types!` struct shares, so it is
/// wired in as that struct's one `=> extra` hook rather than folded into
/// `Bounded for OrgRepo` or the list bound in `bounded_field!`.
fn campaign_repos_are_distinct(campaign: &PipelineCampaign, at: &str) -> Result<(), PipelineRefusal> {
    let mut seen = std::collections::HashSet::new();
    for repo in &campaign.repos {
        if !seen.insert(repo.identity()) {
            return Err(PipelineRefusal::DuplicateRepo {
                field: format!("{at}.repos"),
                repo: repo.0.clone(),
            });
        }
    }
    Ok(())
}

unit_enums! {
    EvidenceKind { Command, Test, Mutation, Probe, SourceRead, Capture }
    ClaimOutcome { Holds, Falsified, Unproven }
    FindingConfidence { Confirmed, Plausible }
    FindingSeverity { Blocker, High, Medium, Low }
    RulingAuthority { Operator, Standing, Defaulted }
    FindingOrigin { Introduced, PreExisting }
}

value_types! {
    pub struct CodeLocation { path: Short, line: u32, end_line: Option<u32> }
    pub struct CommitRange { base: Sha, head: Sha }
    pub struct Evidence { kind: EvidenceKind, locator: Line, result: Line }
    /// A citation of a document in another repo's pipeline store (D6).
    pub struct ForeignRef { repo: OrgRepo, commit: FullSha, kind: PipelineKind, id: Short, payload_sha256: Sha256Hex }
    pub struct DocRef { path: Short, start_line: u32, end_line: u32, commit: Sha }
    pub struct TargetInvariant { label: Label, statement: Line }
    pub struct QuestionOption { label: Label, text: Line }
    pub struct CutDelete { path: Short, lines: u32, note: Line }
    pub struct FileChange { location: CodeLocation, change: Line }
    pub struct AuthorityMap {
        owner: Line, inputs: Vec<Line>[16], outputs: Vec<Line>[16], derived_state: Vec<Line>[16],
        forbidden_writers: Vec<Line>[16], shared_paths: Vec<Line>[16], deletion_line: Line,
    }
    pub struct VerificationTest { name: Short, pins: Line }
    pub struct NegativeCheck { pattern: Short, scope: Line }
    pub struct CutVerification {
        builds: Vec<Line>[64], tests: Vec<VerificationTest>[64],
        negative: Vec<NegativeCheck>[64], operator: Vec<Line>[64],
    }
    pub struct ReportCommit { sha: Sha, subject: Line, builds: bool }
    /// One mutation a report ran: a key-safe label a verdict claim can name,
    /// the exact edit, and the tree it was applied to.
    pub struct MutationRecord { label: Label, rule: Line, location: CodeLocation, before: Line, after: Line, commit: Sha, failed_as_expected: bool }
    pub struct Deviation { what: Line, why: Line }
    // D1 tables no maximum for these lists, so they take the shared list
    // default of 64 rather than a number invented for this field alone.
    pub struct StructuralDelta {
        lines_added: u32, lines_removed: u32,
        dependencies_added: Vec<Short>[64], dependencies_removed: Vec<Short>[64],
        formats_added: Vec<Short>[64], formats_removed: Vec<Short>[64],
        targets_added: Vec<Short>[64], targets_removed: Vec<Short>[64],
    }
    /// A name the cut landed, and where it lives.
    pub struct LandedName { name: Short, path: Short }
    /// A promise a report makes about what it landed. Not a document and not
    /// resolvable: it is identified by its report's id and its label, as a
    /// `TargetInvariant` is by its target's. Soul measures every one (ruling
    /// A), and the rule that every one is measured is admission's.
    pub struct Promise { label: Label, text: Line }
    /// The promise this claim measured, and the report's mutations it ran. An
    /// `Unproven` claim with a promise is one Soul could not reach.
    pub struct VerdictClaim {
        claim: Line, outcome: ClaimOutcome, evidence: Vec<Evidence>[8], findings: Vec<Short>[16],
        promise: Option<Label>, mutations: Vec<Label>[8],
    }

    pub struct PipelineCampaign { slug: Slug, title: Title, repos: Vec<OrgRepo>[8], working_branch: Short, target_doc: DocRef } => campaign_repos_are_distinct
    pub struct PipelineTarget {
        campaign: Slug, revision: u32, invariants: Vec<TargetInvariant>[32], not_in_scope: Vec<Line>[32],
        canonical_implementations: Vec<Line>[16], doc: DocRef,
    }
    pub struct PipelineQuestion {
        campaign: Slug, label: Label, title: Title, question: Para, options: Vec<QuestionOption>[8], recommended: Label,
        depends: Vec<Line>[8], raised_in: Option<PipelineRef>, asked_on: Date,
    }
    pub struct PipelineRuling {
        campaign: Slug, label: Label, title: Title, answers: Option<Short>, choice: Option<Label>, ruling: Para,
        operator_quote: Option<Para>, ruled_on: Date, precedents: Vec<ForeignRef>[8], authority: RulingAuthority,
    }
    pub struct PipelineCutSpec {
        campaign: Slug, cut: Label, revision: u32, title: Title, repo: OrgRepo, branch: Short, base: Sha,
        depends_on: Vec<Short>[8], first: Vec<Line>[16], deletes: Vec<CutDelete>[64], keeps_moves: Vec<Line>[64],
        adds: Vec<Line>[64], file_changes: Vec<FileChange>[256], authority_map: Option<AuthorityMap>,
        verification: CutVerification, estimate: StructuralDelta,
        rulings: Vec<Short>[32], questions: Vec<Short>[16],
    }
    pub struct PipelineCutReport {
        campaign: Slug, cut_spec: Short, attempt: u32, repo: OrgRepo, branch: Short,
        commits: Vec<ReportCommit>[128], range: CommitRange, verification: Vec<Evidence>[64],
        mutations: Vec<MutationRecord>[64], deviations: Vec<Deviation>[32], forks: Vec<Short>[8],
        structural_delta: StructuralDelta, landed_names: Vec<LandedName>[128], undone: Vec<Line>[32],
        promises: Vec<Promise>[64],
    }
    pub struct PipelineVerdict { campaign: Slug, cut_report: Short, pass: u32, range: CommitRange, claims: Vec<VerdictClaim>[64] }
    pub struct PipelineFinding {
        campaign: Slug, verdict: Short, label: Label, range: CommitRange, confidence: FindingConfidence,
        severity: FindingSeverity, claim: Line, invariants: Vec<Label>[8], locations: Vec<CodeLocation>[16],
        failure_scenario: Para, evidence: Vec<Evidence>[16], precedents: Vec<ForeignRef>[8], origin: FindingOrigin,
    }
    pub struct PipelineFollowUp {
        campaign: Slug, label: Label, source: PipelineRef, repo: OrgRepo, locations: Vec<CodeLocation>[16],
        item: Line, why_it_can_wait: Line, owner: Short,
    }
    /// The record of how a subject was closed and by what. `sequence` is per
    /// subject and set by the writer, `1` for the first, and it is the last
    /// part of the key, so a subject's resolutions share a prefix and a
    /// withdrawn one is kept under its subject rather than overwritten.
    /// Whether a sequence is the previous plus one, and whether an earlier
    /// resolution still stands, are admission's rules: this crate refuses
    /// neither `0` nor a gap, exactly as it refuses neither `revision: 0` nor
    /// a `revision` with no predecessor.
    pub struct PipelineResolution { subject: PipelineRef, sequence: u32, outcome: ResolutionOutcome, rationale: Para, resolved_on: Date }

    /// A mind's identity document. A store is canonical to exactly one
    /// instance, and this says which; identity lives in the state, not in a
    /// path.
    pub struct PipelineInstance { instance: Slug, display_name: Short, created_at: Date, host: Short }
    /// Stewardship over a repo, as an assignment recorded in a mind. One
    /// instance may steward several repos, so the repo is part of the key.
    /// `sequence` is per `(instance, repo)`, set by the writer, `1` for the
    /// first, so a repo transferred away and back is two records under one
    /// prefix rather than one document overwriting the other. `assigned_on`
    /// is a field, not a key part. As on a resolution, whether the sequence
    /// is the previous plus one, and which assignment is in force, are
    /// admission's rules and not this crate's.
    pub struct PipelineStewardship { instance: Slug, repo: OrgRepo, sequence: u32, assigned_on: Date, note: Line }
    /// A reassignment of stewardship, recorded in both minds. `documents` names
    /// what travels with it.
    pub struct PipelineHandOff {
        from_instance: Slug, to_instance: Slug, repo: OrgRepo, documents: Vec<Short>[256],
        reason: Para, handed_on: Date,
    }
}

/// A reference to another document. The id is parsed as a full pipeline id of
/// the declared `kind`, so the kind and the id's kind segment cannot disagree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineRef {
    pub kind: PipelineKind,
    pub id: Short,
}

impl Bounded for PipelineRef {
    fn validate(&self, at: &str) -> Result<(), PipelineRefusal> {
        self.id.validate(&format!("{at}.id"))?;
        pipeline_id(&format!("{at}.id"), &self.id.0, self.kind).map(|_| ())
    }
}

impl PipelineRef {
    /// The grammar's door for a reader holding a bare reference. A read side
    /// outside this crate is handed a `kind` and an `id` that may disagree, and
    /// `Bounded` is crate-private, so without this there is no way to ask; a
    /// reader with no way to ask answers empty, which reads as "no such
    /// document" rather than "that is not a reference". It delegates, so the
    /// grammar keeps one owner: this is exactly what a document's own
    /// `PipelineRef` field is held to, at the field name `ref`. It is kind-deep,
    /// like every other read of an id: whether the named document exists, and
    /// whether the local's shape suits the kind, are admission's rules.
    ///
    /// A method rather than a free function because it concerns one public
    /// type, as `prepare` and `decode` do; the two free doors act on a cache or
    /// an envelope this crate does not own. `validate_ref` rather than a bare
    /// `validate`, because an inherent `validate` would shadow `Bounded`'s for
    /// every in-crate caller holding a `PipelineRef`.
    pub fn validate_ref(&self) -> Result<(), PipelineRefusal> {
        self.validate("ref")
    }
}

/// How a subject was resolved. Every referent is a parsed `PipelineRef`, so a
/// resolution names its records by ids of the kinds they declare, validated
/// where every other referent is; a `Fixed` commit is a `Sha` whose referent
/// is outside the document set (ruling B). Whether a named document exists,
/// and how many may supersede one subject, are admission's rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ResolutionOutcome {
    /// Overturned, in whole or in part, by later records. The list is bounded
    /// here because the enum is outside `value_types!`, the only place a list
    /// maximum is emitted automatically.
    Superseded {
        #[schemars(extend("maxItems" = 8))]
        by: Vec<PipelineRef>,
    },
    Answered { by: PipelineRef },
    Fixed { commit: Sha, by: Option<PipelineRef> },
    Deferred { to: PipelineRef },
    Recorded { reason: Line },
    Withdrawn { reason: Line },
}

impl Bounded for ResolutionOutcome {
    fn validate(&self, field: &str) -> Result<(), PipelineRefusal> {
        match self {
            Self::Superseded { by } => list(&format!("{field}.by"), by, 8),
            Self::Answered { by } => by.validate(&format!("{field}.by")),
            Self::Fixed { commit, by } => {
                commit.validate(&format!("{field}.commit"))?;
                by.validate(&format!("{field}.by"))
            }
            Self::Deferred { to } => to.validate(&format!("{field}.to")),
            Self::Recorded { reason } | Self::Withdrawn { reason } => reason.validate(&format!("{field}.reason")),
        }
    }
}

macro_rules! pipeline_kinds {
    ($($variant:ident($value:ident) => $document:ident, $name:literal, $type_id:tt, $schema:tt;)*) => {
        $(
            #[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
            #[cultcache(type = $type_id, schema = $schema)]
            pub(crate) struct $document {
                #[cultcache(key = 0)]
                pub(crate) value: $value,
            }
        )*

        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        pub enum PipelineKind { $($variant),* }

        impl Bounded for PipelineKind {
            fn validate(&self, _field: &str) -> Result<(), PipelineRefusal> {
                Ok(())
            }
        }

        /// The envelope enum, adjacently tagged on the kind name: a document
        /// on a wire is `{ kind, value }`, and `kind` is exactly what
        /// `PipelineKind::name()` returns, so a reader dispatches on the kind
        /// segment it already knows from the key and needs no second registry
        /// mapping variant spellings to kinds. Adjacent rather than internal
        /// because a value is a named map of its own and an internal tag would
        /// have to be merged into it; the tag stays beside the value instead.
        /// This is not a published schema: `schemas/cultnet` publishes the
        /// thirteen per-kind value documents, and the envelope is the wire's
        /// shape, not a document's.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(tag = "kind", content = "value", rename_all = "snake_case")]
        pub enum PipelineDocument { $($variant($value)),* }

        impl PipelineKind {
            pub const ALL: &'static [PipelineKind] = &[$(Self::$variant),*];

            /// The kind segment of a document key.
            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),* }
            }

            pub fn type_id(self) -> &'static str {
                match self { $(Self::$variant => <$document as DatabaseEntry>::TYPE),* }
            }

            #[cfg(test)]
            pub(crate) fn schema_name(self) -> &'static str {
                match self { $(Self::$variant => <$document as DatabaseEntry>::SCHEMA_NAME),* }
            }

            #[cfg(test)]
            pub(crate) fn derived_schema(self) -> schemars::Schema {
                match self { $(Self::$variant => schemars::schema_for!($value)),* }
            }
        }

        impl PipelineDocument {
            pub fn kind(&self) -> PipelineKind {
                match self { $(Self::$variant(_) => PipelineKind::$variant),* }
            }

            /// Field bounds and formats (D1), in UTF-8 bytes and list maximums.
            pub fn validate(&self) -> Result<(), PipelineRefusal> {
                match self { $(Self::$variant(value) => value.validate($name)),* }
            }

            /// Prepares the envelope to be stored: keyed by `pipeline_key`,
            /// payload `[value]` through `prepare_entry_named`.
            pub fn prepare(&self, cache: &CultCache) -> Result<CultCacheEnvelope> {
                let key = pipeline_key(self)?;
                Ok(match self {
                    $(Self::$variant(value) => {
                        cache.prepare_entry_named(key, &$document { value: value.clone() })?.0
                    })*
                })
            }

            /// Decodes a stored envelope, type-matched both ways: an envelope of
            /// any other type is `ForeignStore`, never the kind that happens to
            /// parse its payload. It validates neither bounds nor the key: a
            /// decoded document is a typed read, and
            /// `validate_pipeline_write_envelope` is the check before a write.
            pub fn decode(envelope: &CultCacheEnvelope) -> Result<Self, PipelineRefusal> {
                let invalid = |error: rmp_serde::decode::Error| format_error("payload", &error.to_string());
                $(if envelope.r#type == <$document as DatabaseEntry>::TYPE {
                    let document: $document = rmp_serde::from_slice(&envelope.payload).map_err(invalid)?;
                    return Ok(Self::$variant(document.value));
                })*
                Err(PipelineRefusal::ForeignStore { r#type: envelope.r#type.clone() })
            }
        }

        /// Registers every pipeline kind in a cache, and nothing else: the one
        /// door to the crate-private wrappers, so a caller registers exactly
        /// what this crate publishes.
        pub fn register_pipeline_document_types(cache: &mut CultCache) -> Result<()> {
            $(cache.register_entry_type::<$document>()?;)*
            Ok(())
        }
    };
}

/// A document that is not a pipeline document, for the decode refusal and the
/// registrar's count to be pinned against something real. It stands in for
/// any other document a store may hold beside pipeline documents, a commit
/// receipt say, which is still not a document this library may decode. The
/// stand-in carries a receipt-shaped type id and is registered only by the
/// tests' own cache, never by the live registrar.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "epiphany.mind_commit_receipt.v1", schema = "ForeignDocument")]
pub(crate) struct ForeignDocument {
    #[cultcache(key = 0)]
    pub(crate) marker: Short,
}

pipeline_kinds! {
    Campaign(PipelineCampaign) => EpiphanyPipelineCampaignDocument, "campaign",
        "epiphany.pipeline.campaign.v2", "EpiphanyPipelineCampaignDocument";
    Target(PipelineTarget) => EpiphanyPipelineTargetDocument, "target",
        "epiphany.pipeline.target.v2", "EpiphanyPipelineTargetDocument";
    Question(PipelineQuestion) => EpiphanyPipelineQuestionDocument, "question",
        "epiphany.pipeline.question.v2", "EpiphanyPipelineQuestionDocument";
    Ruling(PipelineRuling) => EpiphanyPipelineRulingDocument, "ruling",
        "epiphany.pipeline.ruling.v2", "EpiphanyPipelineRulingDocument";
    CutSpec(PipelineCutSpec) => EpiphanyPipelineCutSpecDocument, "cut_spec",
        "epiphany.pipeline.cut_spec.v2", "EpiphanyPipelineCutSpecDocument";
    CutReport(PipelineCutReport) => EpiphanyPipelineCutReportDocument, "cut_report",
        "epiphany.pipeline.cut_report.v2", "EpiphanyPipelineCutReportDocument";
    Verdict(PipelineVerdict) => EpiphanyPipelineVerdictDocument, "verdict",
        "epiphany.pipeline.verdict.v2", "EpiphanyPipelineVerdictDocument";
    Finding(PipelineFinding) => EpiphanyPipelineFindingDocument, "finding",
        "epiphany.pipeline.finding.v2", "EpiphanyPipelineFindingDocument";
    FollowUp(PipelineFollowUp) => EpiphanyPipelineFollowUpDocument, "follow_up",
        "epiphany.pipeline.follow_up.v2", "EpiphanyPipelineFollowUpDocument";
    Resolution(PipelineResolution) => EpiphanyPipelineResolutionDocument, "resolution",
        "epiphany.pipeline.resolution.v2", "EpiphanyPipelineResolutionDocument";
    Instance(PipelineInstance) => EpiphanyPipelineInstanceDocument, "instance",
        "epiphany.pipeline.instance.v2", "EpiphanyPipelineInstanceDocument";
    Stewardship(PipelineStewardship) => EpiphanyPipelineStewardshipDocument, "stewardship",
        "epiphany.pipeline.stewardship.v2", "EpiphanyPipelineStewardshipDocument";
    HandOff(PipelineHandOff) => EpiphanyPipelineHandOffDocument, "hand_off",
        "epiphany.pipeline.hand_off.v2", "EpiphanyPipelineHandOffDocument";
}

/// Parses a full document id: the grammar read backwards. A key and an id are
/// the same string, so this accepts exactly what `pipeline_key` derives:
/// `<root>:<kind>:<local>`, three segments for every kind. The kind segment
/// must equal `kind`'s name, the root is a `Slug`, and the local is `Label`s
/// joined by `.` and bounded whole by `local_max(kind)`, so `..`, an empty part, spaces and trailing
/// junk are all refused. Returns the root and the local; nothing is inferred
/// from either, and no kind is read any other way.
fn pipeline_id<'a>(
    field: &str,
    id: &'a str,
    kind: PipelineKind,
) -> Result<(&'a str, &'a str), PipelineRefusal> {
    let mut segments = id.split(':');
    let (Some(root), Some(name), Some(local), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return Err(format_error(field, id));
    };
    if name != kind.name() {
        return Err(format_error(field, id));
    }
    dotted_text(field, root)?;
    dotted_within(field, local, local_max(kind))?;
    Ok((root, local))
}

/// The local segment of a parent id in `campaign`, of the expected kind.
fn parent_local<'a>(
    field: &str,
    id: &'a str,
    campaign: &str,
    kind: PipelineKind,
) -> Result<&'a str, PipelineRefusal> {
    let (owner, local) = pipeline_id(field, id, kind)?;
    if owner != campaign {
        return Err(format_error(field, id));
    }
    Ok(local)
}

/// The cut label inside a parent local `cut-<label>.<marker><N>`: exactly two
/// parts, the label a `Label` and `<N>` a non-empty run of digits.
fn parent_cut<'a>(field: &str, local: &'a str, marker: char) -> Result<&'a str, PipelineRefusal> {
    let invalid = || format_error(field, local);
    let mut parts = local.split('.');
    let (Some(head), Some(suffix), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid());
    };
    let cut = head.strip_prefix("cut-").ok_or_else(invalid)?;
    label_text(field, cut)?;
    let digits = suffix.strip_prefix(marker).ok_or_else(invalid)?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    Ok(cut)
}

/// A `Slug` or an `OrgRepo` entering a local, escaped to one label. `/` is not
/// a `Label` byte, so it cannot survive into a key, and neither may `.`: no
/// local part carries the separator, or the boundary before it would have two
/// readings. A dotted slug and a dotted repo name are both ordinary, so both
/// bytes are escaped rather than refused. The escape has to be injective or two
/// values claim one key: replacing `/` with `_` alone is not, since
/// `GameCult_Epiphany/thing` and `GameCult/Epiphany_thing` both give
/// `GameCult_Epiphany_thing`. So `_` is escaped as well, and every code is two
/// bytes starting with `_`: `_` becomes `__`, `/` becomes `_-`, and `.` becomes
/// `_d`. Every other byte is passed through and is never `_`, so a reader going
/// left to right takes each `_` together with the byte after it and never has a
/// choice to make; the encoding is therefore reversible, and distinct values
/// give distinct segments. The caller validates the value as its own type first
/// and checks the result against the local's rules, which is where an over-long
/// or otherwise unlabelled value is refused.
fn key_segment(value: &str) -> String {
    value.replace('_', "__").replace('/', "_-").replace('.', "_d")
}

/// The local of a root. A campaign's or an instance's identity is entirely its
/// root segment, so its local carries none; a reader never consults it.
const ROOT_LOCAL: &str = "self";

/// The bound on a composed local of every kind but a resolution, whole, in
/// UTF-8 bytes. Every stored subject key is within it.
const SUBJECT_LOCAL_MAX: usize = 64;

/// The longest kind name a resolution can name as its subject (`stewardship`),
/// checked against `PipelineKind::ALL` by a test.
const LONGEST_KIND_NAME: usize = "stewardship".len();

/// A `.n<sequence>` part at `u32::MAX`.
const SEQUENCE_PART_MAX: usize = ".n".len() + "4294967295".len();

/// The bound on a resolution's local: a depth-two chain over a maximal
/// subject, `resolution.<kind>.<subject local>.n<seq>.n<seq>`, at `u32::MAX`
/// sequences. Depth is capped at two by admission (Q19), and this bound admits
/// exactly that for every subject, so no subject is born unresolvable. Derived
/// from `SUBJECT_LOCAL_MAX` and the kind names, never written as a number.
const RESOLUTION_LOCAL_MAX: usize = "resolution.".len() + LONGEST_KIND_NAME + ".".len() + SUBJECT_LOCAL_MAX + 2 * SEQUENCE_PART_MAX;

// `<root>:<kind>:<local>` must fit a `Short` wherever an id is cited.
const _: () = assert!(64 + ":".len() + LONGEST_KIND_NAME + ":".len() + RESOLUTION_LOCAL_MAX <= 200);

/// The bound on a kind's composed local, chosen here and nowhere else.
fn local_max(kind: PipelineKind) -> usize {
    match kind {
        PipelineKind::Resolution => RESOLUTION_LOCAL_MAX,
        _ => SUBJECT_LOCAL_MAX,
    }
}

/// The schema epoch every pipeline store is written at, owned here with the
/// schemas it names. Evolution is additive and keeps it: a new named field
/// with a serde default, or a widened `PipelineKind`, since each reader ships
/// with the variants it knows and refuses an unknown kind on the kind, not on
/// the epoch. A breaking change bumps it, so that a store written at the old
/// one can be refused by whoever opens it; this crate owns no store and
/// refuses none.
pub const PIPELINE_SCHEMA_EPOCH: &str = "epiphany.pipeline.epoch.v2";

/// Composes and validates a local: every part is a `Label`, and the join is
/// bounded whole by `local_max(kind)`. Both rules live here because this is the only way a local is
/// built; `pipeline_key` has no other path to a key string. Parts are a slice,
/// not a builder, so an arm's arity is written at its call site.
fn local(field: &str, kind: PipelineKind, parts: &[&str]) -> Result<String, PipelineRefusal> {
    for part in parts {
        label_text(field, part)?;
    }
    let joined = parts.join(".");
    if joined.len() > local_max(kind) {
        return Err(format_error(field, &joined));
    }
    Ok(joined)
}

/// Derives a document's identity key (D1, "Keys: identity, not convenience").
/// Thirteen arms, one exit: every arm names its root and composes its local
/// through `local`, and the key is formatted here and nowhere else.
pub fn pipeline_key(document: &PipelineDocument) -> Result<String, PipelineRefusal> {
    use PipelineDocument as D;
    let document_kind = document.kind();
    let kind = document_kind.name();
    let key_field = format!("{kind}.key");
    let campaign_field = format!("{kind}.campaign");
    let (root_field, root, composed) = match document {
        D::Campaign(value) => ("campaign.slug", value.slug.0.as_str(), local(&key_field, document_kind, &[ROOT_LOCAL])?),
        D::Instance(value) => ("instance.instance", value.instance.0.as_str(), local(&key_field, document_kind, &[ROOT_LOCAL])?),
        // A resolution is keyed inside its subject's root, with the subject's
        // kind and local as its own local and its per-subject sequence last, so
        // the subject's resolutions and only they share the prefix
        // `<root>:resolution:<kind>.<local>.n`. A resolution's own key is an
        // ordinary id, so it composes as a subject like any other; each nesting
        // prepends `resolution.` (11 bytes) and appends `.n<s>` (3 bytes for
        // one digit). Depth is capped at two by admission (Q19), and
        // `RESOLUTION_LOCAL_MAX` admits exactly a depth-two chain over a
        // maximal subject, so every subject resolves and its resolution can be
        // withdrawn.
        D::Resolution(value) => {
            let field = "resolution.subject.id";
            let (subject_root, subject_local) = pipeline_id(field, &value.subject.id.0, value.subject.kind)?;
            let sequence = format!("n{}", value.sequence);
            let parts = std::iter::once(value.subject.kind.name()).chain(subject_local.split('.')).chain(std::iter::once(sequence.as_str())).collect::<Vec<_>>();
            (field, subject_root, local(&key_field, document_kind, &parts)?)
        }
        // Stewardship and hand-off hang off an instance rather than a campaign.
        // The root is the whole difference; the key shape is the same.
        D::Stewardship(value) => {
            org_repo_text("stewardship.repo", &value.repo.0)?;
            let sequence = format!("n{}", value.sequence);
            (
                "stewardship.instance",
                value.instance.0.as_str(),
                local(&key_field, document_kind, &[&key_segment(&value.repo.identity()), &sequence])?,
            )
        }
        D::HandOff(value) => {
            dotted_text("hand_off.to_instance", &value.to_instance.0)?;
            org_repo_text("hand_off.repo", &value.repo.0)?;
            value.handed_on.validate("hand_off.handed_on")?;
            (
                "hand_off.from_instance",
                value.from_instance.0.as_str(),
                local(
                    &key_field,
                    document_kind,
                    &[&key_segment(&value.to_instance.0), &key_segment(&value.repo.identity()), &value.handed_on.0],
                )?,
            )
        }
        D::Target(value) => (campaign_field.as_str(), value.campaign.0.as_str(), local(&key_field, document_kind, &[&format!("r{}", value.revision)])?),
        D::Question(value) => (campaign_field.as_str(), value.campaign.0.as_str(), local(&key_field, document_kind, &[&value.label.0])?),
        D::Ruling(value) => (campaign_field.as_str(), value.campaign.0.as_str(), local(&key_field, document_kind, &[&value.label.0])?),
        D::FollowUp(value) => (campaign_field.as_str(), value.campaign.0.as_str(), local(&key_field, document_kind, &[&value.label.0])?),
        D::CutSpec(value) => (
            campaign_field.as_str(),
            value.campaign.0.as_str(),
            local(&key_field, document_kind, &[&format!("cut-{}", value.cut.0), &format!("r{}", value.revision)])?,
        ),
        D::CutReport(value) => {
            let spec = parent_local("cut_report.cut_spec", &value.cut_spec.0, &value.campaign.0, PipelineKind::CutSpec)?;
            let cut = parent_cut("cut_report.cut_spec", spec, 'r')?;
            (
                campaign_field.as_str(),
                value.campaign.0.as_str(),
                local(&key_field, document_kind, &[&format!("cut-{cut}"), &format!("h{}", value.attempt)])?,
            )
        }
        D::Verdict(value) => {
            let report = parent_local("verdict.cut_report", &value.cut_report.0, &value.campaign.0, PipelineKind::CutReport)?;
            let cut = parent_cut("verdict.cut_report", report, 'h')?;
            (
                campaign_field.as_str(),
                value.campaign.0.as_str(),
                local(&key_field, document_kind, &[&format!("cut-{cut}"), &format!("s{}", value.pass)])?,
            )
        }
        D::Finding(value) => {
            let verdict = parent_local("finding.verdict", &value.verdict.0, &value.campaign.0, PipelineKind::Verdict)?;
            parent_cut("finding.verdict", verdict, 's')?;
            let parts = verdict.split('.').chain(std::iter::once(value.label.0.as_str())).collect::<Vec<_>>();
            (campaign_field.as_str(), value.campaign.0.as_str(), local(&key_field, document_kind, &parts)?)
        }
    };
    dotted_text(root_field, root)?;
    Ok(format!("{root}:{kind}:{composed}"))
}

/// Bounds, formats, then key recomputation, on the envelope that will be
/// stored, whoever prepared it. It is the whole of what this crate checks
/// before a write; per-kind and cross-document rules are not decided here and
/// belong to the admission path of the organ that will admit these documents,
/// which is expected to call this first rather than re-derive it.
pub fn validate_pipeline_write_envelope(envelope: &CultCacheEnvelope) -> Result<(), PipelineRefusal> {
    let document = PipelineDocument::decode(envelope)?;
    document.validate()?;
    let expected = pipeline_key(&document)?;
    if envelope.key != expected {
        return Err(PipelineRefusal::InvalidIdentity {
            kind: document.kind(),
            key: envelope.key.clone(),
            expected,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    const CAMPAIGN: &str = "eureka-state";
    const INSTANCE: &str = "yggdrasil";

    fn s(value: &str) -> Short {
        value.into()
    }

    fn t(value: &str) -> Title {
        value.into()
    }

    fn l(value: &str) -> Label {
        value.into()
    }

    fn slug(value: &str) -> Slug {
        value.into()
    }

    fn id(kind: &str, local: &str) -> Short {
        Short(format!("{CAMPAIGN}:{kind}:{local}"))
    }

    fn sha() -> Sha {
        Sha("5f98228d".into())
    }

    fn date() -> Date {
        Date("2026-09-15".into())
    }

    fn range() -> CommitRange {
        CommitRange { base: sha(), head: sha() }
    }

    fn location() -> CodeLocation {
        CodeLocation { path: s("crates/eureka-pipeline/src/lib.rs"), line: 1, end_line: Some(9) }
    }

    fn evidence() -> Evidence {
        Evidence { kind: EvidenceKind::Test, locator: "cargo test".into(), result: "ok".into() }
    }

    fn doc_ref() -> DocRef {
        DocRef { path: s("notes/eureka-pipeline-state-target.md"), start_line: 1, end_line: 9, commit: sha() }
    }

    /// One valid document of every kind, with its expected key.
    fn samples() -> Vec<(PipelineDocument, String)> {
        use PipelineDocument as D;
        let repo = || OrgRepo("GameCult/Epiphany".into());
        let branch = || s("codex/eureka-pipeline-state");
        vec![
            (D::Campaign(PipelineCampaign {
                slug: slug(CAMPAIGN), title: t("Eureka pipeline state"), repos: vec![repo()],
                working_branch: branch(), target_doc: doc_ref(),
            }), format!("{CAMPAIGN}:campaign:self")),
            (D::Target(PipelineTarget {
                campaign: slug(CAMPAIGN), revision: 2,
                invariants: vec![TargetInvariant { label: l("mind-admits"), statement: "Only admission writes.".into() }],
                not_in_scope: vec!["Eve browsing".into()], canonical_implementations: vec!["CultLib".into()], doc: doc_ref(),
            }), format!("{CAMPAIGN}:target:r2")),
            (D::Question(PipelineQuestion {
                campaign: slug(CAMPAIGN), label: l("Q1"), title: t("Who owns the state?"), question: "Who owns the state?".into(),
                options: vec![
                    QuestionOption { label: l("A"), text: "an instance".into() },
                    QuestionOption { label: l("B"), text: "a repo".into() },
                ],
                recommended: l("A"), depends: vec!["Cut 3a".into()],
                raised_in: Some(PipelineRef { kind: PipelineKind::CutSpec, id: id("cut_spec", "cut-3a.r1") }),
                asked_on: date(),
            }), format!("{CAMPAIGN}:question:Q1")),
            (D::Ruling(PipelineRuling {
                campaign: slug(CAMPAIGN), label: l("R8"), title: t("An instance owns its mind"), answers: Some(id("question", "Q1")), choice: Some(l("A")),
                ruling: "An instance owns its mind.".into(), operator_quote: Some("all recommendations, go ahead".into()),
                ruled_on: date(),
                precedents: vec![ForeignRef {
                    repo: OrgRepo("GameCult/Aetheria".into()), commit: FullSha("a".repeat(40)), kind: PipelineKind::Ruling,
                    id: s("cultcache:ruling:R1"), payload_sha256: Sha256Hex("b".repeat(64)),
                }],
                authority: RulingAuthority::Operator,
            }), format!("{CAMPAIGN}:ruling:R8")),
            (D::CutSpec(PipelineCutSpec {
                campaign: slug(CAMPAIGN), cut: l("3a"), revision: 1, title: t("Pipeline documents"), repo: repo(),
                branch: branch(), base: sha(), depends_on: vec![s("2")], first: vec!["Read the spec.".into()],
                deletes: vec![CutDelete { path: s("old.rs"), lines: 3, note: "dead".into() }],
                keeps_moves: vec!["commit owner".into()], adds: vec!["pipeline_documents.rs".into()],
                file_changes: vec![FileChange { location: location(), change: "add".into() }],
                authority_map: Some(AuthorityMap {
                    owner: "core".into(), inputs: vec!["typed documents".into()], outputs: vec!["envelopes".into()],
                    derived_state: vec!["derived keys".into()], forbidden_writers: vec!["MCP".into()],
                    shared_paths: vec!["admission".into()], deletion_line: "n/a".into(),
                }),
                verification: CutVerification {
                    builds: vec!["cargo check".into()],
                    tests: vec![VerificationTest { name: s("keys"), pins: "one derived key".into() }],
                    negative: vec![NegativeCheck { pattern: s("Vec<u8>"), scope: "documents".into() }],
                    operator: vec!["none".into()],
                },
                // Cut 3a's real numbers, and in this order: the tripwire test
                // reads them back, so a transposition is a failure, not a typo.
                estimate: StructuralDelta {
                    lines_added: 900, lines_removed: 0,
                    dependencies_added: vec![s("schemars")], dependencies_removed: vec![],
                    formats_added: vec![s("epiphany.pipeline.*.v1")], formats_removed: vec![],
                    targets_added: vec![], targets_removed: vec![],
                },
                rulings: vec![id("ruling", "R8")], questions: vec![id("question", "Q1")],
            }), format!("{CAMPAIGN}:cut_spec:cut-3a.r1")),
            (D::CutReport(PipelineCutReport {
                campaign: slug(CAMPAIGN), cut_spec: id("cut_spec", "cut-3a.r1"), attempt: 1, repo: repo(), branch: branch(),
                commits: vec![ReportCommit { sha: sha(), subject: "Add the pipeline documents".into(), builds: true }],
                range: range(), verification: vec![evidence()],
                mutations: vec![MutationRecord {
                    // `.into()` on both, not `l()` and `sha()`: the type-level
                    // mutations (M19, M20) widen these fields to `Short`, and
                    // a sample spelled with the narrow constructors would stop
                    // compiling instead of letting the forgery through.
                    label: "M1".into(), rule: "key derivation".into(), location: location(),
                    before: "parent_cut(field, spec, 'r')?".into(), after: "spec".into(), commit: "5f98228d".into(),
                    failed_as_expected: true,
                }],
                deviations: vec![Deviation { what: "names".into(), why: "glob exports".into() }],
                forks: vec![id("question", "Q1")],
                structural_delta: StructuralDelta {
                    lines_added: 900, lines_removed: 0, dependencies_added: vec![s("schemars")],
                    dependencies_removed: vec![], formats_added: vec![s("epiphany.pipeline.*.v1")],
                    formats_removed: vec![], targets_added: vec![], targets_removed: vec![],
                },
                landed_names: vec![LandedName { name: s("PipelineDocument"), path: s("crates/eureka-pipeline/src/lib.rs") }],
                undone: vec!["admission".into()],
                promises: vec![Promise { label: l("P1"), text: "One derived key per document.".into() }],
            }), format!("{CAMPAIGN}:cut_report:cut-3a.h1")),
            (D::Verdict(PipelineVerdict {
                campaign: slug(CAMPAIGN), cut_report: id("cut_report", "cut-3a.h1"), pass: 2, range: range(),
                claims: vec![VerdictClaim {
                    claim: "A composed key has one source.".into(), outcome: ClaimOutcome::Falsified,
                    evidence: vec![evidence()], findings: vec![id("finding", "cut-3a.s2.F4")],
                    promise: Some(l("P1")), mutations: vec![l("M1")],
                }],
            }), format!("{CAMPAIGN}:verdict:cut-3a.s2")),
            (D::Finding(PipelineFinding {
                campaign: slug(CAMPAIGN), verdict: id("verdict", "cut-3a.s2"), label: l("F4"), range: range(),
                confidence: FindingConfidence::Confirmed, severity: FindingSeverity::High,
                claim: "A dotted label composes two keys.".into(), invariants: vec![l("mind-admits")],
                locations: vec![location()], failure_scenario: "Two documents claim one key.".into(),
                evidence: vec![evidence()], precedents: vec![], origin: FindingOrigin::Introduced,
            }), format!("{CAMPAIGN}:finding:cut-3a.s2.F4")),
            (D::FollowUp(PipelineFollowUp {
                campaign: slug(CAMPAIGN), label: l("FU-4"),
                source: PipelineRef { kind: PipelineKind::Finding, id: id("finding", "cut-3a.s2.F4") },
                repo: repo(), locations: vec![location()], item: "Per-kind admission rules.".into(),
                why_it_can_wait: "The organ owns admission.".into(), owner: s("Hands"),
            }), format!("{CAMPAIGN}:follow_up:FU-4")),
            (D::Resolution(PipelineResolution {
                subject: PipelineRef { kind: PipelineKind::Question, id: id("question", "Q1") }, sequence: 1,
                outcome: ResolutionOutcome::Answered { by: PipelineRef { kind: PipelineKind::Ruling, id: id("ruling", "R8") } },
                rationale: "Ruled A.".into(), resolved_on: date(),
            }), format!("{CAMPAIGN}:resolution:question.Q1.n1")),
            // The mind's own three kinds. They are appended rather than
            // inserted because the helpers below index this list by position.
            (D::Instance(PipelineInstance {
                instance: slug(INSTANCE), display_name: s("Yggdrasil mind"), created_at: date(),
                host: s("yggdrasil"),
            }), format!("{INSTANCE}:instance:self")),
            (D::Stewardship(PipelineStewardship {
                instance: slug(INSTANCE), repo: repo(), sequence: 1, assigned_on: date(),
                note: "The Eureka campaign repo.".into(),
            }), format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1")),
            (D::HandOff(PipelineHandOff {
                from_instance: slug(INSTANCE), to_instance: slug("thought-cage"), repo: repo(),
                documents: vec![s(CAMPAIGN), id("ruling", "R8")],
                reason: "The workstation mind takes the campaign.".into(), handed_on: date(),
            }), format!("{INSTANCE}:hand_off:thought-cage.gamecult_-epiphany.{}", date().0)),
        ]
    }

    fn instance_sample() -> PipelineInstance {
        let PipelineDocument::Instance(instance) = samples().remove(10).0 else { unreachable!() };
        instance
    }

    fn stewardship_sample() -> PipelineStewardship {
        let PipelineDocument::Stewardship(stewardship) = samples().remove(11).0 else { unreachable!() };
        stewardship
    }

    fn hand_off_sample() -> PipelineHandOff {
        let PipelineDocument::HandOff(hand_off) = samples().remove(12).0 else { unreachable!() };
        hand_off
    }

    fn campaign_sample() -> PipelineDocument {
        samples().remove(0).0
    }

    fn question_sample() -> PipelineQuestion {
        let PipelineDocument::Question(question) = samples().remove(2).0 else { unreachable!() };
        question
    }

    fn ruling_sample() -> PipelineRuling {
        let PipelineDocument::Ruling(ruling) = samples().remove(3).0 else { unreachable!() };
        ruling
    }

    fn report_sample() -> PipelineCutReport {
        let PipelineDocument::CutReport(report) = samples().remove(5).0 else { unreachable!() };
        report
    }

    fn verdict_sample() -> PipelineVerdict {
        let PipelineDocument::Verdict(verdict) = samples().remove(6).0 else { unreachable!() };
        verdict
    }

    fn finding_sample() -> PipelineFinding {
        let PipelineDocument::Finding(finding) = samples().remove(7).0 else { unreachable!() };
        finding
    }

    fn resolution_sample() -> PipelineResolution {
        let PipelineDocument::Resolution(resolution) = samples().remove(9).0 else { unreachable!() };
        resolution
    }

    /// The golden corpus: every sample plus one document per encoding-relevant
    /// variant (every enum variant, every Option both ways), each under a
    /// unique label that names what it varies.
    fn golden_documents() -> Vec<(String, PipelineDocument)> {
        use PipelineDocument as D;
        let mut out: Vec<(String, PipelineDocument)> =
            samples().into_iter().map(|(d, _)| (format!("sample.{}", d.kind().name()), d)).collect();
        let by = |kind: PipelineKind, kind_name: &str, local: &str| PipelineRef { kind, id: id(kind_name, local) };
        let resolution = |outcome: ResolutionOutcome| {
            let mut r = resolution_sample();
            r.outcome = outcome;
            D::Resolution(r)
        };
        let ruling_ref = || by(PipelineKind::Ruling, "ruling", "R8");
        out.push(("resolution.superseded".into(), resolution(ResolutionOutcome::Superseded { by: vec![ruling_ref(), by(PipelineKind::Question, "question", "Q1")] })));
        out.push(("resolution.fixed-by-none".into(), resolution(ResolutionOutcome::Fixed { commit: sha(), by: None })));
        out.push(("resolution.fixed-by-some".into(), resolution(ResolutionOutcome::Fixed { commit: sha(), by: Some(by(PipelineKind::CutReport, "cut_report", "cut-3a.h1")) })));
        out.push(("resolution.deferred".into(), resolution(ResolutionOutcome::Deferred { to: by(PipelineKind::FollowUp, "follow_up", "FU-4") })));
        out.push(("resolution.recorded".into(), resolution(ResolutionOutcome::Recorded { reason: "Noted.".into() })));
        out.push(("resolution.withdrawn".into(), resolution(ResolutionOutcome::Withdrawn { reason: "Mistaken.".into() })));
        for (name, authority) in [("standing", RulingAuthority::Standing), ("defaulted", RulingAuthority::Defaulted)] {
            let mut r = ruling_sample();
            r.authority = authority;
            out.push((format!("ruling.authority-{name}"), D::Ruling(r)));
        }
        let mut r = ruling_sample();
        (r.answers, r.choice, r.operator_quote, r.precedents) = (None, None, None, vec![]);
        out.push(("ruling.options-none".into(), D::Ruling(r)));
        let mut q = question_sample();
        q.raised_in = None;
        out.push(("question.raised-in-none".into(), D::Question(q)));
        let PipelineDocument::CutSpec(mut spec) = samples().remove(4).0 else { unreachable!() };
        spec.authority_map = None;
        out.push(("cut_spec.authority-map-none".into(), D::CutSpec(spec)));
        for (name, outcome) in [("holds", ClaimOutcome::Holds), ("unproven", ClaimOutcome::Unproven)] {
            let mut v = verdict_sample();
            v.claims[0].outcome = outcome;
            v.claims[0].promise = None;
            out.push((format!("verdict.claim-{name}-promise-none"), D::Verdict(v)));
        }
        let mut f = finding_sample();
        (f.confidence, f.severity, f.origin) = (FindingConfidence::Plausible, FindingSeverity::Blocker, FindingOrigin::PreExisting);
        f.locations[0].end_line = None;
        f.evidence = [EvidenceKind::Command, EvidenceKind::Mutation, EvidenceKind::Probe, EvidenceKind::SourceRead, EvidenceKind::Capture]
            .into_iter().map(|kind| Evidence { kind, locator: "x".into(), result: "y".into() }).collect();
        out.push(("finding.plausible-blocker-preexisting-kinds".into(), D::Finding(f)));
        for (name, severity) in [("medium", FindingSeverity::Medium), ("low", FindingSeverity::Low)] {
            let mut f = finding_sample();
            f.severity = severity;
            out.push((format!("finding.severity-{name}"), D::Finding(f)));
        }
        out
    }

    /// One line per document: `label key type schema payload-hex`.
    fn golden_lines() -> Result<Vec<String>> {
        let cache = schema_cache()?;
        golden_documents()
            .into_iter()
            .map(|(label, document)| {
                document.validate()?;
                let envelope = document.prepare(&cache)?;
                let hex: String = envelope.payload.iter().map(|b| format!("{b:02x}")).collect();
                Ok(format!("{label} {} {} {} {hex}", envelope.key, envelope.r#type, document.kind().schema_name()))
            })
            .collect()
    }

    /// The wire is pinned to bytes. `golden/envelopes.txt` was derived once
    /// from this crate at its copy of Epiphany ef956865 (huginn 87b6455), which
    /// was proved byte-identical to the live mind store and to Epiphany; see
    /// its header. Payload bytes are load-bearing (`ForeignRef.payload_sha256`
    /// hashes them), and the cultcache type id and schema name are how a store
    /// resolves a persisted entry. A deliberate wire change re-derives the file
    /// with `golden_dump` below, in the same commit as a ruling that allows it.
    #[test]
    fn encoded_envelopes_match_the_committed_golden() -> Result<()> {
        let golden = include_str!("../golden/envelopes.txt");
        let expected: Vec<&str> = golden.lines().filter(|line| !line.starts_with('#') && !line.is_empty()).collect();
        let actual = golden_lines()?;
        assert_eq!(actual.len(), expected.len(), "the corpus and the golden file have the same documents");
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual, expected);
        }
        let kinds: BTreeSet<&str> = actual.iter().filter_map(|line| line.split(' ').nth(2)).collect();
        assert_eq!(kinds.len(), PipelineKind::ALL.len(), "every kind's type id is in the golden");
        Ok(())
    }

    /// Prints the golden body to the file named by `GOLDEN_OUT`. Not a check:
    /// run only for a deliberate wire change, then paste under the header.
    #[test]
    #[ignore = "re-derives golden/envelopes.txt; run only for a ruled wire change"]
    fn golden_dump() -> Result<()> {
        let out = std::env::var("GOLDEN_OUT").expect("GOLDEN_OUT names the output file");
        std::fs::write(out, golden_lines()?.join("
") + "
")?;
        Ok(())
    }

    /// The live registrar plus the foreign stand-in, which only the tests
    /// register.
    fn schema_cache() -> Result<CultCache> {
        let mut cache = CultCache::new();
        register_pipeline_document_types(&mut cache)?;
        cache.register_entry_type::<ForeignDocument>()?;
        Ok(cache)
    }

    #[test]
    fn every_pipeline_kind_round_trips_through_named_slot_zero() -> Result<()> {
        let cache = schema_cache()?;
        let samples = samples();
        let kinds = samples.iter().map(|(document, _)| document.kind()).collect::<BTreeSet<_>>();
        assert_eq!(kinds.len(), PipelineKind::ALL.len(), "every kind has a sample");
        for (document, _) in samples {
            document.validate()?;
            let envelope = document.prepare(&cache)?;
            assert_eq!(envelope.r#type, document.kind().type_id());
            assert_eq!(envelope.payload[0], 0x91, "{:?} payload is a one-element array", document.kind());
            assert!(
                matches!(envelope.payload[1], 0x80..=0x8f | 0xde | 0xdf),
                "{:?} slot 0 is a named map",
                document.kind()
            );
            assert_eq!(PipelineDocument::decode(&envelope)?, document);
            Ok::<_, anyhow::Error>(())?;
        }
        Ok(())
    }

    /// Soul F2: a decode is type-matched in both directions. The round-trip
    /// test above pins the positive match; this pins the refusal, so an
    /// envelope belonging to another kind can never be decoded as whichever
    /// pipeline kind happens to parse its payload. The commit receipt is the
    /// sharp case: a mind's store may legitimately hold one, and it is still
    /// not a document; `ForeignDocument` stands in for it here.
    #[test]
    fn decode_refuses_an_envelope_of_a_foreign_type() -> Result<()> {
        let cache = schema_cache()?;
        let foreign = <ForeignDocument as DatabaseEntry>::TYPE;
        assert!(!foreign.starts_with("epiphany.pipeline."), "{foreign} is a foreign type id");
        let mut envelope = campaign_sample().prepare(&cache)?;
        envelope.r#type = foreign.into();
        assert_eq!(
            PipelineDocument::decode(&envelope),
            Err(PipelineRefusal::ForeignStore { r#type: foreign.into() })
        );
        Ok(())
    }

    #[test]
    fn bounds_refuse_in_utf8_bytes() {
        let PipelineDocument::Campaign(mut campaign) = campaign_sample() else { unreachable!() };
        campaign.title = Title("é".repeat(100));
        assert_eq!(PipelineDocument::Campaign(campaign.clone()).validate(), Ok(()));
        campaign.title = Title(format!("{}a", "é".repeat(100)));
        assert_eq!(
            PipelineDocument::Campaign(campaign.clone()).validate(),
            Err(PipelineRefusal::FieldBound { field: "campaign.title".into(), limit: 200, actual: 201 })
        );
        campaign.title = t("ok");
        campaign.repos = vec![OrgRepo("GameCult/Epiphany".into()); 9];
        assert_eq!(
            PipelineDocument::Campaign(campaign).validate(),
            Err(PipelineRefusal::FieldBound { field: "campaign.repos".into(), limit: 8, actual: 9 })
        );
    }

    /// RS-L closing fix 4: `campaign.repos` refuses two entries naming one
    /// repository by identity, not only by raw spelling, so `GameCult/Epiphany`
    /// and `gamecult/epiphany` in one list refuse exactly as the same spelling
    /// twice would. Two genuinely distinct repos still validate.
    #[test]
    fn campaign_repos_refuse_duplicates_by_identity() {
        let PipelineDocument::Campaign(mut campaign) = campaign_sample() else { unreachable!() };
        campaign.repos = vec![OrgRepo("GameCult/Epiphany".into()), OrgRepo("gamecult/epiphany".into())];
        assert_eq!(
            PipelineDocument::Campaign(campaign.clone()).validate(),
            Err(PipelineRefusal::DuplicateRepo {
                field: "campaign.repos".into(),
                repo: "gamecult/epiphany".into(),
            })
        );

        campaign.repos = vec![OrgRepo("GameCult/Epiphany".into()); 2];
        assert_eq!(
            PipelineDocument::Campaign(campaign.clone()).validate(),
            Err(PipelineRefusal::DuplicateRepo {
                field: "campaign.repos".into(),
                repo: "GameCult/Epiphany".into(),
            })
        );

        campaign.repos = vec![OrgRepo("GameCult/Epiphany".into()), OrgRepo("GameCult/Aetheria".into())];
        assert_eq!(PipelineDocument::Campaign(campaign).validate(), Ok(()));
    }

    #[test]
    fn repo_fields_must_be_org_slash_repo() {
        let mut report = report_sample();
        report.repo = OrgRepo("Epiphany".into());
        assert_eq!(
            PipelineDocument::CutReport(report).validate(),
            Err(PipelineRefusal::InvalidFormat { field: "cut_report.repo".into(), value: "Epiphany".into() })
        );
        let PipelineDocument::CutSpec(mut spec) = samples().remove(4).0 else { unreachable!() };
        spec.repo = OrgRepo("GameCult/Epiphany/extra".into());
        assert!(matches!(
            PipelineDocument::CutSpec(spec).validate(),
            Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "cut_spec.repo"
        ));
        let PipelineDocument::FollowUp(mut follow_up) = samples().remove(8).0 else { unreachable!() };
        follow_up.repo = OrgRepo("/Epiphany".into());
        assert!(matches!(
            PipelineDocument::FollowUp(follow_up).validate(),
            Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "follow_up.repo"
        ));
    }

    #[test]
    fn keys_are_derived_and_mismatch_refuses() -> Result<()> {
        let cache = schema_cache()?;
        for (document, key) in samples() {
            assert_eq!(pipeline_key(&document), Ok(key.clone()));
            let mut envelope = document.prepare(&cache)?;
            envelope.key = format!("{key}-forged");
            assert_eq!(
                validate_pipeline_write_envelope(&envelope),
                Err(PipelineRefusal::InvalidIdentity { kind: document.kind(), key: envelope.key, expected: key })
            );
        }
        let mut resolution = resolution_sample();
        let answered = pipeline_key(&PipelineDocument::Resolution(resolution.clone()));
        resolution.outcome = ResolutionOutcome::Withdrawn { reason: "moot".into() };
        assert_eq!(
            pipeline_key(&PipelineDocument::Resolution(resolution)),
            answered,
            "a subject has one resolution key whatever the outcome"
        );
        Ok(())
    }

    /// F2, and Soul's second pass: a composed local segments one way only. The
    /// rule -- no local part carries the separator -- is pinned on the composer
    /// in `no_local_part_carries_the_separator`; the pairs below are the ones
    /// Soul measured through the documents, and each is a pair only because the
    /// rule holds.
    #[test]
    fn composed_keys_cannot_collide() {
        let key = |document: PipelineDocument| pipeline_key(&document);

        // Soul's hand-off pair. A dotted slug is ordinary on the receiver side
        // and a dotted repo name (the part after the mandatory `/`, GitHub's
        // grammar owns no dot on the owner side) is ordinary on the repo side,
        // so both are escaped: left unescaped these two both key to
        // `yggdrasil:hand_off:thought-cage.GameCult.Epiphany_-thing.2026-09-15`.
        let handed = |to: &str, repo: &str| {
            let mut hand_off = hand_off_sample();
            hand_off.to_instance = slug(to);
            hand_off.repo = OrgRepo(repo.into());
            key(PipelineDocument::HandOff(hand_off))
        };
        let dotted_receiver = handed("thought-cage.GameCult", "Epiphany/thing");
        let dotted_repo = handed("thought-cage", "GameCult/Epiphany.thing");
        assert_eq!(
            dotted_receiver,
            Ok(format!("{INSTANCE}:hand_off:thought-cage_dGameCult.epiphany_-thing.2026-09-15"))
        );
        assert_eq!(
            dotted_repo,
            Ok(format!("{INSTANCE}:hand_off:thought-cage.gamecult_-epiphany_dthing.2026-09-15"))
        );
        assert_ne!(dotted_receiver, dotted_repo, "a dotted repo and a dotted receiver cannot claim one key");

        let mut wide_label = finding_sample();
        wide_label.label = l("F1.G");
        assert!(
            matches!(key(PipelineDocument::Finding(wide_label)), Err(PipelineRefusal::InvalidFormat { .. })),
            "a finding label carrying a dot is refused, not silently composed"
        );
        let mut deep_verdict = finding_sample();
        deep_verdict.verdict = id("verdict", "cut-3a.s1.F1");
        deep_verdict.label = l("G");
        assert!(
            matches!(key(PipelineDocument::Finding(deep_verdict)), Err(PipelineRefusal::InvalidFormat { .. })),
            "a verdict local that is not cut-<label>.s<N> is refused"
        );
        let mut plain = finding_sample();
        plain.verdict = id("verdict", "cut-3a.s1");
        plain.label = l("F1");
        assert_eq!(key(PipelineDocument::Finding(plain)), Ok(format!("{CAMPAIGN}:finding:cut-3a.s1.F1")));

        let mut deep_spec = report_sample();
        deep_spec.cut_spec = id("cut_spec", "cut-3a.h1.r1");
        deep_spec.attempt = 2;
        assert!(
            matches!(key(PipelineDocument::CutReport(deep_spec)), Err(PipelineRefusal::InvalidFormat { .. })),
            "a cut label carrying a dot is refused, so cut-3a.h1.h2 has one source"
        );
        let mut attempt_two = report_sample();
        attempt_two.attempt = 2;
        assert_eq!(key(PipelineDocument::CutReport(attempt_two)), Ok(format!("{CAMPAIGN}:cut_report:cut-3a.h2")));
    }

    /// F3: every parent id is parsed strictly. These are the bad parents Soul
    /// found accepted, plus the wrong-kind and marker rules that mutations S1
    /// and S3 remove.
    #[test]
    fn parent_ids_are_parsed_strictly() {
        let refused = |parent: &str| {
            let mut report = report_sample();
            report.cut_spec = Short(parent.into());
            let key = pipeline_key(&PipelineDocument::CutReport(report));
            assert!(
                matches!(&key, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "cut_report.cut_spec"),
                "parent {parent:?} must be refused, got {key:?}"
            );
        };
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.r1:junk"));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.rX"));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-..r1"));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-.r1"));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.r1 "));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.r"));
        refused(&format!("{CAMPAIGN}:cut_report:cut-3a.h1"));
        // Right kind and a well-formed number, but the marker belongs to
        // another kind: the marker rule is pinned on its own.
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.h1"));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.s1"));
        // A well-formed number with no marker at all. Only the marker rule
        // refuses this one: the digits rule is satisfied either way.
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.1"));
        refused(&format!("{CAMPAIGN}:CUT_SPEC:cut-3a.r1"));
        refused(&format!("EUREKA-STATE:cut_spec:cut-3a.r1"));
        refused(&format!("{CAMPAIGN}:cut_spec:cut-3a.r1:"));
        refused(&format!("{CAMPAIGN}::cut-3a.r1"));
        refused("cut-3a.r1");
        let mut other_campaign = report_sample();
        other_campaign.campaign = slug("other-campaign");
        assert!(
            matches!(
                pipeline_key(&PipelineDocument::CutReport(other_campaign)),
                Err(PipelineRefusal::InvalidFormat { .. })
            ),
            "a parent in another campaign is refused"
        );
        let mut verdict = verdict_sample();
        verdict.cut_report = id("cut_report", "cut-3a.r1");
        assert!(
            matches!(pipeline_key(&PipelineDocument::Verdict(verdict)), Err(PipelineRefusal::InvalidFormat { .. })),
            "a verdict's parent must carry the report marker, not the spec marker"
        );
    }

    /// F4: a resolution's subject id is a full id of the declared kind.
    #[test]
    fn resolution_subject_is_a_full_id_of_its_kind() {
        let refused = |kind: PipelineKind, subject: &str| {
            let mut resolution = resolution_sample();
            resolution.subject = PipelineRef { kind, id: Short(subject.into()) };
            let document = PipelineDocument::Resolution(resolution);
            assert!(
                matches!(document.validate(), Err(PipelineRefusal::InvalidFormat { .. })),
                "subject {subject:?} must fail validation"
            );
            assert!(
                matches!(pipeline_key(&document), Err(PipelineRefusal::InvalidFormat { .. })),
                "subject {subject:?} must not compose a key"
            );
        };
        refused(PipelineKind::Question, "");
        refused(PipelineKind::Question, "not an id: spaces and : colons");
        refused(PipelineKind::Question, "eureka-state:question:1１");
        refused(PipelineKind::Question, &format!("{CAMPAIGN}:ruling:R8"));
        refused(PipelineKind::Ruling, &format!("{CAMPAIGN}:question:Q1"));
        refused(PipelineKind::Question, &format!("{CAMPAIGN}:question:.."));

        let mut valid = resolution_sample();
        valid.subject = PipelineRef { kind: PipelineKind::Ruling, id: id("ruling", "R8") };
        assert_eq!(
            pipeline_key(&PipelineDocument::Resolution(valid)),
            Ok(format!("{CAMPAIGN}:resolution:ruling.R8.n1"))
        );
    }

    /// Soul G2: a resolution is named by the key it has, and a `PipelineRef`
    /// of kind `Resolution` accepts exactly that id. `PipelineRef` takes any
    /// kind, so this is reachable; a follow-up sourced from a resolution is the
    /// live path. The grammar admits `<campaign>:resolution:R8` too: it is
    /// well-formed, and no resolution derives it, but whether a document with an
    /// id exists is admission's rule, not the grammar's, so the reader does not
    /// refuse it and this test does not ask it to.
    #[test]
    fn a_resolution_is_named_by_the_key_it_has() {
        let sourced = |id: &str| {
            let PipelineDocument::FollowUp(mut follow_up) = samples().remove(8).0 else { unreachable!() };
            follow_up.source = PipelineRef { kind: PipelineKind::Resolution, id: Short(id.into()) };
            PipelineDocument::FollowUp(follow_up).validate()
        };
        let key = pipeline_key(&PipelineDocument::Resolution(resolution_sample())).expect("the sample keys");
        assert_eq!(key, format!("{CAMPAIGN}:resolution:question.Q1.n1"));
        assert_eq!(sourced(&key), Ok(()), "a resolution is named by the key it has");
        assert_eq!(sourced(&format!("{CAMPAIGN}:resolution:R8")), Ok(()), "well-formed grammar is not the reader's to refuse");

        for refused in [
            // The prefix shape the writer used to emit. No resolution carries it.
            format!("resolution:{CAMPAIGN}:question:Q1"),
            format!("{CAMPAIGN}:resolution:{CAMPAIGN}:question:Q1"),
            // No local, a subject local with an empty part, trailing junk, and a
            // kind segment that is not `resolution`.
            format!("{CAMPAIGN}:resolution:"),
            format!("{CAMPAIGN}:resolution:question..Q1"),
            format!("{CAMPAIGN}:resolution:question.Q1:junk"),
            format!("{CAMPAIGN}:question:Q1"),
        ] {
            assert!(
                matches!(sourced(&refused), Err(PipelineRefusal::InvalidFormat { .. })),
                "{refused:?} is not an id any resolution has"
            );
        }

        // A root subject reads back too: a resolution of a campaign carries the
        // campaign's kind and its `self` local.
        let mut of_campaign = resolution_sample();
        of_campaign.subject = PipelineRef { kind: PipelineKind::Campaign, id: Short(format!("{CAMPAIGN}:campaign:self")) };
        let root_key = pipeline_key(&PipelineDocument::Resolution(of_campaign)).expect("a root subject keys");
        assert_eq!(root_key, format!("{CAMPAIGN}:resolution:campaign.self.n1"));
        assert_eq!(sourced(&root_key), Ok(()));
    }

    /// The three kinds a mind is keyed by. The samples above already round-trip
    /// every kind; this pins the shapes D2 gives these three specifically: an
    /// instance is a root keyed by its own slug, and the other two hang off an
    /// instance rather than a campaign, so neither borrows the campaign tail.
    #[test]
    fn instance_stewardship_and_hand_off_round_trip() -> Result<()> {
        let cache = schema_cache()?;
        let instance = PipelineDocument::Instance(instance_sample());
        assert_eq!(pipeline_key(&instance), Ok(format!("{INSTANCE}:instance:self")));
        for document in [
            instance,
            PipelineDocument::Stewardship(stewardship_sample()),
            PipelineDocument::HandOff(hand_off_sample()),
        ] {
            document.validate()?;
            let envelope = document.prepare(&cache)?;
            assert_eq!(PipelineDocument::decode(&envelope)?, document);
            let key = pipeline_key(&document)?;
            assert!(
                !key.starts_with(&format!("{CAMPAIGN}:")),
                "{:?} is keyed by its instance, not by a campaign: {key}",
                document.kind()
            );
            validate_pipeline_write_envelope(&envelope)?;
        }
        // A bounded list is still bounded: `documents` carries a maximum, as
        // every `Vec` field must.
        let mut wide = hand_off_sample();
        wide.documents = vec![s("x"); 257];
        assert_eq!(
            PipelineDocument::HandOff(wide).validate(),
            Err(PipelineRefusal::FieldBound { field: "hand_off.documents".into(), limit: 256, actual: 257 })
        );
        Ok(())
    }

    /// Soul F3: a key and an id are the same string, so every kind's key reads
    /// back as an id of that kind. The kinds a mind is keyed by were added to
    /// the key writer and left out of the id reader, and nothing noticed,
    /// because the key tests assert strings and never read one back: an
    /// instance keys to its slug and then fails to parse as an instance id, so
    /// no `PipelineRef` and no resolution could ever name one. Every kind now
    /// reads back with no kind excused, a resolution included, and a root kind
    /// read as the other root kind is refused: the reader checks the kind
    /// segment for the roots exactly as for every other kind.
    #[test]
    fn keys_read_back_as_ids_of_their_kind() -> Result<()> {
        for (document, expected) in samples() {
            let kind = document.kind();
            let key = pipeline_key(&document)?;
            assert_eq!(key, expected, "{kind:?} keys to its sample's key");
            assert_eq!(
                pipeline_id("read_back", &key, kind).map(|_| ()),
                Ok(()),
                "{kind:?} key {key} does not read back as an id of its kind"
            );
        }

        let campaign_key = pipeline_key(&campaign_sample())?;
        let instance_key = pipeline_key(&PipelineDocument::Instance(instance_sample()))?;
        assert_eq!(
            pipeline_id("read_back", &instance_key, PipelineKind::Instance),
            Ok((INSTANCE, ROOT_LOCAL)),
            "an instance key reads back as an instance"
        );
        assert_eq!(
            pipeline_id("read_back", &instance_key, PipelineKind::Campaign),
            Err(format_error("read_back", &instance_key)),
            "an instance key is not a campaign id"
        );
        assert_eq!(
            pipeline_id("read_back", &campaign_key, PipelineKind::Instance),
            Err(format_error("read_back", &campaign_key)),
            "a campaign key is not an instance id"
        );

        // Soul X1 and X11: the reader's root and local are each `dotted_text`
        // whole, so a trailing dot on either is refused by the reader itself,
        // not only by the writer that never composes one. A leading dot and
        // an empty part are the same rule on the local.
        for malformed in ["c.:target:x", "c:target:x.", "c:target:.x", "c:target:a..b"] {
            let read = pipeline_id("read_back", malformed, PipelineKind::Target);
            assert!(
                matches!(&read, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "read_back"),
                "{malformed:?} is not an id of any kind, got {read:?}"
            );
        }

        // The instance root's own key segment is validated, not merely bounded.
        // Nothing else stands between a `Slug` and a key: a space would compose
        // a store key carrying a byte no `Label` may hold.
        let mut spaced = instance_sample();
        spaced.instance = Slug("thought cage".into());
        assert_eq!(
            pipeline_key(&PipelineDocument::Instance(spaced)),
            Err(PipelineRefusal::InvalidFormat {
                field: "instance.instance".into(),
                value: "thought cage".into(),
            }),
            "an instance slug is validated where the key is composed"
        );
        Ok(())
    }

    /// The repo is one key segment, so its slash is escaped. Left unescaped it
    /// would both add a segment the reader cannot tell from a real one and put a
    /// byte in the key that no `Label` may carry; escaped to a bare `_` it would
    /// let two repos claim one key, which the colliding pair below pins.
    #[test]
    fn stewardship_key_escapes_the_repo_slash() {
        let stewardship = stewardship_sample();
        assert_eq!(stewardship.repo, OrgRepo("GameCult/Epiphany".into()));
        let key = pipeline_key(&PipelineDocument::Stewardship(stewardship.clone()));
        assert_eq!(key, Ok(format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1")));
        assert!(!key.unwrap().contains('/'), "no key segment carries a slash");

        // The escape is not a cosmetic substitution, and the pair that proves
        // it is a pair: GitHub's grammar keeps `.` and `_` out of the owner,
        // so both live in the repo half here, and under a naive scheme that
        // collapsed `/`, `.` and `_` all to a bare `_` both of these key to
        // `GameCult_Epiphany_thing`, and one of the two documents is lost.
        let keyed = |repo: &str| {
            let mut value = stewardship.clone();
            value.repo = OrgRepo(repo.into());
            pipeline_key(&PipelineDocument::Stewardship(value))
        };
        let dotted_repo = keyed("GameCult/Epiphany.thing");
        let underscored_repo = keyed("GameCult/Epiphany_thing");
        assert_eq!(
            dotted_repo,
            Ok(format!("{INSTANCE}:stewardship:gamecult_-epiphany_dthing.n1"))
        );
        assert_eq!(
            underscored_repo,
            Ok(format!("{INSTANCE}:stewardship:gamecult_-epiphany__thing.n1"))
        );
        assert_ne!(dotted_repo, underscored_repo, "two repos cannot claim one key");

        let mut no_org = stewardship.clone();
        no_org.repo = OrgRepo("Epiphany".into());
        assert_eq!(
            pipeline_key(&PipelineDocument::Stewardship(no_org)),
            Err(PipelineRefusal::InvalidFormat { field: "stewardship.repo".into(), value: "Epiphany".into() })
        );

        // S-1: key derivation goes through `identity()`, so a case variant of
        // the same repository is one store key, not two stewardships of what
        // is really one repo.
        let mut upper = stewardship.clone();
        upper.repo = OrgRepo("gamecult/epiphany".into());
        assert_eq!(
            pipeline_key(&PipelineDocument::Stewardship(upper)),
            pipeline_key(&PipelineDocument::Stewardship(stewardship.clone())),
            "GameCult/Epiphany and gamecult/epiphany must derive one store key"
        );

        // An escaped repo is still bound by the key's segment rules.
        let mut long = stewardship;
        long.repo = OrgRepo(format!("GameCult/{}", "a".repeat(60)));
        assert!(
            matches!(
                pipeline_key(&PipelineDocument::Stewardship(long)),
                Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "stewardship.key"
            ),
            "a repo segment longer than a key segment is refused"
        );
    }

    /// A hand-off is recorded in both minds, so both instances are in the key:
    /// the sender owns the first segment and the receiver the local. Changing
    /// either one moves the document.
    #[test]
    fn hand_off_names_both_instances() {
        let key = |hand_off: PipelineHandOff| pipeline_key(&PipelineDocument::HandOff(hand_off));
        let base = key(hand_off_sample()).expect("the sample keys");
        assert_eq!(base, format!("{INSTANCE}:hand_off:thought-cage.gamecult_-epiphany.2026-09-15"));

        let mut other_sender = hand_off_sample();
        other_sender.from_instance = slug("thought-cage");
        assert_ne!(key(other_sender), Ok(base.clone()), "the sender is in the key");

        let mut other_receiver = hand_off_sample();
        other_receiver.to_instance = slug("mimir");
        assert_ne!(key(other_receiver), Ok(base.clone()), "the receiver is in the key");

        // Two hand-offs of the same repo between the same pair on different
        // days are different documents.
        let mut later = hand_off_sample();
        later.handed_on = Date("2026-09-16".into());
        assert_ne!(key(later), Ok(base), "the date is in the key");

        // A dotted receiver is a `Slug` entering a local, so it is escaped to
        // one label rather than widening the local by a part.
        let mut dotted_receiver = hand_off_sample();
        dotted_receiver.to_instance = slug("thought-cage.GameCult");
        assert_eq!(
            key(dotted_receiver),
            Ok(format!("{INSTANCE}:hand_off:thought-cage_dGameCult.gamecult_-epiphany.2026-09-15"))
        );

        // The escape's `_` half is not only for repos: a receiver `a_db` and a
        // receiver `a.b` would both key to `a_db` if the slug's own `_` passed
        // through raw, so the slug is escaped as `a__db` and the two differ.
        let mut underscored_receiver = hand_off_sample();
        underscored_receiver.to_instance = slug("a_db");
        let mut dotted_twin = hand_off_sample();
        dotted_twin.to_instance = slug("a.b");
        let underscored_receiver = key(underscored_receiver).expect("an underscored receiver keys");
        assert!(underscored_receiver.contains("a__db"), "{underscored_receiver}: the slug's underscore is escaped");
        assert_ne!(key(dotted_twin), Ok(underscored_receiver), "`a_db` and `a.b` are two receivers");

        // Both instance slugs are validated as key segments, not merely bounded.
        let mut bad_receiver = hand_off_sample();
        bad_receiver.to_instance = Slug("thought cage".into());
        assert_eq!(
            key(bad_receiver),
            Err(PipelineRefusal::InvalidFormat {
                field: "hand_off.to_instance".into(),
                value: "thought cage".into(),
            })
        );
        let mut bad_date = hand_off_sample();
        bad_date.handed_on = Date("2026-9-15".into());
        assert!(
            matches!(key(bad_date), Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "hand_off.handed_on"),
            "a malformed date does not compose a key"
        );
    }

    /// A resolution whose subject is the given document, keyed.
    fn resolution_of(kind: PipelineKind, id: &str) -> Result<String, PipelineRefusal> {
        let mut resolution = resolution_sample();
        resolution.subject = PipelineRef { kind, id: Short(id.into()) };
        pipeline_key(&PipelineDocument::Resolution(resolution))
    }

    /// R1, the grammar's own test: every key is `<root>:<kind>:<local>`, three
    /// segments with no kind excused, the root a `Slug`, the kind the literal
    /// name, and every local part a `Label`. A root that would add a segment is
    /// refused where the key is composed: `pipeline_key` is `pub` and does not
    /// validate the document first, so this is reachable without one.
    #[test]
    fn every_key_has_exactly_three_segments() {
        let nested = resolution_of(PipelineKind::Resolution, &format!("{CAMPAIGN}:resolution:question.Q1.n1"))
            .expect("a resolution of a resolution keys");
        let keys = samples()
            .into_iter()
            .map(|(document, _)| (document.kind(), pipeline_key(&document).expect("every sample keys")))
            .chain([(PipelineKind::Resolution, nested)]);
        for (kind, key) in keys {
            let segments = key.split(':').collect::<Vec<_>>();
            assert_eq!(segments.len(), 3, "{kind:?} key {key} has three segments");
            assert_eq!(dotted_text("root", segments[0]), Ok(()), "{key}: the root is a slug");
            assert_eq!(segments[1], kind.name(), "{key}: the kind segment is the literal kind");
            assert!(PipelineKind::ALL.iter().any(|known| known.name() == segments[1]));
            for part in segments[2].split('.') {
                assert_eq!(label_text("local", part), Ok(()), "{key}: local part {part:?} is a label");
            }
        }

        // A root that would add a segment, one that would empty a part, and one
        // wider than a segment are all refused before a key is formatted. The
        // refusal names the part that failed, so an empty part reports `""`.
        let trailing_dot = format!("{CAMPAIGN}.");
        let leading_dot = format!(".{CAMPAIGN}");
        let over = "a".repeat(65);
        for root in ["a:b", trailing_dot.as_str(), leading_dot.as_str(), "a..b", over.as_str()] {
            let PipelineDocument::Target(mut target) = samples().remove(1).0 else { unreachable!() };
            target.campaign = Slug(root.into());
            let key = pipeline_key(&PipelineDocument::Target(target));
            assert!(
                matches!(&key, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "target.campaign"),
                "root {root:?} does not compose a key, got {key:?}"
            );
        }
    }

    /// Defect 1: the two roots are in distinct namespaces because the kind
    /// segment distinguishes them, the same mechanism that separates every
    /// other pair of kinds. Both key; only the pair sees a shared namespace.
    #[test]
    fn roots_of_different_kinds_do_not_share_a_key() {
        let PipelineDocument::Campaign(mut campaign) = campaign_sample() else { unreachable!() };
        campaign.slug = slug(INSTANCE);
        let campaign = pipeline_key(&PipelineDocument::Campaign(campaign));
        let instance = pipeline_key(&PipelineDocument::Instance(instance_sample()));
        assert_eq!(campaign, Ok(format!("{INSTANCE}:campaign:self")));
        assert_eq!(instance, Ok(format!("{INSTANCE}:instance:self")));
        assert_ne!(campaign, instance, "a campaign and an instance of one slug are two documents");
    }

    /// The root is a `Slug`, so a dotted root keys and reads back whole on
    /// both sides: the writer's root check and the reader's root check are
    /// each `dotted_text`, not `label_text`. A campaign `game.cult` and an
    /// instance `ygg.drasil` key, a document under the dotted campaign keys,
    /// and each reads back through `pipeline_id` recovering the root exactly.
    #[test]
    fn dotted_roots_key_and_read_back() {
        let PipelineDocument::Campaign(mut campaign) = campaign_sample() else { unreachable!() };
        campaign.slug = slug("game.cult");
        let campaign_key = pipeline_key(&PipelineDocument::Campaign(campaign)).expect("a dotted campaign keys");
        assert_eq!(campaign_key, "game.cult:campaign:self");
        assert_eq!(
            pipeline_id("read_back", &campaign_key, PipelineKind::Campaign),
            Ok(("game.cult", ROOT_LOCAL)),
            "the dotted campaign root reads back whole"
        );

        let mut instance = instance_sample();
        instance.instance = slug("ygg.drasil");
        let instance_key = pipeline_key(&PipelineDocument::Instance(instance)).expect("a dotted instance keys");
        assert_eq!(instance_key, "ygg.drasil:instance:self");
        assert_eq!(
            pipeline_id("read_back", &instance_key, PipelineKind::Instance),
            Ok(("ygg.drasil", ROOT_LOCAL)),
            "the dotted instance root reads back whole"
        );

        let PipelineDocument::Target(mut target) = samples().remove(1).0 else { unreachable!() };
        target.campaign = slug("game.cult");
        let target_key = pipeline_key(&PipelineDocument::Target(target)).expect("a target under a dotted campaign keys");
        assert_eq!(target_key, "game.cult:target:r2");
        assert_eq!(
            pipeline_id("read_back", &target_key, PipelineKind::Target),
            Ok(("game.cult", "r2")),
            "the dotted root and the local both read back"
        );

        let resolution = resolution_of(PipelineKind::Campaign, &campaign_key).expect("a resolution of a dotted campaign keys");
        assert_eq!(resolution, "game.cult:resolution:campaign.self.n1");
        assert_eq!(
            pipeline_id("read_back", &resolution, PipelineKind::Resolution),
            Ok(("game.cult", "campaign.self.n1")),
            "a resolution under a dotted root reads back"
        );
    }

    /// Cut 6c: a resolution's referent is a full id of the kind it declares,
    /// validated through `PipelineRef` like every other referent, so a
    /// well-formed id of the wrong kind is refused and the refusal names the
    /// entry. The positive case is Ghostlight's own: a ruling partly
    /// overturned by two later records, one of them a resolution named by the
    /// key it has, which only the 6b grammar made nameable. An id no
    /// resolution derives, `eureka-state:resolution:R8`, still validates:
    /// whether a document with an id exists is admission's rule.
    #[test]
    fn resolution_outcome_referents_are_parsed_ids_of_their_kind() {
        let resolved = |subject: PipelineRef, outcome: ResolutionOutcome| {
            let mut resolution = resolution_sample();
            resolution.subject = subject;
            resolution.outcome = outcome;
            PipelineDocument::Resolution(resolution).validate()
        };
        let refused = |field: &str, result: Result<(), PipelineRefusal>| {
            assert!(
                matches!(&result, Err(PipelineRefusal::InvalidFormat { field: at, .. }) if at == field),
                "expected InvalidFormat at {field}, got {result:?}"
            );
        };
        let ruling = |label: &str| PipelineRef { kind: PipelineKind::Ruling, id: id("ruling", label) };
        let question = |label: &str| PipelineRef { kind: PipelineKind::Question, id: id("question", label) };
        let inner = pipeline_key(&PipelineDocument::Resolution(resolution_sample())).expect("the sample keys");
        let resolution = PipelineRef { kind: PipelineKind::Resolution, id: Short(inner.clone()) };

        let superseded = |second: PipelineRef| ResolutionOutcome::Superseded { by: vec![ruling("R9"), second] };
        assert_eq!(resolved(ruling("R8"), superseded(resolution)), Ok(()));
        let forged = PipelineRef { kind: PipelineKind::Ruling, id: Short(inner) };
        refused("resolution.outcome.by[1].id", resolved(ruling("R8"), superseded(forged)));
        let underived = PipelineRef { kind: PipelineKind::Resolution, id: Short(format!("{CAMPAIGN}:resolution:R8")) };
        assert_eq!(resolved(ruling("R8"), superseded(underived)), Ok(()), "well-formed grammar is not the shape's to refuse");

        assert_eq!(resolved(question("Q1"), ResolutionOutcome::Answered { by: ruling("R8") }), Ok(()));
        let forged = PipelineRef { kind: PipelineKind::Question, id: id("ruling", "R8") };
        refused("resolution.outcome.by.id", resolved(question("Q1"), ResolutionOutcome::Answered { by: forged }));

        assert_eq!(resolved(question("Q1"), ResolutionOutcome::Deferred { to: question("Q2") }), Ok(()));
        let forged = PipelineRef { kind: PipelineKind::Ruling, id: id("question", "Q2") };
        refused("resolution.outcome.to.id", resolved(question("Q1"), ResolutionOutcome::Deferred { to: forged }));

        let finding = PipelineRef { kind: PipelineKind::Finding, id: id("finding", "cut-3a.s2.F4") };
        let fixed = |by: Option<PipelineRef>| ResolutionOutcome::Fixed { commit: sha(), by };
        assert_eq!(resolved(finding.clone(), fixed(None)), Ok(()));
        assert_eq!(resolved(finding.clone(), fixed(Some(ruling("R8")))), Ok(()));
        let forged = PipelineRef { kind: PipelineKind::Finding, id: id("ruling", "R8") };
        refused("resolution.outcome.by.id", resolved(finding, fixed(Some(forged))));

        // Soul F2: the validator's bound on a supersession is its own, not
        // only the schema's. Eight referents are the most a subject names;
        // nine are refused as a count before any item is read.
        let rulings = |count: usize| (1..=count).map(|n| ruling(&format!("R{n}"))).collect::<Vec<_>>();
        assert_eq!(resolved(ruling("R8"), ResolutionOutcome::Superseded { by: rulings(8) }), Ok(()));
        assert_eq!(
            resolved(ruling("R8"), ResolutionOutcome::Superseded { by: rulings(9) }),
            Err(PipelineRefusal::FieldBound { field: "resolution.outcome.by".into(), limit: 8, actual: 9 })
        );

        // Soul F3: the two reason-carrying outcomes are bounded like every
        // `Line`; the shared arm is pinned on both so neither can drop out.
        let wide = Line("a".repeat(1001));
        for outcome in [
            ResolutionOutcome::Recorded { reason: wide.clone() },
            ResolutionOutcome::Withdrawn { reason: wide },
        ] {
            let finding = PipelineRef { kind: PipelineKind::Finding, id: id("finding", "cut-3a.s2.F4") };
            assert_eq!(
                resolved(finding, outcome),
                Err(PipelineRefusal::FieldBound { field: "resolution.outcome.reason".into(), limit: 1000, actual: 1001 })
            );
        }
    }

    /// Ruling B: a fix names the tree where the finding stopped being true,
    /// as a `Sha`. Uppercase hex of a legal length is the forgery a length
    /// check passes and `hex` refuses.
    #[test]
    fn fixed_resolution_requires_a_commit_sha() {
        let fixed = |commit: &str, by: Option<PipelineRef>| {
            let mut resolution = resolution_sample();
            resolution.subject = PipelineRef { kind: PipelineKind::Finding, id: id("finding", "cut-3a.s2.F4") };
            resolution.outcome = ResolutionOutcome::Fixed { commit: Sha(commit.into()), by };
            PipelineDocument::Resolution(resolution).validate()
        };
        assert_eq!(fixed("5f98228d9c", None), Ok(()));
        for forged in ["5F98228D9C", "5f9822", "dirty-worktree", ""] {
            assert_eq!(
                fixed(forged, None),
                Err(PipelineRefusal::InvalidFormat { field: "resolution.outcome.commit".into(), value: forged.into() }),
                "{forged:?} is not a commit"
            );
        }
        // Soul F1: naming the record that fixed it does not excuse the commit.
        let ruling = PipelineRef { kind: PipelineKind::Ruling, id: id("ruling", "R8") };
        assert_eq!(
            fixed("dirty-worktree", Some(ruling)),
            Err(PipelineRefusal::InvalidFormat { field: "resolution.outcome.commit".into(), value: "dirty-worktree".into() }),
            "a fix with a named record still names a commit"
        );
    }

    /// A mutation record has a key-safe identity a verdict claim can name,
    /// and is pinned to a tree: its label is a `Label` and its commit a
    /// `Sha`, so a dotted label and free text where a sha belongs are refused.
    #[test]
    fn mutation_records_carry_a_dot_free_label_and_a_commit() {
        let recorded = |label: &str, commit: &str| {
            let mut report = report_sample();
            // Through `From<&str>`, which every text type has, so the
            // widened types under M19 and M20 still compile and the
            // forgeries reach validation.
            report.mutations[0].label = label.into();
            report.mutations[0].commit = commit.into();
            PipelineDocument::CutReport(report).validate()
        };
        assert_eq!(recorded("M1", "5f98228d"), Ok(()));
        assert_eq!(
            recorded("M1.a", "5f98228d"),
            Err(PipelineRefusal::InvalidFormat { field: "cut_report.mutations[0].label".into(), value: "M1.a".into() })
        );
        assert_eq!(
            recorded("M1", "dirty-worktree"),
            Err(PipelineRefusal::InvalidFormat {
                field: "cut_report.mutations[0].commit".into(),
                value: "dirty-worktree".into(),
            })
        );
    }

    /// Ruling A's shape half: a claim names the promise it measured and the
    /// mutations it ran, both as `Label`s, and names at most eight mutations.
    #[test]
    fn a_verdict_claim_names_the_promise_and_the_mutation_it_measured() {
        let sample = verdict_sample();
        assert_eq!(sample.claims[0].promise, Some(l("P1")));
        assert_eq!(sample.claims[0].mutations, vec![l("M1")]);
        assert_eq!(PipelineDocument::Verdict(sample).validate(), Ok(()));

        let claimed = |promise: Option<Label>, mutations: Vec<Label>| {
            let mut verdict = verdict_sample();
            verdict.claims[0].promise = promise;
            verdict.claims[0].mutations = mutations;
            PipelineDocument::Verdict(verdict).validate()
        };
        assert_eq!(claimed(None, vec![]), Ok(()), "a claim need not measure a promise");
        assert_eq!(
            claimed(Some(l("P1.a")), vec![]),
            Err(PipelineRefusal::InvalidFormat { field: "verdict.claims[0].promise".into(), value: "P1.a".into() })
        );
        assert_eq!(
            claimed(None, vec![l("M1.a")]),
            Err(PipelineRefusal::InvalidFormat { field: "verdict.claims[0].mutations[0]".into(), value: "M1.a".into() })
        );
        let labels = |count: usize| (1..=count).map(|n| l(&format!("M{n}"))).collect::<Vec<_>>();
        assert_eq!(claimed(None, labels(8)), Ok(()));
        assert_eq!(
            claimed(None, labels(9)),
            Err(PipelineRefusal::FieldBound { field: "verdict.claims[0].mutations".into(), limit: 8, actual: 9 })
        );
    }

    /// A tripwire on the one hazard of retyping the estimate: the delta's two
    /// `u32` fields are not interchangeable. The sample spec is Cut 3a's, a
    /// net addition of 900 lines, and the order is asserted.
    #[test]
    fn the_sample_cut_spec_estimates_a_net_addition() {
        let PipelineDocument::CutSpec(spec) = samples().remove(4).0 else { unreachable!() };
        assert_eq!(spec.estimate.lines_added, 900);
        assert_eq!(spec.estimate.lines_removed, 0);
        assert_eq!(spec.estimate, report_sample().structural_delta, "the estimate and the actual are one shape");
    }

    /// Defect 2: a resolution's key carries its subject's kind as a literal
    /// part of the local, so nothing infers it and two subjects of one local
    /// and different kinds resolve to two keys. Each reads back through the
    /// reader with the subject kind it was built from.
    #[test]
    fn a_resolution_names_its_subjects_kind() {
        let pairs = [
            (PipelineKind::Question, id("question", "Q1").0, PipelineKind::Ruling, id("ruling", "Q1").0),
            (
                PipelineKind::Campaign,
                format!("{CAMPAIGN}:campaign:self"),
                PipelineKind::Instance,
                format!("{CAMPAIGN}:instance:self"),
            ),
        ];
        for (left_kind, left_id, right_kind, right_id) in pairs {
            let left = resolution_of(left_kind, &left_id).expect("the left subject keys");
            let right = resolution_of(right_kind, &right_id).expect("the right subject keys");
            assert_ne!(left, right, "resolutions of {left_id} and {right_id} are two documents");
            for (kind, key) in [(left_kind, &left), (right_kind, &right)] {
                let (root, local) = pipeline_id("read_back", key, PipelineKind::Resolution).expect("reads back");
                assert_eq!(root, CAMPAIGN);
                assert_eq!(local.split('.').next(), Some(kind.name()), "{key} names its subject's kind");
            }
        }
    }

    /// Defect 3: a resolution's own key is an ordinary id, so it composes as a
    /// subject like any other, and the key reads back to the inner key -- the
    /// whole inner key, sequence included, since the subject is recovered by
    /// stripping this resolution's own sequence from the end and the kind from
    /// the front. The depth bound is tested per kind by
    /// `every_resolvable_kind_resolves_two_deep`.
    #[test]
    fn a_resolution_of_a_resolution_reads_back() {
        let inner = resolution_of(PipelineKind::Question, &id("question", "Q1").0).expect("the inner keys");
        let mut outer = resolution_sample();
        outer.subject = PipelineRef { kind: PipelineKind::Resolution, id: Short(inner.clone()) };
        let outer = PipelineDocument::Resolution(outer);
        assert_eq!(outer.validate(), Ok(()));
        let key = pipeline_key(&outer).expect("a resolution of a resolution keys");
        assert_eq!(key, format!("{CAMPAIGN}:resolution:resolution.question.Q1.n1.n1"));
        let (root, local) = pipeline_id("read_back", &key, PipelineKind::Resolution).expect("reads back");
        let (subject_kind, rest) = local.split_once('.').expect("the local names a subject");
        let (subject_local, _sequence) = rest.rsplit_once('.').expect("the local ends in this resolution's sequence");
        assert_eq!(format!("{root}:{subject_kind}:{subject_local}"), inner, "the recovered subject is the inner key");
    }

    /// Each level of a nesting carries its own sequence, and only its own: the
    /// part appended is this resolution's `sequence` field, never a copy of the
    /// sequence the subject already ends in. The fixtures elsewhere all sit at
    /// `1` at every level, where the two are indistinguishable, so the chain
    /// here differs at every level and reads back level by level.
    ///
    /// On spelling, so silence is not read as coverage: the leaf emits `n<N>`
    /// with `N` as `u32::to_string` writes it, so `n0` and `n12` are emitted
    /// and `n01` is not. A hand-built id ending `n01`, `n1x`, or no `n<N>` at
    /// all is still a well-formed reference -- those are legal labels, and a
    /// grammar that reads keys may not refuse them on spelling. Nothing in the
    /// leaf canonicalises them either: such an id simply names a document the
    /// leaf would never key, which admission's A7 sees as a missing referent.
    #[test]
    fn every_level_of_a_nesting_keeps_its_own_sequence() {
        let resolved = |kind: PipelineKind, subject: &str, sequence: u32| {
            let mut resolution = resolution_sample();
            resolution.subject = PipelineRef { kind, id: Short(subject.into()) };
            resolution.sequence = sequence;
            pipeline_key(&PipelineDocument::Resolution(resolution)).expect("a sequenced resolution keys")
        };
        let first = resolved(PipelineKind::Question, &id("question", "Q1").0, 9);
        let second = resolved(PipelineKind::Resolution, &first, 10);
        let third = resolved(PipelineKind::Resolution, &second, 0);
        let fourth = resolved(PipelineKind::Resolution, &third, 12);
        assert_eq!(first, format!("{CAMPAIGN}:resolution:question.Q1.n9"));
        assert_eq!(second, format!("{CAMPAIGN}:resolution:resolution.question.Q1.n9.n10"));
        assert_eq!(third, format!("{CAMPAIGN}:resolution:resolution.resolution.question.Q1.n9.n10.n0"));
        assert_eq!(
            fourth,
            format!("{CAMPAIGN}:resolution:resolution.resolution.resolution.question.Q1.n9.n10.n0.n12")
        );
        for (outer, inner, sequence) in [(&fourth, &third, 12), (&third, &second, 0), (&second, &first, 10)] {
            let (root, local) = pipeline_id("read_back", outer, PipelineKind::Resolution).expect("reads back");
            let (subject_kind, rest) = local.split_once('.').expect("the local names a subject");
            let (subject_local, last) = rest.rsplit_once('.').expect("the local ends in this resolution's sequence");
            assert_eq!(subject_kind, "resolution", "{outer}: the subject of a nesting is a resolution");
            assert_eq!(format!("{root}:{subject_kind}:{subject_local}"), *inner, "{outer}: the subject is the inner key");
            assert_eq!(last, format!("n{sequence}"), "{outer}: this level's own sequence, not the subject's");
        }
    }

    /// A stewardship is the other sequenced kind, and it composes as a subject
    /// like any other: the subject's whole local, escaped repo and sequence
    /// both, is the resolution's local, with the resolution's own sequence
    /// last. So the two assignments of one repo have two resolutions, and a
    /// resolution of either is itself resolvable.
    #[test]
    fn a_resolution_of_a_stewardship_reads_back() {
        let stewarded = |sequence: u32| {
            let mut stewardship = stewardship_sample();
            stewardship.sequence = sequence;
            pipeline_key(&PipelineDocument::Stewardship(stewardship)).expect("a sequenced stewardship keys")
        };
        let first = stewarded(1);
        let second = stewarded(2);
        assert_eq!(first, format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1"));
        assert_eq!(second, format!("{INSTANCE}:stewardship:gamecult_-epiphany.n2"));

        let resolved = resolution_of(PipelineKind::Stewardship, &first).expect("a resolution of a stewardship keys");
        let resolved_second = resolution_of(PipelineKind::Stewardship, &second).expect("the second assignment resolves");
        assert_eq!(resolved, format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n1"));
        assert_eq!(resolved_second, format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n2.n1"));
        assert_ne!(resolved, resolved_second, "each assignment keeps its own resolution");

        for (key, subject) in [(&resolved, &first), (&resolved_second, &second)] {
            let (root, local) = pipeline_id("read_back", key, PipelineKind::Resolution).expect("reads back");
            assert_eq!(root, INSTANCE, "{key}: a stewardship's resolution lives in the mind's root");
            let (subject_kind, rest) = local.split_once('.').expect("the local names a subject");
            let (subject_local, last) = rest.rsplit_once('.').expect("the local ends in this resolution's sequence");
            assert_eq!(subject_kind, PipelineKind::Stewardship.name(), "{key} names its subject's kind");
            assert_eq!(
                format!("{root}:{subject_kind}:{subject_local}"),
                *subject,
                "{key}: the subject's own sequence is part of the recovered key"
            );
            assert_eq!(last, "n1", "{key}: this resolution's own sequence");

            // A withdrawal of that resolution nests one out, as it does under
            // any other subject, and reads back to the resolution it withdraws.
            let withdrawal = resolution_of(PipelineKind::Resolution, key).expect("a withdrawal keys");
            let subject_local = key.split(':').nth(2).expect("three segments");
            assert_eq!(withdrawal, format!("{INSTANCE}:resolution:resolution.{subject_local}.n1"));
            let (root, local) = pipeline_id("read_back", &withdrawal, PipelineKind::Resolution).expect("reads back");
            let (subject_kind, rest) = local.split_once('.').expect("the local names a subject");
            let (subject_local, _) = rest.rsplit_once('.').expect("the local ends in its own sequence");
            assert_eq!(format!("{root}:{subject_kind}:{subject_local}"), **key, "{withdrawal}: the withdrawn resolution");
        }
    }

    /// A legal dotted local of exactly `length` bytes (`a.a.a`, each part a
    /// label), for bounds wider than one label.
    fn dotted_local(length: usize) -> String {
        match length % 2 {
            1 => format!("{}a", "a.".repeat(length / 2)),
            _ => format!("{}aa", "a.".repeat(length / 2 - 1)),
        }
    }

    /// A resolution of `subject`, at `sequence`, keyed.
    fn resolved_at(kind: PipelineKind, subject: &str, sequence: u32) -> Result<String, PipelineRefusal> {
        let mut resolution = resolution_sample();
        resolution.subject = PipelineRef { kind, id: Short(subject.into()) };
        resolution.sequence = sequence;
        pipeline_key(&PipelineDocument::Resolution(resolution))
    }

    /// every-subject-resolvable: for each kind that can be a subject, at the
    /// widest subject local (64) and `u32::MAX` sequences, depth one and depth
    /// two key and read back through `pipeline_id`; depth three is refused on
    /// the local bound.
    #[test]
    fn every_resolvable_kind_resolves_two_deep() {
        let wide = "a".repeat(SUBJECT_LOCAL_MAX);
        let mut widest = 0;
        for kind in PipelineKind::ALL.iter().copied().filter(|kind| *kind != PipelineKind::Resolution) {
            let subject = format!("{CAMPAIGN}:{}:{wide}", kind.name());
            assert_eq!(pipeline_id("subject", &subject, kind), Ok((CAMPAIGN, wide.as_str())), "{kind:?}: the subject parses");
            let one = resolved_at(kind, &subject, u32::MAX).unwrap_or_else(|error| panic!("{kind:?} depth one: {error}"));
            let two = resolved_at(PipelineKind::Resolution, &one, u32::MAX)
                .unwrap_or_else(|error| panic!("{kind:?} depth two: {error}"));
            for key in [&one, &two] {
                assert!(key.len() <= 200, "{key}: an id fits a Short");
                let (root, local) = pipeline_id("read_back", key, PipelineKind::Resolution)
                    .unwrap_or_else(|error| panic!("{kind:?}: {key} does not read back: {error}"));
                assert_eq!(root, CAMPAIGN);
                assert!(local.len() <= RESOLUTION_LOCAL_MAX);
            }
            widest = widest.max(two.len() - format!("{CAMPAIGN}:resolution:").len());
            assert!(
                matches!(
                    resolved_at(PipelineKind::Resolution, &two, u32::MAX),
                    Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "resolution.key"
                ),
                "{kind:?}: depth three is refused on the local bound"
            );
        }
        assert_eq!(widest, RESOLUTION_LOCAL_MAX, "the bound is exactly the widest depth-two chain");
        assert_eq!(RESOLUTION_LOCAL_MAX, 111);
    }

    #[test]
    fn the_longest_kind_name_is_the_longest() {
        let longest = PipelineKind::ALL.iter().map(|kind| kind.name().len()).max();
        assert_eq!(longest, Some(LONGEST_KIND_NAME));
    }

    /// The parser bounds an id's local by kind, exactly where the composer does.
    #[test]
    fn an_id_local_is_bounded_by_its_kind() {
        let at = |kind: PipelineKind, length: usize| {
            pipeline_id("probe", &format!("{CAMPAIGN}:{}:{}", kind.name(), dotted_local(length)), kind).map(|_| ())
        };
        assert_eq!(at(PipelineKind::Question, SUBJECT_LOCAL_MAX), Ok(()));
        assert!(at(PipelineKind::Question, SUBJECT_LOCAL_MAX + 1).is_err());
        assert_eq!(at(PipelineKind::Resolution, RESOLUTION_LOCAL_MAX), Ok(()));
        assert!(at(PipelineKind::Resolution, RESOLUTION_LOCAL_MAX + 1).is_err());
    }

    #[test]
    fn sha_names_same_commit() {
        let full = Sha("5f98228d1c2b3a4f5e6d7c8b9a0f1e2d3c4b5a69".into());
        assert!(full.names_same_commit(&Sha("5f98228".into())));
        assert!(Sha("5f98228".into()).names_same_commit(&full));
        assert!(full.names_same_commit(&full));
        assert!(!full.names_same_commit(&Sha("5f98229".into())));
        assert!(!Sha("5f98228".into()).names_same_commit(&Sha("5f98229".into())));
        assert!(!Sha("5f98228a".into()).names_same_commit(&Sha("5f98228b".into())), "equal length, differing");
    }

    /// stored-documents-valid and stored-bytes-reproduce, on the real mind.
    /// `HUGINN_MIND_SNAPSHOT` is a copy of a state root holding
    /// `minds/eureka/mind.redb`; the test copies the store file into a scratch
    /// directory first (redb opens read-write), so the snapshot may be
    /// read-only. Read through cultcache-rs's own store. Each pipeline
    /// document must re-prepare to its stored envelope: key, type and payload
    /// bytes.
    #[test]
    #[ignore = "needs HUGINN_MIND_SNAPSHOT, a copy of a Huginn state root; see the crate README"]
    fn stored_documents_read_back() {
        use cultcache_rs::{CacheBackingStore, OwnedRedbMessagePackBackingStore};
        let root = std::env::var("HUGINN_MIND_SNAPSHOT").expect("HUGINN_MIND_SNAPSHOT names the snapshot state root");
        let scratch = std::env::temp_dir().join(format!("eureka-pipeline-readback-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).expect("scratch directory");
        let working = scratch.join("mind.redb");
        std::fs::copy(Path::new(&root).join("minds").join("eureka").join("mind.redb"), &working).expect("the snapshot copies");
        let store = OwnedRedbMessagePackBackingStore::new(&working).expect("the snapshot opens");
        let cache = schema_cache().expect("the registrar registers");
        let envelopes = store.pull_all().expect("the snapshot reads");
        let mut stored = Vec::new();
        for envelope in &envelopes {
            match PipelineDocument::decode(envelope) {
                Ok(document) => {
                    assert_eq!(document.validate(), Ok(()), "{}: valid", envelope.key);
                    assert_eq!(pipeline_key(&document).as_deref(), Ok(envelope.key.as_str()), "{}: key derives byte for byte", envelope.key);
                    let again = document.prepare(&cache).unwrap_or_else(|error| panic!("{}: does not re-prepare: {error}", envelope.key));
                    assert_eq!(
                        (&again.key, &again.r#type, &again.payload),
                        (&envelope.key, &envelope.r#type, &envelope.payload),
                        "{}: re-encodes to the stored bytes",
                        envelope.key
                    );
                    stored.push((document.kind(), envelope.key.clone()));
                }
                Err(PipelineRefusal::ForeignStore { .. }) => {}
                Err(error) => panic!("{}: does not decode: {error}", envelope.key),
            }
        }
        assert!(stored.len() > 200, "{} pipeline documents read", stored.len());
        let mut subjects = 0;
        let resolvable = |kind: &PipelineKind| !matches!(kind, PipelineKind::Resolution | PipelineKind::Campaign | PipelineKind::Instance);
        for (kind, key) in stored.iter().filter(|(kind, _)| resolvable(kind)) {
            let one = resolved_at(*kind, key, 1).unwrap_or_else(|error| panic!("{key}: no resolution keys: {error}"));
            let two = resolved_at(PipelineKind::Resolution, &one, 1).unwrap_or_else(|error| panic!("{key}: no withdrawal keys: {error}"));
            for derived in [&one, &two] {
                pipeline_id("read_back", derived, PipelineKind::Resolution).unwrap_or_else(|error| panic!("{derived}: does not parse: {error}"));
            }
            subjects += 1;
        }
        for (_, key) in stored.iter().filter(|(kind, _)| *kind == PipelineKind::Resolution) {
            resolved_at(PipelineKind::Resolution, key, 1).unwrap_or_else(|error| panic!("{key}: cannot be withdrawn: {error}"));
        }
        // The six findings that could not be resolved before this cut.
        let stuck = [
            "cut-bifrost-retire-alarm.s2.verb-default-accepts-malformed",
            "cut-ops-notice-deploy.s2.pinned-bifrost-predates-retry",
            "cut-bifrost-notice-retry.s1.unknown-test-ignores-backoff",
            "cut-bifrost-notice-retry.s2.closed-unknown-retry-unpinned",
            "cut-bifrost-notice-retry.s2.flapping-clock-retries-every-tick",
            "cut-idunn-topology-lock.s2.boot-reconcile-skipped-on-contention",
        ];
        for label in stuck {
            let found = stored
                .iter()
                .find(|(kind, key)| *kind == PipelineKind::Finding && key.ends_with(label))
                .unwrap_or_else(|| panic!("{label}: not in the snapshot"));
            let one = resolved_at(PipelineKind::Finding, &found.1, 1).unwrap_or_else(|error| panic!("{label}: still stuck: {error}"));
            assert!(one.split(':').nth(2).is_some_and(|local| local.len() > SUBJECT_LOCAL_MAX), "{label}: the old bound refused it");
        }
        eprintln!("read back {} pipeline documents, {subjects} subjects", stored.len());
        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// The total bound, in the one place it lives: parts that are each a legal
    /// label compose a local wider than 64 bytes and are refused as a whole.
    #[test]
    fn a_composed_local_is_bounded_whole() {
        let mut wide = hand_off_sample();
        wide.to_instance = Slug("a".repeat(60));
        assert_eq!(wide.to_instance.validate("hand_off.to_instance"), Ok(()), "the receiver alone is legal");
        assert_eq!(
            label_text("part", &key_segment(&wide.repo.0)),
            Ok(()),
            "the repo alone is legal"
        );
        let key = pipeline_key(&PipelineDocument::HandOff(wide));
        assert!(
            matches!(&key, Err(PipelineRefusal::InvalidFormat { field, value }) if field == "hand_off.key" && value.len() > 64),
            "a local of legal parts is still bounded whole, got {key:?}"
        );

        // The bound is 64 exactly, on both sides, whether the part that fills
        // it is a plain label or an escaped repo. A hand-off local is
        // `<receiver>.<repo>.<date>`, the repo `gamecult_-epiphany` and the
        // date ten bytes, so the receiver fills the rest; a stewardship local is
        // `<repo escaped>.n<N>`, so `GameCult_-` and `.n1` bound the name.
        let hand_off_rest = "gamecult_-epiphany".len() + ".".len() + date().0.len() + ".".len();
        let stewardship_rest = "GameCult_-".len() + ".n1".len();
        let received = |receiver: usize| {
            let mut hand_off = hand_off_sample();
            hand_off.to_instance = Slug("a".repeat(receiver));
            pipeline_key(&PipelineDocument::HandOff(hand_off))
        };
        let stewarded = |name: usize| {
            let mut stewardship = stewardship_sample();
            stewardship.repo = OrgRepo(format!("GameCult/{}", "a".repeat(name)));
            pipeline_key(&PipelineDocument::Stewardship(stewardship))
        };
        for (label, at_64, at_65) in [
            ("plain label", received(64 - hand_off_rest), received(65 - hand_off_rest)),
            ("escaped repo", stewarded(64 - stewardship_rest), stewarded(65 - stewardship_rest)),
        ] {
            let keyed = at_64.unwrap_or_else(|error| panic!("{label}: a 64-byte local keys: {error}"));
            assert_eq!(keyed.split(':').nth(2).map(str::len), Some(64), "{label}: {keyed} fills the local exactly");
            assert!(
                matches!(&at_65, Err(PipelineRefusal::InvalidFormat { value, .. }) if value.len() == 65),
                "{label}: a 65-byte local is refused, got {at_65:?}"
            );
        }
    }

    /// The resolution side of the bound: 111 keys and 112 is refused on
    /// `resolution.key`, filled by a resolution subject (`resolution.<L>.n1`).
    #[test]
    fn a_composed_resolution_local_is_bounded_whole() {
        let overhead = "resolution.".len() + ".n1".len();
        let subject = |length: usize| format!("{CAMPAIGN}:resolution:{}", dotted_local(length));
        let at_max = resolved_at(PipelineKind::Resolution, &subject(RESOLUTION_LOCAL_MAX - overhead), 1)
            .expect("a 111-byte resolution local keys");
        assert_eq!(at_max.split(':').nth(2).map(str::len), Some(RESOLUTION_LOCAL_MAX));
        assert!(matches!(
            resolved_at(PipelineKind::Resolution, &subject(RESOLUTION_LOCAL_MAX - overhead + 1), 1),
            Err(PipelineRefusal::InvalidFormat { field, value }) if field == "resolution.key" && value.len() == RESOLUTION_LOCAL_MAX + 1
        ));
    }

    /// R2, on the composer every kind goes through: no local part carries the
    /// separator, the head included. The live case is a hand-off with a dotted
    /// receiver, which keys to an escaped label rather than a wider local.
    #[test]
    fn no_local_part_carries_the_separator() {
        assert_eq!(
            local("probe", PipelineKind::Question, &["a.b"]),
            Err(PipelineRefusal::InvalidFormat { field: "probe".into(), value: "a.b".into() }),
            "the head may not carry the separator"
        );
        assert_eq!(
            local("probe", PipelineKind::Question, &["a", "b.c"]),
            Err(PipelineRefusal::InvalidFormat { field: "probe".into(), value: "b.c".into() }),
            "a tail part may not carry the separator"
        );
        assert_eq!(local("probe", PipelineKind::Question, &["a", "b", "c"]), Ok("a.b.c".into()));

        let mut dotted = hand_off_sample();
        dotted.to_instance = slug("thought-cage.GameCult");
        let key = pipeline_key(&PipelineDocument::HandOff(dotted)).expect("a dotted receiver keys");
        let local = key.split(':').nth(2).expect("three segments");
        assert_eq!(local.split('.').count(), 3, "{key}: the receiver is one part, not two");
        assert_eq!(local.split('.').next(), Some("thought-cage_dGameCult"));
    }

    /// Q17 B: a subject's resolutions are distinct records, ordered by a
    /// per-subject sequence the key carries as its last part, so a withdrawn
    /// resolution stays under its subject and the next closure is nameable.
    /// The sequence is a field the writer sets and this crate only composes:
    /// it bounds nothing beyond `u32` and the local's own width, so `0` and a
    /// gap both key. Which sequence a document may claim, and whether an
    /// earlier resolution still stands, are admission's rules, exactly as
    /// `revision: 0` is.
    #[test]
    fn a_subject_keeps_every_resolution_it_had() {
        let resolved = |sequence: u32| {
            let mut resolution = resolution_sample();
            resolution.sequence = sequence;
            pipeline_key(&PipelineDocument::Resolution(resolution)).expect("a sequenced resolution keys")
        };
        let first = resolved(1);
        let second = resolved(2);
        assert_eq!(first, format!("{CAMPAIGN}:resolution:question.Q1.n1"));
        assert_eq!(second, format!("{CAMPAIGN}:resolution:question.Q1.n2"));
        assert_ne!(first, second, "two resolutions of one subject are two documents");
        for (sequence, key) in [(1, &first), (2, &second)] {
            let (root, local) = pipeline_id("read_back", key, PipelineKind::Resolution).expect("reads back");
            assert_eq!(root, CAMPAIGN, "{key}: the subject's root");
            let last = format!("n{sequence}");
            assert_eq!(local.rsplit('.').next(), Some(last.as_str()), "{key} ends in its sequence");
            assert_eq!(local.split('.').count(), 3, "{key}: subject kind, subject local, sequence");
        }

        // Outcome-invariance now holds per sequence: one record under two
        // outcomes is still one key, which is exactly why a second closure of
        // a subject needs a second sequence and not a second outcome.
        let mut withdrawn = resolution_sample();
        withdrawn.outcome = ResolutionOutcome::Withdrawn { reason: "moot".into() };
        assert_eq!(
            pipeline_key(&PipelineDocument::Resolution(withdrawn)),
            Ok(first.clone()),
            "one sequence is one key whatever the outcome"
        );

        // Stated limits. Both are admission's to refuse and neither is the
        // leaf's, so both compose here.
        assert_eq!(resolved(0), format!("{CAMPAIGN}:resolution:question.Q1.n0"), "the leaf refuses no sequence value");
        let mut widest = resolution_sample();
        widest.subject = PipelineRef { kind: PipelineKind::Ruling, id: id("ruling", "A") };
        widest.sequence = u32::MAX;
        assert_eq!(
            pipeline_key(&PipelineDocument::Resolution(widest)),
            Ok(format!("{CAMPAIGN}:resolution:ruling.A.n4294967295")),
            "the widest sequence is an eleven-byte label, not a refusal"
        );
    }

    /// D2: the sequence is the *last* part, so a subject's resolutions and
    /// only they share the prefix `<root>:resolution:<kind>.<local>.n`. R2 is
    /// what makes the prefix exact: no local part carries a dot, so
    /// `question.Q1.n` cannot match `question.Q10.n1`, and the withdrawal of
    /// `Q1`'s first resolution is a record of that resolution, one nesting
    /// out and outside `Q1`'s prefix. Pinned as a string property, which is
    /// what a prefix query is.
    #[test]
    fn a_subjects_resolutions_share_a_prefix_no_other_key_has() {
        let resolved = |kind: PipelineKind, subject: &str, sequence: u32| {
            let mut resolution = resolution_sample();
            resolution.subject = PipelineRef { kind, id: Short(subject.into()) };
            resolution.sequence = sequence;
            pipeline_key(&PipelineDocument::Resolution(resolution)).expect("the subject keys")
        };
        let history = [1, 2, 3].map(|sequence| resolved(PipelineKind::Question, &id("question", "Q1").0, sequence));
        let others = [
            resolved(PipelineKind::Question, &id("question", "Q10").0, 1),
            resolved(PipelineKind::Ruling, &id("ruling", "Q1").0, 1),
            resolved(PipelineKind::Campaign, &format!("{CAMPAIGN}:campaign:self"), 1),
            resolved(PipelineKind::Resolution, &history[0], 1),
        ];
        assert_eq!(
            others[3],
            format!("{CAMPAIGN}:resolution:resolution.question.Q1.n1.n1"),
            "a withdrawal keys under the resolution it withdraws, not under the subject"
        );
        let prefix = format!("{CAMPAIGN}:resolution:question.Q1.n");
        let selected = history
            .iter()
            .chain(others.iter())
            .filter(|key| key.starts_with(&prefix))
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            history.iter().collect::<Vec<_>>(),
            "the prefix selects the subject's resolutions and nothing else"
        );
    }

    /// Q18 A and Q20 A: a repo's stewardships on a mind are distinct records,
    /// ordered by a per-`(instance, repo)` sequence the key carries last, so a
    /// repo transferred away and later transferred back is two records under
    /// one prefix rather than one document overwriting the other. The date is
    /// not in the key: `assigned_on` is validated where every field is, by the
    /// derived `Bounded` impl, and two assignments differing only in it are
    /// one document.
    #[test]
    fn a_repo_keeps_every_stewardship_it_had() {
        let stewarded = |repo: &str, sequence: u32, assigned_on: &str| {
            let mut stewardship = stewardship_sample();
            stewardship.repo = OrgRepo(repo.into());
            stewardship.sequence = sequence;
            stewardship.assigned_on = Date(assigned_on.into());
            pipeline_key(&PipelineDocument::Stewardship(stewardship)).expect("a sequenced stewardship keys")
        };
        let first = stewarded("GameCult/Epiphany", 1, "2026-09-15");
        let second = stewarded("GameCult/Epiphany", 2, "2026-09-16");
        assert_eq!(first, format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1"));
        assert_eq!(second, format!("{INSTANCE}:stewardship:gamecult_-epiphany.n2"));
        assert_ne!(first, second, "two assignments of one repo are two documents");
        for (sequence, key) in [(1, &first), (2, &second)] {
            let (root, local) = pipeline_id("read_back", key, PipelineKind::Stewardship).expect("reads back");
            assert_eq!(root, INSTANCE, "{key}: the mind's root");
            let last = format!("n{sequence}");
            assert_eq!(local.rsplit('.').next(), Some(last.as_str()), "{key} ends in its sequence");
            assert_eq!(local.split('.').count(), 2, "{key}: the escaped repo and the sequence, nothing else");
        }

        assert_eq!(
            stewarded("GameCult/Epiphany", 1, "2026-09-16"),
            first,
            "assigned_on is a field, not a key part: one sequence is one key whatever the date"
        );

        let others = [
            stewarded("GameCult/Epiphany_thing", 1, "2026-09-15"),
            stewarded("GameCult/Huginn", 1, "2026-09-15"),
        ];
        let prefix = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n");
        let history = [first, second];
        let selected = history
            .iter()
            .chain(others.iter())
            .filter(|key| key.starts_with(&prefix))
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            history.iter().collect::<Vec<_>>(),
            "the prefix selects the repo's assignments on this mind and nothing else"
        );

        assert_eq!(
            stewarded("GameCult/Epiphany", 0, "2026-09-15"),
            format!("{INSTANCE}:stewardship:gamecult_-epiphany.n0"),
            "the leaf refuses no sequence value"
        );
    }

    /// Cut 8: the leaf owns the serialisation of the shapes it owns, so a wire
    /// carrying a document or a refusal reads back the same value rather than
    /// a mirror definition kept in step by hand in the reader's repo. Both
    /// encodings are pinned: JSON is what a schema describes, MessagePack is
    /// what a CultCache payload is written in, and an adjacent tag has to
    /// survive both. The tag is asserted against `name()` rather than against
    /// a literal, because the point of the tag is that a reader dispatching on
    /// a key's kind segment finds the same string here.
    #[test]
    fn every_document_and_refusal_serialises_and_reads_back() -> Result<()> {
        for (document, _) in samples() {
            let json = serde_json::to_value(&document)?;
            assert_eq!(json["kind"].as_str(), Some(document.kind().name()), "{json} is tagged with its kind name");
            assert_eq!(serde_json::from_value::<PipelineDocument>(json.clone())?, document, "{json} reads back");
            let packed = rmp_serde::to_vec_named(&document)?;
            assert_eq!(
                rmp_serde::from_slice::<PipelineDocument>(&packed)?,
                document,
                "{:?} reads back through MessagePack",
                document.kind()
            );
        }

        // Every variant, and every one of them carrying its named parts: a
        // refusal read off a wire names the field it refused, as a local one
        // does.
        let refusals = [
            PipelineRefusal::FieldBound { field: "campaign.title".into(), limit: 200, actual: 201 },
            PipelineRefusal::InvalidFormat { field: "campaign.slug".into(), value: "a b".into() },
            PipelineRefusal::InvalidIdentity {
                kind: PipelineKind::Campaign,
                key: format!("{CAMPAIGN}:campaign:forged"),
                expected: format!("{CAMPAIGN}:campaign:self"),
            },
            PipelineRefusal::DuplicateRepo { field: "campaign.repos".into(), repo: "gamecult/epiphany".into() },
            PipelineRefusal::ForeignStore { r#type: <ForeignDocument as DatabaseEntry>::TYPE.into() },
        ];
        for refusal in &refusals {
            let json = serde_json::to_value(refusal)?;
            assert_eq!(&serde_json::from_value::<PipelineRefusal>(json.clone())?, refusal, "{json} reads back");
            let packed = rmp_serde::to_vec_named(refusal)?;
            assert_eq!(
                &rmp_serde::from_slice::<PipelineRefusal>(&packed)?,
                refusal,
                "{refusal:?} reads back through MessagePack"
            );
        }

        // The derived schemas list the same two sets. A `const` in the tag
        // position is what a schema reader dispatches on, so it is read as the
        // schema writes it and compared to `name()`, not to a literal list.
        let tag = |variant: &serde_json::Value| {
            variant["properties"]["kind"]["const"]
                .as_str()
                .unwrap_or_else(|| panic!("a document variant schema carries a kind const: {variant}"))
                .to_owned()
        };
        let document_schema = serde_json::to_value(schemars::schema_for!(PipelineDocument))?;
        let variants = document_schema["oneOf"].as_array().expect("the document schema is a oneOf");
        assert_eq!(PipelineKind::ALL.len(), 13, "thirteen kinds");
        assert_eq!(
            variants.iter().map(tag).collect::<Vec<_>>(),
            PipelineKind::ALL.iter().map(|kind| kind.name().to_owned()).collect::<Vec<_>>(),
            "the document schema lists exactly the thirteen kinds, by name"
        );

        let refusal_schema = serde_json::to_value(schemars::schema_for!(PipelineRefusal))?;
        let refusal_variants = refusal_schema["oneOf"].as_array().expect("the refusal schema is a oneOf");
        let named = refusal_variants
            .iter()
            .map(|variant| {
                let properties = variant["properties"].as_object().expect("a refusal variant is an object");
                assert_eq!(properties.len(), 1, "{variant} is externally tagged");
                properties.keys().next().expect("one key").to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(named, vec!["FieldBound", "InvalidFormat", "InvalidIdentity", "DuplicateRepo", "ForeignStore"]);
        assert_eq!(named.len(), refusals.len(), "every refusal variant is in the schema and in the round trip");
        Ok(())
    }

    #[test]
    fn pipeline_published_schemas_match_derivation() -> Result<()> {
        let published = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/cultnet");
        let index: serde_json::Value = serde_json::from_slice(&std::fs::read(published.join("index.json"))?)?;
        // A directory this run alone writes, so everything in it is what this
        // run derived. A previous run's leftovers -- a mutation run's
        // especially, since those deliberately derive schemas no committed type
        // produces -- are not this derivation, and copying one of those into
        // `schemas/cultnet` publishes a schema no Rust type produces. A shared
        // directory cleared first would have to defend that clear against a
        // held handle; an unshared one has nothing to defend. The cost is that
        // nothing ever removes one. The directory is only created when
        // something is stale, so a passing run leaves none -- but a failing run
        // leaves its copies behind for good, one directory per failing run
        // where the shared design left one however often it failed. That is the
        // trade: the copies are the evidence the failure message names, and
        // they outlive the run that wrote them. Whoever reads the failure is
        // the one who deletes them.
        let derived_dir = std::env::temp_dir().join(format!(
            "eureka-pipeline-schemas-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut stale = Vec::new();
        for kind in PipelineKind::ALL {
            let file = format!("{}.schema.json", kind.type_id());
            let mut schema = serde_json::to_value(kind.derived_schema())?;
            schema["$id"] = format!("https://gamecult.dev/epiphany/cultnet/{file}").into();
            let derived = format!("{}\n", serde_json::to_string_pretty(&schema)?);
            if std::fs::read(published.join(&file)).ok().as_deref() != Some(derived.as_bytes()) {
                std::fs::create_dir_all(&derived_dir)?;
                std::fs::write(derived_dir.join(&file), &derived)?;
                stale.push(derived_dir.join(&file));
            }
            let entry = serde_json::json!({
                "schemaId": format!("https://gamecult.dev/epiphany/cultnet/{file}"),
                "kind": "document_payload",
                "wireContracts": ["cultnet.schema.v0"],
                "schemaVersion": kind.type_id(),
                "documentType": kind.type_id(),
                "title": format!("Epiphany Pipeline {kind:?} v2"),
                "path": file,
            });
            assert!(
                index["schemas"].as_array().is_some_and(|schemas| schemas.contains(&entry)),
                "index.json lacks {entry}"
            );
        }
        assert!(stale.is_empty(), "published pipeline schemas differ from the Rust derivation; derived copies: {stale:?}");
        Ok(())
    }

    /// A refusal read as text names its variant and its parts, so an operator
    /// who sees only the message (a log line, an `anyhow` chain) can tell which
    /// field was refused and why.
    #[test]
    fn a_refusal_read_as_text_names_its_variant_and_parts() {
        let text = PipelineRefusal::FieldBound { field: "title".into(), limit: 200, actual: 201 }.to_string();
        assert!(text.starts_with("pipeline refusal: "), "{text}");
        for part in ["FieldBound", "title", "200", "201"] {
            assert!(text.contains(part), "{part} is missing from {text}");
        }
    }

    /// The catalogue is exactly the files beside it: every entry's `path` is a
    /// schema file in the directory, and every schema file in the directory
    /// has an entry, so a schema removed from the catalogue (or a file left
    /// behind by a move) fails here instead of publishing a dangling or
    /// unlisted contract. The one rule: every `*.schema.json` in the directory,
    /// Huginn's own wire schemas included, is listed.
    #[test]
    fn the_catalogue_lists_exactly_the_schema_files_it_publishes() -> Result<()> {
        let published = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/cultnet");
        let index: serde_json::Value = serde_json::from_slice(&std::fs::read(published.join("index.json"))?)?;
        let listed = index["schemas"]
            .as_array()
            .expect("index.json carries a schemas array")
            .iter()
            .map(|entry| entry["path"].as_str().expect("every entry names its file").to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        for path in &listed {
            assert!(published.join(path).is_file(), "index.json lists {path}, which is not in the directory");
        }
        let mut files = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(&published)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if name.ends_with(".schema.json") {
                files.insert(name);
            }
        }
        assert_eq!(listed, files, "the catalogue and the schema files beside it disagree");
        Ok(())
    }

    /// The epoch is the version every kind is published at: its version
    /// segment is read from the constant, not restated, and every type id
    /// carries the same one, so a bumped epoch fails here until the kinds
    /// move with it. The live registrar registers the kinds and nothing else,
    /// so the foreign stand-in is not among them.
    #[test]
    fn every_kind_is_at_the_epochs_version() -> Result<()> {
        let (name, version) = PIPELINE_SCHEMA_EPOCH.rsplit_once('.').expect("the epoch carries a version");
        assert_eq!(name, "epiphany.pipeline.epoch");
        assert!(
            version.len() > 1 && version.starts_with('v') && version[1..].bytes().all(|byte| byte.is_ascii_digit()),
            "{version:?} is not a version segment"
        );
        for kind in PipelineKind::ALL {
            let type_id = kind.type_id();
            assert!(type_id.ends_with(&format!(".{version}")), "{type_id} is not at the epoch's version {version}");
        }

        let mut cache = CultCache::new();
        register_pipeline_document_types(&mut cache)?;
        let registered = cache.registered_entry_types();
        assert_eq!(registered.len(), PipelineKind::ALL.len(), "the live registrar registers exactly the kinds: {registered:?}");
        for kind in PipelineKind::ALL {
            assert!(registered.iter().any(|name| name == kind.type_id()), "{kind:?} is registered");
        }
        Ok(())
    }

    /// The public door onto the reference grammar, for the read side that
    /// holds a `kind` and an `id` and no way to reach `Bounded`. Every sample
    /// document's own key is a reference of its own kind; a kind that is not
    /// the id's, and a local no writer composes, are `InvalidFormat` at `ref.id`.
    #[test]
    fn a_ref_validates_as_an_id_of_the_kind_it_declares() -> Result<()> {
        for (document, expected) in samples() {
            let kind = document.kind();
            let key = pipeline_key(&document)?;
            assert_eq!(key, expected);
            assert_eq!(
                PipelineRef { kind, id: s(&key) }.validate_ref(),
                Ok(()),
                "{kind:?}'s own key is not a reference of its kind"
            );
        }

        let refused = |reference: PipelineRef, why: &str| {
            let read = reference.validate_ref();
            assert!(
                matches!(&read, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "ref.id"),
                "{why}: {reference:?} was not refused, got {read:?}"
            );
        };

        // The kind and the id's kind segment cannot disagree: the whole reason
        // the door exists, since a reader that cannot ask would look this up
        // and report the nothing it found.
        refused(
            PipelineRef { kind: PipelineKind::Campaign, id: id("question", "Q1") },
            "a question id is not a campaign reference",
        );
        refused(
            PipelineRef { kind: PipelineKind::Ruling, id: id("question", "Q1") },
            "a question id is not a ruling reference",
        );
        // The local is `dotted_text` whole, so a trailing dot is refused by the
        // reader and not only by the writer that never composes one.
        refused(
            PipelineRef { kind: PipelineKind::Question, id: s(&format!("{CAMPAIGN}:question:Q1.")) },
            "a trailing dot is no local",
        );

        // The boundary, pinned rather than assumed: a resolution id missing the
        // per-subject sequence its writer always composes is still a
        // well-formed id of its kind here. The leaf's grammar is kind-deep, and
        // the local's per-kind shape belongs to admission; refusing this would
        // be a new rule in `pipeline_id`, not a door onto the one that exists.
        assert_eq!(
            PipelineRef { kind: PipelineKind::Resolution, id: s(&format!("{CAMPAIGN}:resolution:question.Q1")) }
                .validate_ref(),
            Ok(()),
            "the leaf reads a resolution id kind-deep"
        );
        Ok(())
    }

    /// The public door onto the slug grammar, for a caller outside this crate
    /// holding a bare declared name (Huginn's `require_instance` and
    /// `Mind::open`, before either reaches admission or the filesystem). It
    /// applies exactly `dotted_text`: one valid dotted name, and refusals for
    /// fullwidth characters, `..`, both path separators, the empty string,
    /// and an empty label.
    #[test]
    fn slug_validate_applies_the_dotted_grammar() {
        assert_eq!(slug("huginn-yggdrasil").validate_slug(), Ok(()));
        assert_eq!(slug("outer.inner").validate_slug(), Ok(()));

        let refused = |value: &str, why: &str| {
            let result = slug(value).validate_slug();
            assert!(
                matches!(&result, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "slug"),
                "{why}: {value:?} was not refused, got {result:?}"
            );
        };

        refused("\u{FF41}\u{FF41}", "fullwidth characters are not ascii alphanumeric");
        refused("..", "a bare .. is an empty label either side of the dot");
        refused("../escaped", "a forward-slash path separator is not a label byte");
        refused("..\\escaped", "a backslash path separator is not a label byte");
        refused("outer\\inner", "a lone backslash inside one label is not a label byte, with no empty label to hide behind");
        refused("", "the empty string has no label");
        refused("outer..inner", "an empty label between two dots is refused");
        refused(".", "a single dot is one empty label");

        // S5: the 64-byte whole-name bound, pinned with a two-label name
        // whose parts sit far under 64 bytes each (31 and 32, then 32 and
        // 32), so only `dotted_text`'s own `value.len() > 64` check -- not
        // `label_text`'s per-part one -- can be the check that refuses 65.
        let filler = |n: usize| "a".repeat(n);
        let at_64 = format!("{}.{}", filler(31), filler(32));
        let at_65 = format!("{}.{}", filler(32), filler(32));
        assert_eq!(at_64.len(), 64, "a 64-byte whole name, two labels well under 64 each");
        assert_eq!(at_65.len(), 65, "one byte past the whole-name bound, same two-label shape");
        assert_eq!(slug(&at_64).validate_slug(), Ok(()));
        refused(&at_65, "65 bytes overall is refused, even though neither label alone reaches 64");

        // S5: the 64-byte per-label bound, pinned directly against `Label`
        // (the type `label_text` guards on its own, with no separate
        // whole-name wrapper), and with a lone label carrying no dot, so only
        // `label_text`'s own length check can be the one refusing 65.
        let label_at_64 = Label::from(filler(64).as_str());
        let label_at_65 = Label::from(filler(65).as_str());
        assert_eq!(label_at_64.validate("label"), Ok(()));
        assert!(
            matches!(
                label_at_65.validate("label"),
                Err(PipelineRefusal::InvalidFormat { ref field, .. }) if field == "label"
            ),
            "a 65-byte label was not refused, got {:?}",
            label_at_65.validate("label")
        );

        // S5: NUL is refused as a label byte, not merely as one more
        // non-alphanumeric character that happens to be caught by coincidence.
        refused("a\u{0}b", "NUL is not a label byte");
    }

    /// The two doors added beside `Slug::validate_slug` (RS-L), on its
    /// pattern: each delegates to the grammar its own document field already
    /// carries, so a declared name checked through the door and a field of
    /// the same type refuse the same inputs. `OrgRepo` is the `repo` alias's
    /// domain and `Label` is the `cut` alias's (RS-3's vocabulary table).
    #[test]
    fn the_org_repo_and_label_doors_are_the_grammar() {
        assert_eq!(OrgRepo::from("GameCult/Epiphany").validate_org_repo(), Ok(()));
        assert_eq!(OrgRepo::from("a/b").validate_org_repo(), Ok(()));
        assert_eq!(OrgRepo::from("a-b/c.d_e").validate_org_repo(), Ok(()));
        // Consecutive hyphens stay allowed on both sides; only a leading or
        // trailing owner hyphen is refused.
        assert_eq!(OrgRepo::from("a--b/c--d").validate_org_repo(), Ok(()));
        // S4: a 39-byte owner is accepted, a 100-byte repo is accepted.
        let owner_at_39 = format!("{}/b", "a".repeat(39));
        assert_eq!(OrgRepo::from(owner_at_39.as_str()).validate_org_repo(), Ok(()));
        let repo_at_100 = format!("a/{}", "a".repeat(100));
        assert_eq!(OrgRepo::from(repo_at_100.as_str()).validate_org_repo(), Ok(()));
        let repo_refused = |value: &str, why: &str| {
            let result = OrgRepo::from(value).validate_org_repo();
            assert!(
                matches!(&result, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "org_repo"),
                "{why}: {value:?} was not refused, got {result:?}"
            );
        };
        repo_refused("GameCult", "no slash at all");
        repo_refused("/Repo", "an empty org before the slash");
        repo_refused("GameCult/", "an empty repo after the slash");
        repo_refused("a/b/c", "a second slash makes the repo half ambiguous");
        repo_refused("../..", "an owner of dots is not GitHub's grammar");
        repo_refused("a/..", "a repo of .. is refused outright");
        repo_refused("a/.", "a repo of . is refused outright");
        repo_refused("-a/b", "a leading hyphen is not a valid owner");
        repo_refused("a-/b", "a trailing hyphen is not a valid owner");
        repo_refused("a b/c", "a space is not an owner byte");
        repo_refused("a/b c", "a space is not a repo byte");
        repo_refused("\u{e9}/\u{fc}", "non-ascii bytes are refused on either side");
        repo_refused("\0/\0", "NUL is refused on either side");
        repo_refused("a\n/b", "a newline is not an owner byte");
        let owner_at_40 = format!("{}/b", "a".repeat(40));
        repo_refused(&owner_at_40, "40 bytes is past the 39-byte owner bound");
        let repo_at_101 = format!("a/{}", "a".repeat(101));
        repo_refused(&repo_at_101, "101 bytes is past the 100-byte repo bound");
        repo_refused("a//b", "an empty middle segment is still a second slash");
        repo_refused("/b", "an empty owner is refused");
        repo_refused("a/", "an empty repo is refused");
        repo_refused("a.b/c", "a dot is not an owner byte");
        repo_refused("a_b/c", "an underscore is not an owner byte");
        // F7 fix batch: a repo name ending in `.git` is refused.
        repo_refused("a/b.git", "a repo ending in .git is refused");
        repo_refused("a/.git", "a repo of exactly .git is still a .git suffix");
        // S-2: the `.git` refusal is ASCII-case-insensitive, so a case variant
        // cannot dodge it and reach an `identity()` the validator refuses.
        repo_refused("a/b.GIT", "a repo ending in .GIT is refused like .git");
        repo_refused("a/.GIT", "a repo of exactly .GIT is still a .git suffix");
        repo_refused("A/B.GIT", "a mixed-case owner and a .GIT repo both refuse");
        repo_refused("a/b.GiT", "a mixed-case .git suffix is refused");

        // identity(): the one canonical key, ASCII-lowercased. Every
        // comparison, hash, ordering and key derivation goes through it.
        assert_eq!(
            OrgRepo::from("GameCult/Epiphany").identity(),
            OrgRepo::from("gamecult/epiphany").identity()
        );
        assert_eq!(OrgRepo::from("GameCult/Epiphany").identity(), "gamecult/epiphany");

        // S-1: it is structural, not advisory. `==` on `OrgRepo` compares
        // identity, not the raw string, so the two spellings are one
        // repository wherever `OrgRepo` is compared, hashed or ordered.
        assert_eq!(OrgRepo::from("GameCult/Epiphany"), OrgRepo::from("gamecult/epiphany"));
        assert_eq!(OrgRepo::from("GameCult/Epiphany").cmp(&OrgRepo::from("gamecult/epiphany")), std::cmp::Ordering::Equal);
        assert_eq!(
            OrgRepo::from("GameCult/Epiphany").partial_cmp(&OrgRepo::from("gamecult/epiphany")),
            Some(OrgRepo::from("GameCult/Epiphany").cmp(&OrgRepo::from("gamecult/epiphany"))),
            "partial_cmp must agree with cmp for an equal pair"
        );

        // RS-L closing fix 1: every assertion above compares values that
        // should be equal, so `eq -> true`, `hash -> ()` and `cmp -> Equal`
        // all survive as mutants. Pin the unequal direction as well: two
        // different repos must be unequal, hash differently (with
        // overwhelming probability, not by construction, so this is belt and
        // suspenders on top of the `!=` and `cmp` checks), and not compare
        // `Equal`, and `partial_cmp` must agree with `cmp` for an unequal
        // pair too, per std's own contract that `partial_cmp(a, b) ==
        // Some(cmp(a, b))`.
        let a = OrgRepo::from("GameCult/Epiphany");
        let b = OrgRepo::from("GameCult/Aetheria");
        assert_ne!(a, b, "two different repos must not compare equal");
        assert_ne!(
            a.cmp(&b),
            std::cmp::Ordering::Equal,
            "two different repos must not order as Equal"
        );
        assert_eq!(
            a.partial_cmp(&b),
            Some(a.cmp(&b)),
            "partial_cmp must agree with cmp for an unequal pair"
        );
        let hash_of = |repo: &OrgRepo| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            repo.hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(
            hash_of(&a),
            hash_of(&b),
            "two different repos must hash differently"
        );

        let mut hash_map = std::collections::HashMap::new();
        hash_map.insert(OrgRepo::from("GameCult/Epiphany"), 1);
        hash_map.insert(OrgRepo::from("gamecult/epiphany"), 2);
        assert_eq!(hash_map.len(), 1, "a HashMap keyed by both spellings holds one entry");
        assert_eq!(hash_map.get(&OrgRepo::from("GAMECULT/EPIPHANY")), Some(&2));

        let mut btree_map = std::collections::BTreeMap::new();
        btree_map.insert(OrgRepo::from("GameCult/Epiphany"), 1);
        btree_map.insert(OrgRepo::from("gamecult/epiphany"), 2);
        assert_eq!(btree_map.len(), 1, "a BTreeMap keyed by both spellings holds one entry");
        assert_eq!(btree_map.get(&OrgRepo::from("GAMECULT/EPIPHANY")), Some(&2));

        // `identity()` is always itself a valid `OrgRepo`: lowercasing never
        // turns a valid repository into one the grammar refuses, and never
        // turns a refused one into one it accepts.
        for value in ["GameCult/Epiphany", "A/B", "a-B/c.D_e", "GameCult/Epiphany.GIT"] {
            let repo = OrgRepo::from(value);
            assert_eq!(
                repo.validate_org_repo().is_ok(),
                OrgRepo::from(repo.identity().as_str()).validate_org_repo().is_ok(),
                "identity() of {value:?} disagreed with the value's own validity"
            );
        }

        assert_eq!(Label::from("cut-10").validate_label(), Ok(()));
        let label_refused = |value: &str, why: &str| {
            let result = Label::from(value).validate_label();
            assert!(
                matches!(&result, Err(PipelineRefusal::InvalidFormat { field, .. }) if field == "label"),
                "{why}: {value:?} was not refused, got {result:?}"
            );
        };
        label_refused("", "the empty string has no label");
        label_refused("a.b", "a dot is not a label byte");
        label_refused(&"a".repeat(65), "65 bytes is past the 64-byte label bound");
        label_refused("\u{e9}", "a non-ascii byte is not a label byte");
    }

    /// Q-RS1, ruled B: `question` and `ruling` gain a required `title`, the
    /// new `Title` type shared with the two titles that move off `Short`
    /// (campaign, cut spec). Each kind round-trips through `prepare`/`decode`
    /// with its title intact, an empty title is refused for both, and the
    /// bound is pinned at its edge: 200 bytes accepted, 201 refused (L3 dies
    /// on the removal of the non-empty check, not on the length bound, since
    /// `Title`'s length bound is shared with every other `bounded_text!`
    /// member and already covered by `bounds_refuse_in_utf8_bytes`).
    #[test]
    fn question_and_ruling_carry_a_title() -> Result<()> {
        let cache = schema_cache()?;

        let mut question = question_sample();
        question.title = t("Who owns the state?");
        let envelope = PipelineDocument::Question(question.clone()).prepare(&cache)?;
        assert_eq!(PipelineDocument::decode(&envelope)?, PipelineDocument::Question(question.clone()));

        let mut ruling = ruling_sample();
        ruling.title = t("An instance owns its mind");
        let envelope = PipelineDocument::Ruling(ruling.clone()).prepare(&cache)?;
        assert_eq!(PipelineDocument::decode(&envelope)?, PipelineDocument::Ruling(ruling.clone()));

        question.title = Title(String::new());
        assert_eq!(
            PipelineDocument::Question(question.clone()).validate(),
            Err(PipelineRefusal::InvalidFormat { field: "question.title".into(), value: String::new() })
        );
        ruling.title = Title(String::new());
        assert_eq!(
            PipelineDocument::Ruling(ruling.clone()).validate(),
            Err(PipelineRefusal::InvalidFormat { field: "ruling.title".into(), value: String::new() })
        );

        // F2 second fix batch: a title holds at least one
        // `char::is_alphanumeric()` character, no `char::is_control()`
        // character, and no bidi control character. No zero-width denylist:
        // a zero-width joiner/non-joiner inside real text is allowed, and a
        // string of nothing but invisible characters is refused because it
        // has no alphanumeric character, not because those characters are
        // individually denylisted.
        let refused = |value: &str| {
            let mut refused_question = question.clone();
            refused_question.title = Title(value.into());
            assert_eq!(
                PipelineDocument::Question(refused_question).validate(),
                Err(PipelineRefusal::InvalidFormat { field: "question.title".into(), value: value.into() }),
                "{value:?} should be refused"
            );
        };
        refused(" ");
        refused("\u{7f}");
        refused("a\u{7f}");
        refused("\u{0}");
        refused("\u{9f}");
        refused("\u{2028}");
        refused("\u{202e}"); // RLO alone
        refused("\u{202e}text"); // RLO plus text
        refused("\u{200e}"); // LRM alone
        refused("\u{0301}"); // a lone combining mark
        refused("\u{fe0f}"); // a variation selector alone
        refused("\u{a0}"); // NBSP alone
        refused("\u{1f680}"); // emoji only

        // S-3: every one of the twelve bidi control code points is pinned
        // with an embedded fixture (real alphanumeric text on both sides), so
        // the alphanumeric rule cannot mask a hole in the bidi match arm the
        // way a lone-character fixture would.
        refused("a\u{061C}b"); // ALM
        refused("a\u{200E}b"); // LRM
        refused("a\u{200F}b"); // RLM
        refused("a\u{202A}b"); // LRE
        refused("a\u{202B}b"); // RLE
        refused("a\u{202C}b"); // PDF
        refused("a\u{202D}b"); // LRO
        refused("a\u{202E}b"); // RLO
        refused("a\u{2066}b"); // LRI
        refused("a\u{2067}b"); // RLI
        refused("a\u{2068}b"); // FSI
        refused("a\u{2069}b"); // PDI

        // S-4: U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR are
        // refused inside otherwise-alphanumeric text, not only alone, since
        // `char::is_control()` does not reach `Zl`/`Zp`.
        refused("a\u{2028}b");
        refused("a\u{2029}b");

        let accepted = |value: &str| {
            let mut accepted_question = question.clone();
            accepted_question.title = Title(value.into());
            assert_eq!(
                PipelineDocument::Question(accepted_question).validate(),
                Ok(()),
                "{value:?} should validate"
            );
        };
        accepted("a");
        accepted("\u{e9}");
        accepted("\u{200c}\u{647}\u{6cc}"); // Persian text containing ZWNJ
        accepted("a\u{200d}b");
        accepted("a \u{1f680}");
        accepted(" a ");
        // Discrepancy from the fix spec: HANGUL FILLER (U+3164) is Unicode
        // General_Category Lo, so `char::is_alphanumeric()` is true for it in
        // this toolchain; it is not a denylist candidate, since the fix
        // batch deletes the hand-written exclusion list on purpose. A title
        // of U+3164 alone is therefore accepted, not refused.
        accepted("\u{3164}");

        question.title = Title("a".repeat(200));
        assert_eq!(PipelineDocument::Question(question.clone()).validate(), Ok(()));
        question.title = Title("a".repeat(201));
        assert_eq!(
            PipelineDocument::Question(question).validate(),
            Err(PipelineRefusal::FieldBound { field: "question.title".into(), limit: 200, actual: 201 })
        );
        Ok(())
    }
}
