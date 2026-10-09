//! The image and the batch, the two places a rule or a view may look, and
//! every derivation over them. "In force" is recursive: a document is in force
//! when no resolution that is itself in force names it, so a withdrawn
//! resolution stops closing its subject. Computed over image and batch at rule
//! time, over the image at read time, never stored, and never from
//! `stored_at`.

use cultcache_rs::CultCacheEnvelope;
use eureka_pipeline::{
    OrgRepo, PipelineDocument, PipelineKind, PipelineRef, PipelineResolution, PipelineStewardship, ResolutionOutcome,
    Short, Slug,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::refusal::MindRefusal;

/// A batch document after A3 and A5: decoded, keyed, and its envelope.
pub(crate) struct Staged {
    pub(crate) kind: PipelineKind,
    pub(crate) key: String,
    pub(crate) document: PipelineDocument,
    pub(crate) envelope: CultCacheEnvelope,
}

/// An image document, decoded.
pub(crate) struct Held {
    pub(crate) kind: PipelineKind,
    pub(crate) key: String,
    pub(crate) document: PipelineDocument,
}

/// The image and the batch, the two places a rule may look.
pub(crate) struct Docs {
    pub(crate) image: Vec<Held>,
    pub(crate) batch: Vec<Staged>,
}

pub(crate) fn kind_of_type(type_id: &str) -> Option<PipelineKind> {
    PipelineKind::ALL.iter().copied().find(|kind| kind.type_id() == type_id)
}

impl Docs {
    pub(crate) fn from_image(envelopes: &[CultCacheEnvelope]) -> Result<Self, MindRefusal> {
        let mut image = Vec::new();
        for envelope in envelopes {
            let Some(kind) = kind_of_type(&envelope.r#type) else { continue };
            let document = PipelineDocument::decode(envelope).map_err(|refusal| MindRefusal::Unavailable {
                detail: format!("stored document {} does not decode: {refusal}", envelope.key),
            })?;
            image.push(Held { kind, key: envelope.key.clone(), document });
        }
        Ok(Self { image, batch: Vec::new() })
    }

    /// A4: one identity, once.
    pub(crate) fn push(&mut self, staged: Staged) -> Result<(), MindRefusal> {
        if self.batch.iter().any(|other| other.kind == staged.kind && other.key == staged.key) {
            return Err(MindRefusal::IdentityCollision { kind: staged.kind, id: staged.key });
        }
        self.batch.push(staged);
        Ok(())
    }

    pub(crate) fn in_batch(&self, kind: PipelineKind, id: &str) -> Option<&PipelineDocument> {
        self.batch.iter().find(|staged| staged.kind == kind && staged.key == id).map(|staged| &staged.document)
    }

    pub(crate) fn in_image(&self, kind: PipelineKind, id: &str) -> Option<&PipelineDocument> {
        self.image.iter().find(|held| held.kind == kind && held.key == id).map(|held| &held.document)
    }

    pub(crate) fn find(&self, kind: PipelineKind, id: &str) -> Option<&PipelineDocument> {
        self.in_batch(kind, id).or_else(|| self.in_image(kind, id))
    }

    pub(crate) fn of_kind(&self, kind: PipelineKind) -> impl Iterator<Item = (&str, &PipelineDocument)> {
        let batch = self.batch.iter().filter(move |staged| staged.kind == kind).map(|staged| (staged.key.as_str(), &staged.document));
        let image = self.image.iter().filter(move |held| held.kind == kind).map(|held| (held.key.as_str(), &held.document));
        batch.chain(image)
    }

    pub(crate) fn resolutions(&self) -> impl Iterator<Item = (&str, &PipelineResolution)> {
        self.of_kind(PipelineKind::Resolution).filter_map(|(key, document)| match document {
            PipelineDocument::Resolution(resolution) => Some((key, resolution)),
            _ => None,
        })
    }

    pub(crate) fn stewardships(&self) -> impl Iterator<Item = (&str, &PipelineStewardship)> {
        self.of_kind(PipelineKind::Stewardship).filter_map(|(key, document)| match document {
            PipelineDocument::Stewardship(stewardship) => Some((key, stewardship)),
            _ => None,
        })
    }

    /// Every resolution of one subject, in image and batch: the scope a
    /// subject's sequence counts in.
    pub(crate) fn resolutions_of(&self, subject: &PipelineRef) -> impl Iterator<Item = (&str, &PipelineResolution)> {
        self.resolutions().filter(move |(_, resolution)| resolution.subject == *subject)
    }

    /// Every assignment of one repo to one mind, in image and batch, in force
    /// or not: the scope a stewardship's sequence counts in.
    pub(crate) fn assignments_of(&self, mind: &Slug, repo: &OrgRepo) -> impl Iterator<Item = (&str, &PipelineStewardship)> {
        self.stewardships().filter(move |(_, stewardship)| stewardship.instance == *mind && stewardship.repo == *repo)
    }

    /// The greatest sequence among a subject's resolutions in image or batch,
    /// `0` when it has none, so the next is `latest + 1`. `own` is the
    /// document being checked, excluded by content identity so that an exact
    /// replay measures the same `latest` the first admission did.
    pub(crate) fn latest_resolution(&self, subject: &PipelineRef, own: Option<&PipelineResolution>) -> u32 {
        self.resolutions_of(subject)
            .filter(|(_, resolution)| own != Some(*resolution))
            .map(|(_, resolution)| resolution.sequence)
            .max()
            .unwrap_or(0)
    }

    /// The sequence a derivation takes: the one the record it already derived
    /// carries, when image or batch holds that record, so an exact replay
    /// re-derives the same document and A9 answers with the stored receipt;
    /// otherwise the next. A derivation is recognised by what it writes into
    /// its record: a resolution by its outcome, an assignment by its note.
    pub(crate) fn derived_resolution_sequence(
        &self,
        subject: &PipelineRef,
        derived: impl Fn(&ResolutionOutcome) -> bool,
    ) -> u32 {
        self.resolutions()
            .find(|(_, resolution)| resolution.subject == *subject && derived(&resolution.outcome))
            .map_or_else(|| self.latest_resolution(subject, None) + 1, |(_, resolution)| resolution.sequence)
    }

    /// The same for the assignment a hand-off derives, which notes its key.
    pub(crate) fn derived_stewardship_sequence(&self, mind: &Slug, repo: &OrgRepo, note: &str) -> u32 {
        self.stewardships()
            .find(|(_, stewardship)| stewardship.instance == *mind && stewardship.repo == *repo && stewardship.note.0 == note)
            .map_or_else(|| self.latest_stewardship(mind, repo, None) + 1, |(_, stewardship)| stewardship.sequence)
    }

    /// The same for a repo's assignments to one mind.
    pub(crate) fn latest_stewardship(&self, mind: &Slug, repo: &OrgRepo, own: Option<&PipelineStewardship>) -> u32 {
        self.assignments_of(mind, repo)
            .filter(|(_, stewardship)| own != Some(*stewardship))
            .map(|(_, stewardship)| stewardship.sequence)
            .max()
            .unwrap_or(0)
    }

    /// No resolution that is itself in force names the document. The
    /// recursion is the whole rule: a withdrawn resolution stops counting, so
    /// its subject is open again. `key(R)` is strictly longer than the id it
    /// resolves, so the chain is bounded and this terminates.
    pub(crate) fn in_force(&self, kind: PipelineKind, id: &str) -> bool {
        self.in_force_unless(kind, id, |_| false)
    }

    /// In force, ignoring a resolution the caller's own document is or
    /// derives: the ruling that answers a question is what resolves it, the
    /// hand-off that withdraws a stewardship is what resolves that, and a
    /// resolution does not resolve its own subject out from under itself.
    pub(crate) fn in_force_unless(&self, kind: PipelineKind, id: &str, own: impl Fn(&PipelineResolution) -> bool) -> bool {
        self.closing_resolution_unless(kind, id, own).is_none()
    }

    /// The in-force resolution naming the document, other than `own`, if any;
    /// exact rather than a tie-break because A8 and the Q19 cap admit at most
    /// one.
    pub(crate) fn closing_resolution_unless(
        &self,
        kind: PipelineKind,
        id: &str,
        own: impl Fn(&PipelineResolution) -> bool,
    ) -> Option<(&str, &PipelineResolution)> {
        self.resolutions().find(|&(key, resolution)| {
            resolution.subject.kind == kind
                && resolution.subject.id.0 == id
                && !own(resolution)
                && self.in_force(PipelineKind::Resolution, key)
        })
    }

    /// The resolution that closes the document, if any: the views' status,
    /// derived where admission derives whether there is one at all.
    pub(crate) fn closing_resolution(&self, kind: PipelineKind, id: &str) -> Option<(&str, &PipelineResolution)> {
        self.closing_resolution_unless(kind, id, |_| false)
    }

    /// Every in-force stewardship of `(mind, repo)`, ignoring the withdrawal
    /// `hand_off_key` derives. The stewardship row holds one in force at a
    /// time, but that exclusion can put a second back in the list: the one a
    /// replayed hand-off withdrew, beside whatever stewards the repo now.
    pub(crate) fn stewardships_of(
        &self,
        mind: &Slug,
        repo: &OrgRepo,
        hand_off_key: Option<&str>,
    ) -> Vec<(&str, &PipelineStewardship)> {
        self.assignments_of(mind, repo)
            .filter(|(key, _)| {
                self.in_force_unless(PipelineKind::Stewardship, key, |resolution| {
                    matches!(&resolution.outcome, ResolutionOutcome::Withdrawn { reason } if Some(reason.0.as_str()) == hand_off_key)
                })
            })
            .collect()
    }

    /// The in-force record that stands later than `base` in the scope `base`
    /// is sequenced in, if any: what a withdrawal that put `base` back in
    /// force would leave standing beside it.
    pub(crate) fn later_in_force(&self, base: &PipelineDocument) -> Option<&str> {
        let kind = base.kind();
        self.of_kind(kind).find(|(key, other)| later_than(base, other) && self.in_force(kind, key)).map(|(key, _)| key)
    }

    /// The in-force run, other than `run_key`, that holds `claim`: the one
    /// consumption fact run-is-the-grant rests on, asked by a run's opening
    /// and by the withdrawal that would put a closed run back in force.
    pub(crate) fn claim_holder(&self, run_key: &str, claim: &PipelineRef) -> Option<&str> {
        self.of_kind(PipelineKind::Run).find_map(|(other_key, document)| match document {
            PipelineDocument::Run(other)
                if other_key != run_key && other.claims.contains(claim) && self.in_force(PipelineKind::Run, other_key) =>
            {
                Some(other_key)
            }
            _ => None,
        })
    }

    /// The stewardship of `(mind, repo)` a hand-off acts on: the one whose
    /// withdrawal already carries `hand_off_key`, when image or batch holds
    /// that withdrawal, so a replay derives the same withdrawal again and A9
    /// answers with the stored receipt; otherwise the one in force. Key order
    /// cannot pick between the two the exclusion leaves standing -- once a
    /// repo has changed hands ten times, `n11` sorts before `n2` -- so the
    /// content match picks, as it does for the sequence the other derivations
    /// take.
    pub(crate) fn stewardship_of(
        &self,
        mind: &Slug,
        repo: &OrgRepo,
        hand_off_key: Option<&str>,
    ) -> Option<(&str, &PipelineStewardship)> {
        let candidates = self.stewardships_of(mind, repo, hand_off_key);
        hand_off_key
            .and_then(|hand_off| {
                candidates.iter().find(|(key, _)| {
                    self.resolutions().any(|(_, resolution)| {
                        resolution.subject.kind == PipelineKind::Stewardship
                            && resolution.subject.id.0 == *key
                            && matches!(&resolution.outcome, ResolutionOutcome::Withdrawn { reason } if reason.0 == hand_off)
                    })
                })
            })
            .or_else(|| candidates.first())
            .copied()
    }
}

/// Whether `other` is a later record than `base` in the scope `base` is
/// sequenced in: `(instance, repo)` for a stewardship, the campaign for a
/// target and the cut for a cut spec. Kinds that carry no sequence have no
/// scope and no order, and a resolution is never a base here: the cap above
/// refuses the withdrawal that would reinstate one before the question of a
/// later record arises.
fn later_than(base: &PipelineDocument, other: &PipelineDocument) -> bool {
    use PipelineDocument as D;
    match (base, other) {
        (D::Stewardship(base), D::Stewardship(other)) => {
            other.instance == base.instance && other.repo == base.repo && other.sequence > base.sequence
        }
        (D::Target(base), D::Target(other)) => other.campaign == base.campaign && other.revision > base.revision,
        (D::CutSpec(base), D::CutSpec(other)) => {
            other.campaign == base.campaign && other.cut == base.cut && other.revision > base.revision
        }
        _ => false,
    }
}

/// The referring field of a citation edge, one per leaf field that names
/// another document. The wire spelling is the field's own name, so a client
/// reads an edge's role as the field it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CitationRole {
    RaisedIn,
    Answers,
    Rulings,
    Questions,
    CutSpec,
    Forks,
    CutReport,
    Findings,
    Verdict,
    Source,
    Subject,
    SupersededBy,
    ResolvedBy,
    DeferredTo,
    Documents,
    Claims,
}

impl CitationRole {
    pub const ALL: [CitationRole; 16] = [
        Self::RaisedIn,
        Self::Answers,
        Self::Rulings,
        Self::Questions,
        Self::CutSpec,
        Self::Forks,
        Self::CutReport,
        Self::Findings,
        Self::Verdict,
        Self::Source,
        Self::Subject,
        Self::SupersededBy,
        Self::ResolvedBy,
        Self::DeferredTo,
        Self::Documents,
        Self::Claims,
    ];

    /// The wire spelling, equal to the referring field's name.
    pub fn name(self) -> &'static str {
        match self {
            Self::RaisedIn => "raised_in",
            Self::Answers => "answers",
            Self::Rulings => "rulings",
            Self::Questions => "questions",
            Self::CutSpec => "cut_spec",
            Self::Forks => "forks",
            Self::CutReport => "cut_report",
            Self::Findings => "findings",
            Self::Verdict => "verdict",
            Self::Source => "source",
            Self::Subject => "subject",
            Self::SupersededBy => "superseded_by",
            Self::ResolvedBy => "resolved_by",
            Self::DeferredTo => "deferred_to",
            Self::Documents => "documents",
            Self::Claims => "claims",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.name() == name)
    }

    /// The kind whose documents carry this field.
    pub fn carrier(self) -> PipelineKind {
        use PipelineKind as K;
        match self {
            Self::RaisedIn => K::Question,
            Self::Answers => K::Ruling,
            Self::Rulings | Self::Questions => K::CutSpec,
            Self::CutSpec | Self::Forks => K::CutReport,
            Self::CutReport | Self::Findings => K::Verdict,
            Self::Verdict => K::Finding,
            Self::Source => K::FollowUp,
            Self::Subject | Self::SupersededBy | Self::ResolvedBy | Self::DeferredTo => K::Resolution,
            Self::Documents => K::HandOff,
            Self::Claims => K::Run,
        }
    }

    /// The kinds a referent of this field may be: the one kind the field is
    /// typed to, or every kind for the fields typed `PipelineRef` or a full
    /// id of any kind. "Any" is spelled as every kind, never as none.
    pub fn target_kinds(self) -> Vec<PipelineKind> {
        use PipelineKind as K;
        match self {
            Self::Answers | Self::Questions | Self::Forks => vec![K::Question],
            Self::Rulings => vec![K::Ruling],
            Self::CutSpec => vec![K::CutSpec],
            Self::CutReport => vec![K::CutReport],
            Self::Findings => vec![K::Finding],
            Self::Verdict => vec![K::Verdict],
            Self::Claims => vec![K::CutSpec, K::Finding, K::FollowUp],
            Self::RaisedIn
            | Self::Source
            | Self::Subject
            | Self::SupersededBy
            | Self::ResolvedBy
            | Self::DeferredTo
            | Self::Documents => PipelineKind::ALL.to_vec(),
        }
    }
}

/// The kind segment of a full id, for the fields typed `Short` whose referent
/// may be of any kind.
pub(crate) fn kind_of_id(id: &str) -> Option<PipelineKind> {
    let name = id.split(':').nth(1)?;
    PipelineKind::ALL.iter().copied().find(|kind| kind.name() == name)
}

fn cited(kind: PipelineKind, id: &Short) -> PipelineRef {
    PipelineRef { kind, id: id.clone() }
}

/// What a resolution's outcome names, by the field that names it.
pub(crate) fn outcome_citations(outcome: &ResolutionOutcome) -> Vec<(CitationRole, PipelineRef)> {
    match outcome {
        ResolutionOutcome::Superseded { by } => by.iter().map(|by| (CitationRole::SupersededBy, by.clone())).collect(),
        ResolutionOutcome::Answered { by } => vec![(CitationRole::ResolvedBy, by.clone())],
        ResolutionOutcome::Fixed { by: Some(by), .. } => vec![(CitationRole::ResolvedBy, by.clone())],
        ResolutionOutcome::Deferred { to } => vec![(CitationRole::DeferredTo, to.clone())],
        ResolutionOutcome::Fixed { by: None, .. } | ResolutionOutcome::Recorded { .. } | ResolutionOutcome::Withdrawn { .. } => {
            vec![]
        }
    }
}

/// Every citation edge a document carries, derived from its leaf fields each
/// time it is asked and never stored: the one edge list. Admission's A7 reads
/// it to require the referents, and the read side's rows read it as their
/// references. `Fixed.commit`, `ForeignRef` precedents and the labels
/// (`depends_on`, `invariants`, `promise`) are not edges. A hand-off's
/// `documents` entry whose kind segment does not read cites nothing here;
/// admission refuses such an entry on the source side before it asks.
pub(crate) fn citations(document: &PipelineDocument) -> Vec<(CitationRole, PipelineRef)> {
    use CitationRole as R;
    use PipelineDocument as D;
    use PipelineKind as K;
    let mut edges = Vec::new();
    match document {
        D::Question(question) => edges.extend(question.raised_in.iter().map(|to| (R::RaisedIn, to.clone()))),
        D::Ruling(ruling) => edges.extend(ruling.answers.iter().map(|to| (R::Answers, cited(K::Question, to)))),
        D::CutSpec(spec) => {
            edges.extend(spec.rulings.iter().map(|to| (R::Rulings, cited(K::Ruling, to))));
            edges.extend(spec.questions.iter().map(|to| (R::Questions, cited(K::Question, to))));
        }
        D::CutReport(report) => {
            edges.push((R::CutSpec, cited(K::CutSpec, &report.cut_spec)));
            edges.extend(report.forks.iter().map(|to| (R::Forks, cited(K::Question, to))));
        }
        D::Verdict(verdict) => {
            edges.push((R::CutReport, cited(K::CutReport, &verdict.cut_report)));
            edges.extend(
                verdict
                    .claims
                    .iter()
                    .flat_map(|claim| claim.findings.iter())
                    .map(|to| (R::Findings, cited(K::Finding, to))),
            );
        }
        D::Finding(finding) => edges.push((R::Verdict, cited(K::Verdict, &finding.verdict))),
        D::FollowUp(follow_up) => edges.push((R::Source, follow_up.source.clone())),
        D::Resolution(resolution) => {
            edges.push((R::Subject, resolution.subject.clone()));
            edges.extend(outcome_citations(&resolution.outcome));
        }
        D::HandOff(hand_off) => edges.extend(
            hand_off.documents.iter().filter_map(|to| kind_of_id(&to.0).map(|kind| (R::Documents, cited(kind, to)))),
        ),
        D::Run(run) => edges.extend(run.claims.iter().map(|to| (R::Claims, to.clone()))),
        D::Campaign(_) | D::Target(_) | D::Instance(_) | D::Stewardship(_) => {}
    }
    edges
}
