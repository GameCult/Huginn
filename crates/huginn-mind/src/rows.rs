//! What the read side gives CultNet's selection evaluator: the rows, the
//! organ's closed vocabulary, and the door that checks the values a selection
//! carries. Types, rules and boundaries only.
//!
//! The substrate owns the grammar of a selection's names, the order, the
//! cursor, the hop and paging. This module owns what a row answers for each
//! declared alias, which aliases and roles exist, and the value domain of
//! every alias: `cultnet_rs::validate` refuses a name it does not know and
//! never looks at a value, so a malformed filter would read as an empty
//! answer. `refuse_values` is the door that closes that (S6).

use std::collections::BTreeMap;

use cultnet_rs::{Row, RowSet, Selection};
use epiphany_pipeline::{OrgRepo, Label, PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome, Slug};

use crate::docs::{CitationRole, Held, kind_of_id, kind_of_type};
use crate::query::PipelineDocumentView;
use crate::refusal::MindRefusal;

/// The declared index aliases: what a row answers `values(alias)` with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alias {
    /// The key's root segment: a campaign slug, or an instance for
    /// stewardships, hand-offs and the identity document. A resolution's root
    /// is its subject's.
    Root,
    InForce,
    Faculty,
    /// `OrgRepo` identity spelling (lowercase), read through `base`.
    Repo,
    /// The label in `cut-<label>.`, read through `base`.
    Cut,
    Severity,
    Confidence,
    Origin,
    Authority,
    ClaimOutcome,
    Outcome,
}

const FACULTIES: [&str; 7] = ["SelfFaculty", "Imagination", "Hands", "Soul", "MindSteward", "Eyes", "Operator"];
const SEVERITIES: [&str; 4] = ["Blocker", "High", "Medium", "Low"];
const CONFIDENCES: [&str; 2] = ["Confirmed", "Plausible"];
const ORIGINS: [&str; 2] = ["Introduced", "PreExisting"];
const AUTHORITIES: [&str; 3] = ["Operator", "Standing", "Defaulted"];
const CLAIM_OUTCOMES: [&str; 3] = ["Holds", "Falsified", "Unproven"];
const OUTCOMES: [&str; 6] = ["Superseded", "Answered", "Fixed", "Deferred", "Recorded", "Withdrawn"];
const BOOLEANS: [&str; 2] = ["true", "false"];

impl Alias {
    const ALL: [Alias; 11] = [
        Self::Root,
        Self::InForce,
        Self::Faculty,
        Self::Repo,
        Self::Cut,
        Self::Severity,
        Self::Confidence,
        Self::Origin,
        Self::Authority,
        Self::ClaimOutcome,
        Self::Outcome,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::InForce => "in_force",
            Self::Faculty => "faculty",
            Self::Repo => "repo",
            Self::Cut => "cut",
            Self::Severity => "severity",
            Self::Confidence => "confidence",
            Self::Origin => "origin",
            Self::Authority => "authority",
            Self::ClaimOutcome => "claim_outcome",
            Self::Outcome => "outcome",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|alias| alias.name() == name)
    }

    fn declared_on(self, kind: PipelineKind) -> bool {
        use PipelineKind as K;
        match self {
            Self::Root | Self::InForce | Self::Faculty => true,
            Self::Repo => {
                matches!(kind, K::Campaign | K::CutSpec | K::CutReport | K::FollowUp | K::Stewardship | K::HandOff | K::Resolution)
            }
            Self::Cut => matches!(kind, K::CutSpec | K::CutReport | K::Verdict | K::Finding | K::Resolution),
            Self::Severity | Self::Confidence | Self::Origin => kind == K::Finding,
            Self::Authority => kind == K::Ruling,
            Self::ClaimOutcome => kind == K::Verdict,
            Self::Outcome => kind == K::Resolution,
        }
    }

    /// The closed set an enum-valued alias draws from.
    fn closed(self) -> Option<&'static [&'static str]> {
        match self {
            Self::InForce => Some(&BOOLEANS),
            Self::Faculty => Some(&FACULTIES),
            Self::Severity => Some(&SEVERITIES),
            Self::Confidence => Some(&CONFIDENCES),
            Self::Origin => Some(&ORIGINS),
            Self::Authority => Some(&AUTHORITIES),
            Self::ClaimOutcome => Some(&CLAIM_OUTCOMES),
            Self::Outcome => Some(&OUTCOMES),
            Self::Root | Self::Repo | Self::Cut => None,
        }
    }

    /// Whether `value` is in this alias's domain: the leaf's own grammar for
    /// the three grammar-valued aliases, exact spelling for the closed sets.
    fn admits(self, value: &str) -> bool {
        match self {
            Self::Root => Slug(value.into()).validate_slug().is_ok(),
            Self::Repo => OrgRepo(value.into()).validate_org_repo().is_ok(),
            Self::Cut => Label(value.into()).validate_label().is_ok(),
            closed => closed.closed().is_some_and(|set| set.contains(&value)),
        }
    }
}

/// The root and the local of a key, by position in `<root>:<kind>:<local>`:
/// the leaf derives every key that way and its own test pins the three
/// segments. The only place the read side reads a key apart.
pub(crate) fn root_and_local(key: &str) -> (&str, &str) {
    let mut segments = key.split(':');
    let root = segments.next().unwrap_or_default();
    let local = segments.nth(1).unwrap_or_default();
    (root, local)
}

fn outcome_name(outcome: &ResolutionOutcome) -> &'static str {
    match outcome {
        ResolutionOutcome::Superseded { .. } => "Superseded",
        ResolutionOutcome::Answered { .. } => "Answered",
        ResolutionOutcome::Fixed { .. } => "Fixed",
        ResolutionOutcome::Deferred { .. } => "Deferred",
        ResolutionOutcome::Recorded { .. } => "Recorded",
        ResolutionOutcome::Withdrawn { .. } => "Withdrawn",
    }
}

/// One document as the evaluator sees it: its identity, its ordinal, what it
/// answers for each alias, its citation edges, and the view a page projects.
#[derive(Clone, Debug)]
pub(crate) struct SelectionRow {
    pub(crate) kind: PipelineKind,
    pub(crate) key: String,
    ordinal: u64,
    values: BTreeMap<&'static str, Vec<String>>,
    refs: Vec<(CitationRole, PipelineRef)>,
    pub(crate) view: PipelineDocumentView,
}

impl SelectionRow {
    /// `base` is the first non-resolution the document reaches through its
    /// subjects: the row a resolution's `repo` and `cut` are read from.
    pub(crate) fn new(
        held: &Held,
        base: &Held,
        view: PipelineDocumentView,
        refs: Vec<(CitationRole, PipelineRef)>,
    ) -> Self {
        use PipelineDocument as D;
        let mut values: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        let mut put = |alias: Alias, found: Vec<String>| {
            values.insert(alias.name(), found);
        };
        put(Alias::Root, vec![root_and_local(&held.key).0.to_string()]);
        put(Alias::InForce, vec![matches!(view.status, crate::query::PipelineStatus::InForce).to_string()]);
        put(Alias::Faculty, vec![format!("{:?}", view.admission.provenance.faculty)]);
        put(
            Alias::Repo,
            match &base.document {
                D::Campaign(campaign) => campaign.repos.iter().map(OrgRepo::identity).collect(),
                D::CutSpec(spec) => vec![spec.repo.identity()],
                D::CutReport(report) => vec![report.repo.identity()],
                D::FollowUp(follow_up) => vec![follow_up.repo.identity()],
                D::Stewardship(stewardship) => vec![stewardship.repo.identity()],
                D::HandOff(hand_off) => vec![hand_off.repo.identity()],
                D::Target(_) | D::Question(_) | D::Ruling(_) | D::Verdict(_) | D::Finding(_) | D::Instance(_) | D::Resolution(_) => {
                    Vec::new()
                }
            },
        );
        put(
            Alias::Cut,
            root_and_local(&base.key)
                .1
                .strip_prefix("cut-")
                .and_then(|rest| rest.split_once('.'))
                .map(|(label, _)| label.to_string())
                .into_iter()
                .collect(),
        );
        match &held.document {
            D::Finding(finding) => {
                put(Alias::Severity, vec![format!("{:?}", finding.severity)]);
                put(Alias::Confidence, vec![format!("{:?}", finding.confidence)]);
                put(Alias::Origin, vec![format!("{:?}", finding.origin)]);
            }
            D::Ruling(ruling) => put(Alias::Authority, vec![format!("{:?}", ruling.authority)]),
            D::Verdict(verdict) => {
                put(Alias::ClaimOutcome, verdict.claims.iter().map(|claim| format!("{:?}", claim.outcome)).collect())
            }
            D::Resolution(resolution) => put(Alias::Outcome, vec![outcome_name(&resolution.outcome).to_string()]),
            _ => {}
        }
        Self { kind: held.kind, key: held.key.clone(), ordinal: view.admission.ordinal, values, refs, view }
    }
}

impl Row for SelectionRow {
    fn schema_id(&self) -> &str {
        self.kind.type_id()
    }

    fn record_key(&self) -> &str {
        &self.key
    }

    fn ordinal(&self) -> i64 {
        self.ordinal as i64
    }

    fn values(&self, index: &str) -> Vec<String> {
        self.values.get(index).cloned().unwrap_or_default()
    }

    fn number(&self, _index: &str) -> Option<String> {
        None
    }

    fn references(&self) -> Vec<(String, String, Option<Vec<u8>>)> {
        self.refs.iter().map(|(role, target)| (role.name().to_string(), target.id.0.clone(), None)).collect()
    }
}

/// The organ's closed lists: thirteen schemas, eleven aliases, fifteen roles.
/// A schema's name is its id, so the substrate's alias matching resolves
/// nothing beyond exact ids.
pub(crate) struct Vocabulary;

impl RowSet for Vocabulary {
    fn all_schema_ids(&self) -> Vec<String> {
        PipelineKind::ALL.iter().map(|kind| kind.type_id().to_string()).collect()
    }

    fn schema_name(&self, schema_id: &str) -> Option<String> {
        kind_of_type(schema_id).map(|kind| kind.type_id().to_string())
    }

    fn declared_indexes(&self, schema_id: &str) -> Vec<String> {
        let Some(kind) = kind_of_type(schema_id) else { return Vec::new() };
        Alias::ALL.into_iter().filter(|alias| alias.declared_on(kind)).map(|alias| alias.name().to_string()).collect()
    }

    fn declared_roles(&self, schema_id: &str) -> Vec<String> {
        let Some(kind) = kind_of_type(schema_id) else { return Vec::new() };
        CitationRole::ALL.into_iter().filter(|role| role.carrier() == kind).map(|role| role.name().to_string()).collect()
    }

    fn is_numeric(&self, _schema_id: &str, _index: &str) -> bool {
        false
    }

    fn target_leaves(&self, role: &str) -> Vec<String> {
        CitationRole::from_name(role)
            .map(|role| role.target_kinds().into_iter().map(|kind| kind.type_id().to_string()).collect())
            .unwrap_or_default()
    }
}

fn invalid(field: impl Into<String>, value: &str, message: impl Into<String>) -> MindRefusal {
    MindRefusal::SelectionInvalid { field: field.into(), value: Some(value.to_string()), message: message.into() }
}

/// The organ's door (S6), run after `cultnet_rs::validate` has accepted the
/// selection's names: it refuses a value the vocabulary's domains do not hold,
/// so a malformed filter never reads as an empty answer. It also returns the
/// selection with every `repo` value in the identity spelling the rows carry,
/// because `OrgRepo` identity is case-insensitive.
///
/// Refuses, naming the field the client sent: a `schemas` entry that is not
/// one of the thirteen ids; a `keys` entry that is not a full id whose kind
/// segment reads and passes the leaf's ref grammar; an `any_of` value outside
/// its alias's domain; and a `cites.target` whose key fails the ref grammar
/// for its schema's kind (V24's rule, at this door). Whether the target
/// schema id is known is the substrate's refusal and is not repeated here.
pub(crate) fn refuse_values(selection: &Selection) -> Result<Selection, MindRefusal> {
    for schema in selection.schemas.iter().flatten() {
        if kind_of_type(schema).is_none() {
            return Err(invalid("schemas", schema, "no such schema in this mind"));
        }
    }
    for key in selection.keys.iter().flatten() {
        let Some(kind) = kind_of_id(key) else {
            return Err(invalid("keys", key, "not a pipeline id: the kind segment does not read"));
        };
        if let Err(refusal) = (PipelineRef { kind, id: key.as_str().into() }).validate_ref() {
            return Err(invalid("keys", key, format!("not a pipeline id: {refusal:?}")));
        }
    }
    let mut canonical = selection.clone();
    for (position, predicate) in canonical.fields.iter_mut().flatten().enumerate() {
        let Some(alias) = Alias::from_name(&predicate.index) else { continue };
        for value in predicate.values.iter_mut().flatten() {
            if !alias.admits(value) {
                return Err(invalid(
                    format!("fields[{position}].values"),
                    value,
                    match alias.closed() {
                        Some(set) => format!("{} takes one of {}", alias.name(), set.join(", ")),
                        None => format!("not a valid {}", alias.name()),
                    },
                ));
            }
            if alias == Alias::Repo {
                *value = OrgRepo(value.clone()).identity();
            }
        }
    }
    if let Some(cites) = &selection.cites
        && let Some(kind) = kind_of_type(&cites.target.schema_id)
        && let Err(refusal) = (PipelineRef { kind, id: cites.target.record_key.as_str().into() }).validate_ref()
    {
        return Err(invalid("cites.target", &cites.target.record_key, format!("not a {} id: {refusal:?}", kind.name())));
    }
    Ok(canonical)
}
