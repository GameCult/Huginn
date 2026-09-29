//! What of a document is searchable, and nothing else about the index.
//!
//! `index_text` is a rule about the leaf's documents: which fields are prose a
//! reader would search for. It is a pure function of one document, so it reads
//! no receipt, no provenance and no status, and nothing derived from it is
//! stored: the daemon's projection recomputes it, and `INDEX_TEXT_VERSION` is
//! how a change to it makes every projection rebuild. No network, no JSON, no
//! store handle: the daemon owns the vector store and the embedder.

use epiphany_pipeline::{PipelineDocument, PipelineKind, PipelineRef};
use sha2::{Digest, Sha256};

use crate::docs::Docs;
use crate::mind::Mind;
use crate::query::AdmissionIndex;
use crate::refusal::MindRefusal;
use crate::rows::root_and_local;
use crate::store::MindStore;

/// Bumped whenever `index_text` changes what it returns for any document.
pub const INDEX_TEXT_VERSION: u32 = 1;

/// The most bytes one document's text may carry, cut at a character boundary.
pub const INDEX_TEXT_MAX_BYTES: usize = 16 * 1024;

/// One document as the projection needs it: the key, the searchable text and
/// the digest of that text, and the ordinal of the receipt that wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub id: PipelineRef,
    /// The key's root segment: a campaign slug, or an instance.
    pub root: String,
    pub text: String,
    pub text_sha256: String,
    pub ordinal: u64,
}

/// The searchable text of a document, or `None` when its kind carries none
/// (`instance`, `stewardship`) or every field it names is empty. Lines are
/// joined by newlines. Withdrawn resolutions are indexed like any other: a
/// closed thing stays findable.
pub fn index_text(document: &PipelineDocument) -> Option<String> {
    use PipelineDocument as D;
    let mut lines: Vec<&str> = Vec::new();
    match document {
        D::Campaign(campaign) => lines.push(&campaign.title.0),
        D::Target(target) => {
            lines.extend(target.invariants.iter().map(|invariant| invariant.statement.0.as_str()));
            lines.extend(target.not_in_scope.iter().map(|line| line.0.as_str()));
            lines.extend(target.canonical_implementations.iter().map(|line| line.0.as_str()));
        }
        D::Question(question) => {
            lines.push(&question.title.0);
            lines.push(&question.question.0);
            lines.extend(question.options.iter().map(|option| option.text.0.as_str()));
        }
        D::Ruling(ruling) => {
            lines.push(&ruling.title.0);
            lines.push(&ruling.ruling.0);
            lines.extend(ruling.operator_quote.iter().map(|quote| quote.0.as_str()));
        }
        D::CutSpec(spec) => {
            lines.push(&spec.title.0);
            lines.extend(spec.first.iter().map(|line| line.0.as_str()));
            lines.extend(spec.adds.iter().map(|line| line.0.as_str()));
            lines.extend(spec.keeps_moves.iter().map(|line| line.0.as_str()));
        }
        D::CutReport(report) => {
            lines.extend(report.commits.iter().map(|commit| commit.subject.0.as_str()));
            for deviation in &report.deviations {
                lines.push(&deviation.what.0);
                lines.push(&deviation.why.0);
            }
            lines.extend(report.undone.iter().map(|line| line.0.as_str()));
            lines.extend(report.promises.iter().map(|promise| promise.text.0.as_str()));
        }
        D::Verdict(verdict) => lines.extend(verdict.claims.iter().map(|claim| claim.claim.0.as_str())),
        D::Finding(finding) => {
            lines.push(&finding.claim.0);
            lines.push(&finding.failure_scenario.0);
            lines.extend(finding.evidence.iter().map(|evidence| evidence.result.0.as_str()));
        }
        D::FollowUp(follow_up) => {
            lines.push(&follow_up.item.0);
            lines.push(&follow_up.why_it_can_wait.0);
        }
        D::Resolution(resolution) => lines.push(&resolution.rationale.0),
        D::HandOff(hand_off) => lines.push(&hand_off.reason.0),
        D::Instance(_) | D::Stewardship(_) => {}
    }
    lines.retain(|line| !line.is_empty());
    if lines.is_empty() {
        return None;
    }
    let mut text = lines.join("\n");
    if text.len() > INDEX_TEXT_MAX_BYTES {
        let mut cut = INDEX_TEXT_MAX_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    Some(text)
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

impl<S: MindStore> Mind<S> {
    /// The entries to project: every indexable document the mind holds, or
    /// only the named ones. The ordinal is the writing receipt's. A named
    /// document the mind does not hold is a refusal, not a silent omission: a
    /// caller names what a commit just landed.
    pub fn index_entries(&self, only: Option<&[PipelineRef]>) -> Result<Vec<IndexEntry>, MindRefusal> {
        let documents: Vec<(PipelineKind, String, PipelineDocument)> = match only {
            Some(refs) => {
                let mut held = Vec::with_capacity(refs.len());
                for reference in refs {
                    reference.validate_ref()?;
                    let document = self.get(reference.kind, &reference.id.0)?.ok_or_else(|| {
                        MindRefusal::Unavailable { detail: format!("index: {} is not held", reference.id.0) }
                    })?;
                    held.push((reference.kind, reference.id.0.clone(), document));
                }
                held
            }
            None => Docs::from_image(self.envelopes())?
                .image
                .into_iter()
                .map(|held| (held.kind, held.key, held.document))
                .collect(),
        };
        let facts = AdmissionIndex::build(self)?;
        let mut entries = Vec::new();
        for (kind, key, document) in documents {
            let Some(text) = index_text(&document) else { continue };
            let ordinal = facts.ordinal_of(kind, &key)?;
            entries.push(IndexEntry {
                root: root_and_local(&key).0.to_string(),
                id: PipelineRef { kind, id: key.as_str().into() },
                text_sha256: sha256_hex(&text),
                text,
                ordinal,
            });
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::receipt::Faculty;
    use crate::store::test_stores::MemoryStore;
    use epiphany_pipeline::{
        ClaimOutcome, Deviation, FindingConfidence, PipelineDocument as D, PipelineKind as K, RulingAuthority,
    };
    use std::collections::BTreeMap;

    /// A mind holding one document of every kind that carries text, each with
    /// more than one text field where the kind has more than one, and the
    /// question's closure withdrawn beside them. Returns the hand-off's key.
    fn full_mind() -> (Mind<MemoryStore>, String) {
        let mut mind = opened(MemoryStore::new(), INSTANCE);
        let D::Target(mut target_doc) = target(1, &[INVARIANT, "second"]) else { panic!() };
        target_doc.not_in_scope = vec!["No client writes.".into()];
        target_doc.canonical_implementations = vec!["huginn-mind admission".into()];
        committed(admit(
            &mut mind,
            vec![instance(INSTANCE), stewardship(INSTANCE, REPO), campaign(&[REPO]), D::Target(target_doc)],
        ));
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let mut ruling_doc = ruling("R1");
        ruling_doc.answers = Some(s(&id("question", "Q1")));
        ruling_doc.choice = Some(l("A"));
        ruling_doc.operator_quote = Some("Own it.".into());
        ruling_doc.authority = RulingAuthority::Operator;
        committed(admit(&mut mind, vec![D::Ruling(ruling_doc)]));
        // The derived closure of Q1 is withdrawn: a withdrawn resolution is a
        // document like any other and stays findable.
        let closed = id("resolution", "question.Q1.n1");
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &closed), withdrawn())]));
        let mut spec_doc = cut_spec("1", 1);
        spec_doc.first = vec!["Read the leaf.".into()];
        spec_doc.adds = vec!["An index module.".into(), "A worker.".into()];
        spec_doc.keeps_moves = vec!["Admission stays.".into()];
        committed(admit(&mut mind, vec![D::CutSpec(spec_doc)]));
        let mut report_doc = cut_report("1", 1);
        report_doc.deviations = vec![Deviation { what: "Changed a name.".into(), why: "It collided.".into() }];
        report_doc.undone = vec!["The live smoke.".into()];
        committed(admit(
            &mut mind,
            vec![D::CutReport(report_doc), follow_up("FU-1", r(K::Question, &id("question", "Q1")))],
        ));
        committed(admit(
            &mut mind,
            vec![
                verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&id("finding", "cut-1.s1.F1")], Some("P1"), &["M1"])]),
                D::Finding(finding("1", 1, "F1", FindingConfidence::Confirmed)),
            ],
        ));
        let (_, writes) = committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        let hand_off_key = writes.iter().find(|write| write.kind == K::HandOff).unwrap().id.0.clone();
        (mind, hand_off_key)
    }

    fn texts(mind: &Mind<MemoryStore>) -> BTreeMap<String, String> {
        mind.index_entries(None).unwrap().into_iter().map(|entry| (entry.id.id.0, entry.text)).collect()
    }

    /// Each kind's search text is its named fields, joined by newlines, and
    /// the two kinds with no prose have no entry.
    #[test]
    fn index_text_is_each_kinds_named_fields_and_the_identity_kinds_have_none() {
        let (mind, hand_off_key) = full_mind();
        let expected: Vec<(String, &str)> = vec![
            (id("campaign", "self"), "Eureka pipeline state"),
            (id("target", "r1"), "Only admission writes.\nOnly admission writes.\nNo client writes.\nhuginn-mind admission"),
            (id("question", "Q1"), "Who owns the state?\nWho owns the state?\nan option\nan option"),
            (id("ruling", "R1"), "An instance owns its mind\nAn instance owns its mind.\nOwn it."),
            (id("cut_spec", "cut-1.r1"), "A cut\nRead the leaf.\nAn index module.\nA worker.\nAdmission stays."),
            (
                id("cut_report", "cut-1.h1"),
                "Land the cut\nChanged a name.\nIt collided.\nThe live smoke.\nOne derived key per document.",
            ),
            (id("verdict", "cut-1.s1"), "Falsified claim"),
            (id("finding", "cut-1.s1.F1"), "A dotted label composes two keys.\nTwo documents claim one key.\nok"),
            (id("follow_up", "FU-1"), "Per-kind admission rules.\nThe organ owns admission."),
            // The withdrawn closure of Q1, admitted by hand.
            (id("resolution", "resolution.question.Q1.n1.n1"), "Resolved."),
            (hand_off_key, "The workstation mind takes the campaign."),
        ];
        let held = texts(&mind);
        for (key, text) in &expected {
            match held.get(key) {
                Some(found) => assert_eq!(found, text, "{key}"),
                None => panic!("{key} has no entry; entries are {:?}", held.keys().collect::<Vec<_>>()),
            }
        }
        let kinds: Vec<K> = mind.index_entries(None).unwrap().iter().map(|entry| entry.id.kind).collect();
        assert!(!kinds.contains(&K::Instance) && !kinds.contains(&K::Stewardship), "{kinds:?}");
        assert!(kinds.contains(&K::Resolution), "the derived closures are indexed too");
    }

    /// The text is the document's alone: the same document admitted by another
    /// faculty at another clock, or closed by a later ruling, has the same
    /// text and digest. Only the ordinal is the receipt's.
    #[test]
    fn index_text_ignores_admission_facts_and_indexes_withdrawn_resolutions() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let q1 = id("question", "Q1");
        let before = mind.index_entries(Some(&[r(K::Question, &q1)])).unwrap();

        let mut other = seeded();
        committed(admit_as(&mut other, Faculty::Soul, vec![question("Q1", &["A", "B"], "A")]));
        let mut later = ruling("R1");
        later.answers = Some(s(&q1));
        later.choice = Some(l("A"));
        committed(admit(&mut other, vec![D::Ruling(later)]));
        let after = other.index_entries(Some(&[r(K::Question, &q1)])).unwrap();
        assert_eq!(before, after, "a closed question is the same entry as an open one");
        assert_eq!(before[0].ordinal, 2);

        let closure = id("resolution", "question.Q1.n1");
        committed(admit(&mut other, vec![resolution(r(K::Resolution, &closure), withdrawn())]));
        let withdrawn_id = id("resolution", "resolution.question.Q1.n1.n1");
        let entry = other.index_entries(Some(&[r(K::Resolution, &withdrawn_id)])).unwrap();
        assert_eq!(entry.len(), 1, "a withdrawn resolution is indexed");
        assert_eq!(entry[0].text, "Resolved.");
        assert_eq!(entry[0].root, CAMPAIGN);
        assert_eq!(entry[0].ordinal, 4);
    }

    #[test]
    fn the_digest_is_sha256_of_the_text_and_a_missing_document_is_refused() {
        let mind = seeded();
        let campaign_ref = r(K::Campaign, &id("campaign", "self"));
        let entry = &mind.index_entries(Some(std::slice::from_ref(&campaign_ref))).unwrap()[0];
        assert_eq!(entry.text_sha256, "49c7e65adaad4bd34261300e9c5575847357b8f269508c9b2a46ec2aa75cc019");
        assert_eq!((entry.root.as_str(), entry.ordinal), (CAMPAIGN, 1));
        let absent = r(K::Question, &id("question", "Q9"));
        assert!(matches!(mind.index_entries(Some(&[absent])), Err(MindRefusal::Unavailable { .. })));
        // The identity and the stewardship carry no text: the seeded mind's
        // indexable documents are its campaign and its target.
        let all = mind.index_entries(None).unwrap();
        assert_eq!(all.len(), 2, "{all:?}");
    }

    /// The bound is bytes, cut at a character boundary: three-byte characters
    /// put the byte limit inside one.
    #[test]
    fn text_is_bounded_at_a_character_boundary() {
        let wide: String = format!("{}x", "\u{20ac}".repeat(333));
        assert_eq!(wide.len(), 1000);
        let mut spec = cut_spec("1", 1);
        spec.adds = (0..40).map(|_| wide.as_str().into()).collect();
        let text = index_text(&D::CutSpec(spec)).unwrap();
        // The title and its newline, then full lines of 1,001 bytes, then part
        // of the next one, cut back to the last whole character.
        let head = "A cut\n".len();
        let stride = 1001;
        let full_lines = (INDEX_TEXT_MAX_BYTES - head) / stride;
        let consumed = head + full_lines * stride;
        let inside = INDEX_TEXT_MAX_BYTES - consumed;
        assert_ne!(inside % 3, 0, "the limit falls inside a character");
        assert_eq!(text.len(), consumed + inside - inside % 3);
        assert!(text.ends_with('\u{20ac}'));
    }
}
