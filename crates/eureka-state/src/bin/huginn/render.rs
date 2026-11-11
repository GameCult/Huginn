//! Text for stdout. Display only: nothing here decides an answer.

use std::fmt::Write;

use huginn_mind::eureka_pipeline::{PipelineResolution, ResolutionOutcome};
use huginn_mind::{ChatHits, ChatWindow, SourceHits, IndexStatus, PersonaStatus, PipelineDocumentSummary, PipelineDocumentView, PipelineStatus, PipelineStatusSummary};
use serde::Serialize;
use serde_json::Value;

/// A header line is cut here, so a page of them stays one screen per dozen.
const HEADER_MAX_CHARS: usize = 240;

/// The variant or string a serde-tagged value is spelled as: `Hands` for a
/// faculty, `Fixed` for an outcome.
pub fn name_of<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        Ok(Value::Object(fields)) => fields.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

/// The persona door's state as one line: no value of the document, only the
/// vocabulary the door answers in.
pub fn persona_state(persona: &PersonaStatus) -> String {
    match persona {
        PersonaStatus::Absent => "absent".into(),
        PersonaStatus::Stored { updated_at } => format!("stored updatedAt={updated_at}"),
        PersonaStatus::Unreadable => "unreadable".into(),
    }
}

/// An index status as one token and its numbers: `Behind pending=3`.
pub fn index_state(index: &IndexStatus) -> String {
    let mut text = name_of(index);
    if let Ok(Value::Object(variant)) = serde_json::to_value(index)
        && let Some(Value::Object(fields)) = variant.values().next()
    {
        for (key, value) in fields {
            if let Some(shown) = scalar(value) {
                let _ = write!(text, " {key}={shown}");
            }
        }
    }
    text
}

/// One document as `view` prints it: id and status, the admission facts, then
/// the `{kind, value}` document as pretty JSON. `raw` is the JSON alone.
pub fn view(view: &PipelineDocumentView, raw: bool) -> String {
    let document = serde_json::to_string_pretty(&view.document).unwrap_or_default();
    if raw {
        return document;
    }
    let id = &view.id.id.0;
    let status = match &view.status {
        PipelineStatus::InForce => "InForce".to_string(),
        PipelineStatus::Resolved { resolution, record: PipelineResolution { outcome: ResolutionOutcome::Moved { to }, .. } } => {
            format!("Resolved Moved to {} by {}", to.0, resolution.id.0)
        }
        PipelineStatus::Resolved { resolution, record } => format!("Resolved {} by {}", name_of(&record.outcome), resolution.id.0),
    };
    let admission = &view.admission;
    let who = &admission.provenance;
    let imported = who.imported_from.as_ref().map_or_else(String::new, |source| {
        format!(
            "\nimported from {} ordinal {} receipt {} ({}/{} session {})",
            source.instance.0,
            source.ordinal,
            source.receipt_id.0,
            name_of(&source.faculty),
            source.agent.0,
            source.session.0,
        )
    });
    format!(
        "{id} {status}\nadmitted {} ordinal {} {}/{} session {} tool {} receipt {}{imported}\n{document}",
        admission.admitted_at,
        admission.ordinal,
        name_of(&who.faculty),
        who.agent.0,
        who.session.0,
        who.tool.0,
        admission.receipt_id,
    )
}

/// One header as a line: `<id> <status> <faculty> <title or label> k=v...`
/// over the header's scalar facts.
pub fn header(item: &PipelineDocumentSummary) -> String {
    let status = match &item.status {
        PipelineStatusSummary::InForce => "InForce".to_string(),
        PipelineStatusSummary::Resolved { outcome, .. } => format!("Resolved:{}", name_of(outcome)),
    };
    let mut line = format!("{} {status} {}", item.id.id.0, name_of(&item.admission.provenance.faculty));
    if let Ok(facts) = serde_json::to_value(&item.facts)
        && let Some(Value::Object(facts)) = facts.get("value")
    {
        if let Some(Value::String(name)) = facts.get("title").or_else(|| facts.get("label")) {
            let _ = write!(line, " {name}");
        }
        for (key, value) in facts {
            if key == "authority"
                && let Some(Value::Object(steward)) = value.get("Steward")
            {
                // Her words stay out of the header: it names who decided and under what.
                let _ = write!(line, " authority=Steward");
                for field in ["steward", "repo"] {
                    if let Some(shown) = steward.get(field).and_then(scalar) {
                        let _ = write!(line, " {field}={shown}");
                    }
                }
            } else if key != "title"
                && key != "label"
                && let Some(shown) = scalar(value)
            {
                let _ = write!(line, " {key}={shown}");
            }
        }
    }
    line.chars().take(HEADER_MAX_CHARS).collect()
}

/// Source hits as the CLI prints them: per hit a citation line,
/// `repo:path:start-end @commit7 score`, then the chunk's text, and a blank line
/// between hits. Nothing for no hits.
pub fn source_hits(found: &SourceHits) -> String {
    found
        .hits
        .iter()
        .map(|hit| {
            format!(
                "{}:{}:{}-{} @{} {:.3}
{}
",
                hit.repo,
                hit.path,
                hit.line_start,
                hit.line_end,
                &hit.commit[..hit.commit.len().min(7)],
                hit.score,
                hit.text
            )
        })
        .collect::<Vec<_>>()
        .join("
")
}

/// Chat hits as the CLI prints them. Per hit the organ's own header line,
/// `#alias (!room) first..last time..time score` (the alias and the score only
/// when there are some), then one `time sender event_id body` line per message,
/// the body as the room held it with line breaks written `\n`: quoted data
/// under our header, never a message framed as if sent to the reader. A blank line between hits; a
/// truncated listing ends with the `--after` that continues it.
pub fn chat_hits(found: &ChatHits) -> String {
    let mut out: Vec<String> = found
        .hits
        .iter()
        .map(|hit| {
            let room = hit.room_alias.as_ref().map_or_else(|| hit.room_id.clone(), |alias| format!("{alias} ({})", hit.room_id));
            let score = hit.score.map_or_else(String::new, |score| format!(" {score:.3}"));
            let mut text = format!(
                "{room} {}..{} {}..{}{score}\n",
                hit.first_event_id,
                hit.last_event_id,
                crate::chat::shown(hit.ts_start_ms),
                crate::chat::shown(hit.ts_end_ms)
            );
            for message in &hit.messages {
                let _ = writeln!(text, "{} {} {} {}", crate::chat::shown(message.ts_ms), message.sender, message.event_id, one_line(&message.body));
            }
            text
        })
        .collect();
    if found.truncated
        && let Some(last) = found.hits.last()
    {
        out.push(format!("truncated: more follow, continue with --after {}\n", crate::chat::shown(last.ts_end_ms + 1)));
    }
    out.join("\n")
}

/// A message body on one line: a line break is written `\n` (and a carriage
/// return `\r`), so a body cannot start a line of its own. `--json` has the
/// body exactly.
fn one_line(body: &str) -> String {
    body.replace('\r', "\\r").replace('\n', "\\n")
}

/// A chat window as the CLI prints it: `#alias (!room)` (the id alone with no
/// alias), then one `time sender event_id body` line per message, the anchor
/// marked `>` and the rest indented to match; a line says when messages lie
/// beyond either end.
pub fn chat_window(found: &ChatWindow) -> String {
    let room = found.room_alias.as_ref().map_or_else(|| found.room_id.clone(), |alias| format!("{alias} ({})", found.room_id));
    let mut out = format!("{room}\n");
    if found.more_before {
        out.push_str("  (earlier messages exist)\n");
    }
    for message in &found.messages {
        let mark = if message.event_id == found.anchor_event_id { '>' } else { ' ' };
        let _ = writeln!(out, "{mark} {} {} {} {}", crate::chat::shown(message.ts_ms), message.sender, message.event_id, one_line(&message.body));
    }
    if found.more_after {
        out.push_str("  (later messages exist)\n");
    }
    out.pop();
    out
}

/// A fact as one token: a string, a number, a flag, a reference's id, or a
/// list of those joined by commas. Anything deeper is not a header fact.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Array(items) => {
            let shown: Vec<String> = items.iter().filter_map(scalar).collect();
            if shown.is_empty() { None } else { Some(shown.join(",")) }
        }
        Value::Object(fields) => match fields.get("id") {
            Some(Value::String(id)) => Some(id.clone()),
            _ => None,
        },
        Value::String(_) | Value::Null => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use huginn_mind::eureka_pipeline::{Date, PipelineDocument, PipelineInstance, PipelineKind, PipelineRef, Short, Slug};
    use huginn_mind::{AdmissionFacts, Faculty, ImportSource, PipelineProvenance};

    use super::*;

    fn viewed(imported_from: Option<ImportSource>, status: PipelineStatus) -> PipelineDocumentView {
        PipelineDocumentView {
            id: PipelineRef { kind: PipelineKind::Instance, id: Short("heimdall:instance:self".into()) },
            document: PipelineDocument::Instance(PipelineInstance {
                instance: Slug("heimdall".into()),
                display_name: Short("Heimdall".into()),
                created_at: Date("2026-10-10".into()),
                host: Short("yggdrasil".into()),
            }),
            admission: AdmissionFacts {
                receipt_id: "mind-commit-b".into(),
                admitted_at: "2026-11-08T12:00:00Z".into(),
                provenance: PipelineProvenance {
                    faculty: Faculty::SelfFaculty,
                    agent: Short("heimdall-self".into()),
                    session: Short("run-9".into()),
                    tool: Short("huginn".into()),
                    imported_from,
                },
                ordinal: 4,
            },
            status,
        }
    }

    /// A reader of an imported document sees where it came from and who admitted it there; a document
    /// admitted here prints no source line, and a moved one names its receiver.
    #[test]
    fn view_header_names_the_import_source() {
        let source = ImportSource {
            instance: Slug("eureka".into()),
            receipt_id: Short("mind-commit-a".into()),
            ordinal: 7331,
            committed_at: Short("2026-11-08T16:20:05Z".into()),
            faculty: Faculty::Imagination,
            agent: Short("imagination".into()),
            session: Short("eureka-self-1108".into()),
            tool: Short("huginn".into()),
            external: vec![],
        };
        let shown = view(&viewed(Some(source), PipelineStatus::InForce), false);
        assert!(
            shown.contains("
imported from eureka ordinal 7331 receipt mind-commit-a (Imagination/imagination session eureka-self-1108)
"),
            "{shown}"
        );
        assert!(!view(&viewed(None, PipelineStatus::InForce), false).contains("imported from"));

        let record = huginn_mind::eureka_pipeline::PipelineResolution {
            subject: PipelineRef { kind: PipelineKind::Campaign, id: Short("c:campaign:self".into()) },
            sequence: 1,
            outcome: ResolutionOutcome::Moved { to: Slug("heimdall".into()) },
            rationale: "Her repo.".into(),
            resolved_on: Date("2026-11-08".into()),
        };
        let resolution = PipelineRef { kind: PipelineKind::Resolution, id: Short("c:resolution:campaign.self.n1".into()) };
        let moved = view(&viewed(None, PipelineStatus::Resolved { resolution, record }), false);
        assert!(moved.starts_with("heimdall:instance:self Resolved Moved to heimdall by c:resolution:campaign.self.n1
"), "{moved}");
    }

    /// The citation line is what an agent copies: repo, path, the inclusive
    /// line range and the first seven of the commit.
    #[test]
    fn a_source_hit_prints_as_a_citation_then_its_text() {
        let hit = |path: &str, text: &str| huginn_mind::SourceHit {
            repo: "Huginn".into(),
            path: path.into(),
            commit: "0123456789abcdef".into(),
            line_start: 3,
            line_end: 9,
            score: 0.87654,
            text: text.into(),
        };
        let found = SourceHits {
            indexed_under: huginn_mind::SourceModel { name: "m".into(), digest: "d".into() },
            hits: vec![hit("a.rs", "one"), hit("b/c.rs", "two")],
        };
        assert_eq!(source_hits(&found), "Huginn:a.rs:3-9 @0123456 0.877
one

Huginn:b/c.rs:3-9 @0123456 0.877
two
");
        let empty = SourceHits { indexed_under: found.indexed_under.clone(), hits: vec![] };
        assert_eq!(source_hits(&empty), "");
        let json = serde_json::to_string(&found).unwrap();
        assert_eq!(serde_json::from_str::<SourceHits>(&json).unwrap(), found, "--json round-trips");
    }

    #[test]
    fn a_steward_ruling_header_names_the_decider_and_her_stewardship_but_not_her_words() {
        use huginn_mind::eureka_pipeline::{OrgRepo, PipelineRuling, RulingAuthority};
        let mut view = viewed(None, PipelineStatus::InForce);
        view.document = PipelineDocument::Ruling(PipelineRuling {
            campaign: Slug("c".into()),
            label: "R1".into(),
            title: "Zeros".into(),
            answers: None,
            choice: None,
            ruling: "Refuse them.".into(),
            operator_quote: None,
            ruled_on: Date("2026-11-11".into()),
            precedents: vec![],
            authority: RulingAuthority::Steward {
                steward: "@heimdall:chat".into(),
                repo: OrgRepo("GameCult/Bifrost".into()),
                words: "refuse leading zeros".into(),
                event: None,
            },
        });
        let line = header(&PipelineDocumentSummary::of(&view, &|_| None));
        assert!(line.contains("authority=Steward steward=@heimdall:chat repo=GameCult/Bifrost"), "{line}");
        assert!(!line.contains("zeros"), "{line}");
    }

    #[test]
    fn a_fact_is_one_token_or_nothing() {
        assert_eq!(scalar(&json!("")), None);
        assert_eq!(scalar(&json!("a b")), Some("a b".to_string()));
        assert_eq!(scalar(&json!(3)), Some("3".to_string()));
        assert_eq!(scalar(&json!(true)), Some("true".to_string()));
        assert_eq!(scalar(&json!(["a", "", "b"])), Some("a,b".to_string()));
        assert_eq!(scalar(&json!([])), None);
        assert_eq!(scalar(&json!(null)), None);
        assert_eq!(scalar(&json!({ "kind": "Ruling", "id": "c:ruling:R1" })), Some("c:ruling:R1".to_string()));
        assert_eq!(scalar(&json!({ "id": 3 })), None);
        assert_eq!(scalar(&json!({ "Withdrawn": { "reason": "moot" } })), None);
    }
}
