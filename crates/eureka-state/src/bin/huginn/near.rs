//! The one neighbour query of the CLI: the in-force documents of some schemas
//! nearest a text, minus the ones the caller already holds. `admit_hold` asks
//! it before an admission; recall dispatch asks it next. It holds no client
//! and no policy beyond the rule here: the call is the caller's, so each
//! caller brings its own deadline.

use cultnet_rs::{PROJECTION_DOCUMENT, Selection};
use huginn_mind::eureka_pipeline::Line;
use huginn_mind::{HuginnMindResponse, PipelineDocumentView, PipelinePageItems, PipelineSelectionPage, SemanticQuery};
use serde_json::json;

use crate::trouble::Trouble;

/// The query text is cut here, at a character boundary.
const TEXT_MAX_BYTES: usize = 1000;

/// The in-force documents of `schemas` nearest `text`, as (score, document)
/// pairs: `excluded` ids dropped, at most `k` kept, nearest first as the mind
/// ranked them. A score the answer does not carry is 0. The mind is asked for
/// `k` plus the excluded count, so exclusions cannot leave fewer than `k`.
pub fn neighbours(
    call: impl FnOnce(Selection, SemanticQuery) -> Result<HuginnMindResponse, Trouble>,
    text: &str,
    schemas: &[&str],
    excluded: &[String],
    k: usize,
) -> Result<Vec<(f32, PipelineDocumentView)>, Trouble> {
    let selection = crate::door::selection_value(&json!({
        "schemas": schemas,
        "projection": PROJECTION_DOCUMENT,
        "fields": [{ "index": "in_force", "op": "any_of", "values": ["true"] }],
    }))?;
    let top_k = u32::try_from(k + excluded.len()).unwrap_or(u32::MAX);
    let semantic = SemanticQuery { text: Line(cut(text, TEXT_MAX_BYTES).to_string()), top_k };
    let PipelineSelectionPage { items, scores, .. } = match call(selection, semantic)? {
        HuginnMindResponse::Query(page) => page,
        HuginnMindResponse::Refused(refusal) => return Err(Trouble::Internal(format!("the mind refused the query: {refusal:?}"))),
        _ => return Err(Trouble::Internal("the mind answered a query with something else".to_string())),
    };
    let PipelinePageItems::Documents(views) = items else {
        return Err(Trouble::Internal("a document query answered headers".to_string()));
    };
    let scores = scores.unwrap_or_default();
    Ok(views
        .into_iter()
        .enumerate()
        .filter(|(_, view)| !excluded.contains(&view.id.id.0))
        .map(|(index, view)| (scores.get(index).copied().unwrap_or(0.0), view))
        .take(k)
        .collect())
}

/// `text` cut to at most `max` bytes, backing off to a character boundary.
fn cut(text: &str, max: usize) -> &str {
    &text[..text.floor_char_boundary(max)]
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use huginn_mind::eureka_pipeline::{Date, PipelineDocument, PipelineInstance, PipelineRef, Short, Slug, pipeline_key};
    use huginn_mind::{AdmissionFacts, Faculty, PipelineProvenance, PipelineStatus};

    use super::*;

    fn view(name: &str) -> PipelineDocumentView {
        let document = PipelineDocument::Instance(PipelineInstance {
            instance: Slug(name.into()),
            display_name: Short(name.into()),
            created_at: Date("2026-10-10".into()),
            host: Short(name.into()),
        });
        PipelineDocumentView {
            id: PipelineRef { kind: document.kind(), id: Short(pipeline_key(&document).unwrap()) },
            document,
            admission: AdmissionFacts {
                receipt_id: "r".into(),
                admitted_at: "2026-10-10T00:00:00Z".into(),
                provenance: PipelineProvenance {
                    faculty: Faculty::Hands,
                    agent: Short("a".into()),
                    session: Short("s".into()),
                    tool: Short("huginn".into()),
                    imported_from: None,
                },
                ordinal: 1,
            },
            status: PipelineStatus::InForce,
        }
    }

    fn page(names: &[&str], scores: Option<Vec<f32>>) -> HuginnMindResponse {
        HuginnMindResponse::Query(PipelineSelectionPage {
            matched: 0,
            as_of: 1,
            next: None,
            items: PipelinePageItems::Documents(names.iter().map(|name| view(name)).collect()),
            edges: None,
            scores,
        })
    }

    #[test]
    fn the_mind_is_asked_for_k_plus_the_excluded_and_the_cut_keeps_k_nearest_first() {
        let asked = RefCell::new(None);
        let found = neighbours(
            |selection, semantic| {
                *asked.borrow_mut() = Some((selection, semantic));
                Ok(page(&["a", "b", "c", "d", "e"], Some(vec![0.9, 0.8, 0.7, 0.6, 0.5])))
            },
            "words",
            &["x:y", "z:w"],
            &[view("b").id.id.0, view("d").id.id.0],
            2,
        )
        .unwrap();
        let (selection, semantic) = asked.into_inner().unwrap();
        assert_eq!((semantic.text.0.as_str(), semantic.top_k), ("words", 4));
        assert_eq!(selection.schemas, Some(vec!["x:y".to_string(), "z:w".to_string()]));
        assert_eq!(selection.projection, "document");
        let names: Vec<(f32, String)> = found.iter().map(|(score, view)| (*score, view.id.id.0.clone())).collect();
        assert_eq!(names, [(0.9, view("a").id.id.0), (0.7, view("c").id.id.0)]);
    }

    #[test]
    fn a_refusal_or_another_answer_is_a_trouble_that_says_which() {
        let refused = neighbours(|_, _| Ok(HuginnMindResponse::Refused(huginn_mind::MindRefusal::MissingIdentity)), "t", &["s"], &[], 1);
        assert!(matches!(&refused, Err(Trouble::Internal(detail)) if detail.contains("refused the query") && detail.contains("MissingIdentity")), "{refused:?}");
        let other = neighbours(|_, _| Ok(HuginnMindResponse::Persona(None)), "t", &["s"], &[], 1);
        assert!(matches!(&other, Err(Trouble::Internal(detail)) if detail.contains("something else")), "{other:?}");
    }

    #[test]
    fn a_missing_score_is_zero() {
        let found = neighbours(|_, _| Ok(page(&["a", "b"], Some(vec![0.5]))), "t", &["s"], &[], 5).unwrap();
        assert_eq!(found.iter().map(|(score, _)| *score).collect::<Vec<_>>(), [0.5, 0.0]);
        let found = neighbours(|_, _| Ok(page(&["a"], None)), "t", &["s"], &[], 5).unwrap();
        assert_eq!(found[0].0, 0.0);
    }

    #[test]
    fn the_text_is_cut_to_a_thousand_bytes_at_a_character_boundary() {
        let sent = RefCell::new(String::new());
        let ask = |text: &str| {
            neighbours(
                |_, semantic| {
                    *sent.borrow_mut() = semantic.text.0;
                    Ok(page(&[], None))
                },
                text,
                &["s"],
                &[],
                1,
            )
            .unwrap();
            sent.borrow().clone()
        };
        assert_eq!(ask(&"a".repeat(1000)).len(), 1000);
        assert_eq!(ask(&"a".repeat(1001)).len(), 1000);
        assert_eq!(ask("short"), "short");
        // 999 bytes then a 3-byte character straddling the limit: it is dropped whole.
        let straddling = format!("{}€{}", "a".repeat(999), "b");
        assert_eq!(ask(&straddling), "a".repeat(999));
    }
}
