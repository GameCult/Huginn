//! Text for stdout. Display only: nothing here decides an answer.

use std::fmt::Write;

use huginn_mind::{IndexStatus, PersonaStatus, PipelineDocumentSummary, PipelineDocumentView, PipelineStatus, PipelineStatusSummary};
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
        PipelineStatus::Resolved { resolution, record } => format!("Resolved {} by {}", name_of(&record.outcome), resolution.id.0),
    };
    let admission = &view.admission;
    let who = &admission.provenance;
    format!(
        "{id} {status}\nadmitted {} ordinal {} {}/{} session {} tool {} receipt {}\n{document}",
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
            if key != "title"
                && key != "label"
                && let Some(shown) = scalar(value)
            {
                let _ = write!(line, " {key}={shown}");
            }
        }
    }
    line.chars().take(HEADER_MAX_CHARS).collect()
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

    use super::*;

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
