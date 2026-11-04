//! The text door: agent-written JSON becomes typed values here, strictly, and
//! nowhere else. The store's decode stays lenient (serde ignores unknown
//! keys), so a misspelled key would be dropped and the batch committed
//! without it. The door refuses instead: every key path present in the input
//! with a value and absent from the typed value's re-serialisation is named in
//! the refusal. Paths only, never values.
//!
//! Both consumers, the `huginn` CLI and the MCP server, admit through this
//! door and no other.

use std::fmt;

use cultnet_rs::Selection;
use huginn_mind::eureka_pipeline::PipelineDocument;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, error::Category};

/// Why the door did not open.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The input is not what the schema describes; the detail names where, never what.
    Invalid(String),
    /// The door's own fault.
    Internal(String),
}

impl Refusal {
    fn invalid(detail: impl Into<String>) -> Self {
        Self::Invalid(detail.into())
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Self::Invalid(detail) | Self::Internal(detail)) = self;
        f.write_str(detail)
    }
}

/// A batch: a JSON array of `{kind, value}` documents, or one such object.
pub fn documents(text: &str) -> Result<Vec<PipelineDocument>, Refusal> {
    let value = parse(text)?;
    let (items, typed): (&[Value], Vec<PipelineDocument>) = match &value {
        Value::Array(items) => (items.as_slice(), objects(items).and_then(|()| from_text(text, "documents"))?),
        Value::Object(_) => (std::slice::from_ref(&value), vec![from_text(text, "documents[0]")?]),
        _ => return Err(Refusal::invalid("the documents must be a JSON array of {kind, value} objects")),
    };
    let kept = reserialise(&typed, "documents")?;
    let mut lost = Vec::new();
    for (index, (given, held)) in items.iter().zip(&kept).enumerate() {
        dropped(given, held, &format!("documents[{index}]"), &mut lost);
    }
    refuse_lost(&lost)?;
    Ok(typed)
}

pub fn selection(text: &str) -> Result<Selection, Refusal> {
    let value = parse(text)?;
    if !value.is_object() {
        return Err(Refusal::invalid("selection must be a JSON object"));
    }
    let typed: Selection = from_text(text, "selection")?;
    let kept = reserialise(std::slice::from_ref(&typed), "selection")?;
    let mut lost = Vec::new();
    dropped(&value, &kept[0], "selection", &mut lost);
    refuse_lost(&lost)?;
    Ok(typed)
}

pub fn selection_value(value: &Value) -> Result<Selection, Refusal> {
    selection(&value.to_string())
}

fn parse(text: &str) -> Result<Value, Refusal> {
    serde_json::from_str(text).map_err(|error| Refusal::invalid(format!("not JSON: {}", place(&error))))
}

/// Where and what kind of failure a decode hit, never what the input said:
/// serde's own message quotes the offending value whole.
fn place(error: &serde_json::Error) -> String {
    let category = match error.classify() {
        Category::Io => "unreadable input",
        Category::Syntax => "syntax",
        Category::Data => "a value of the wrong type, an unknown variant or a missing field",
        Category::Eof => "unexpected end",
    };
    format!("{category} at line {} column {}", error.line(), error.column())
}

/// Serde would also read a struct from an array, positionally, so only an
/// object is a document or a selection.
fn objects(items: &[Value]) -> Result<(), Refusal> {
    match items.iter().position(|item| !item.is_object()) {
        None => Ok(()),
        Some(index) => Err(Refusal::invalid(format!("documents[{index}] must be a JSON object"))),
    }
}

/// The typed value of the text, decoded from the text itself so the failure
/// has a real line and column.
fn from_text<T: DeserializeOwned>(text: &str, at: &str) -> Result<T, Refusal> {
    serde_json::from_str(text).map_err(|error| Refusal::invalid(format!("{at} does not decode: {}", place(&error))))
}

/// What the typed values hold, as the JSON items to compare with the input.
fn reserialise<T: Serialize>(typed: &[T], at: &str) -> Result<Vec<Value>, Refusal> {
    typed
        .iter()
        .map(|item| serde_json::to_value(item).map_err(|_| Refusal::Internal(format!("{at} does not re-serialise"))))
        .collect()
}

fn refuse_lost(paths: &[String]) -> Result<(), Refusal> {
    if paths.is_empty() {
        Ok(())
    } else {
        Err(Refusal::invalid(format!("keys the schema does not know (`huginn schema` prints it): {}", paths.join(", "))))
    }
}

/// The paths of `input` that carry a value and are absent from `kept`.
fn dropped(input: &Value, kept: &Value, path: &str, out: &mut Vec<String>) {
    match (input, kept) {
        (Value::Object(given), Value::Object(held)) => {
            for (key, value) in given {
                let here = format!("{path}.{key}");
                match held.get(key) {
                    Some(held) => dropped(value, held, &here, out),
                    None if !empty(value) => out.push(here),
                    None => {}
                }
            }
        }
        (Value::Array(given), Value::Array(held)) => {
            for (index, (value, held)) in given.iter().zip(held).enumerate() {
                dropped(value, held, &format!("{path}[{index}]"), out);
            }
        }
        _ => {}
    }
}

/// A value whose loss changes nothing: the spellings of "not given".
fn empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn paths(input: &Value, kept: &Value) -> Vec<String> {
        let mut out = Vec::new();
        dropped(input, kept, "d", &mut out);
        out.sort();
        out
    }

    #[test]
    fn a_dropped_key_with_a_value_is_named_by_path() {
        let input = json!({ "a": { "b": [ { "c": 1, "typo": "CANARY" } ] }, "top": 7 });
        let kept = json!({ "a": { "b": [ { "c": 1 } ] } });
        assert_eq!(paths(&input, &kept), vec!["d.a.b[0].typo".to_string(), "d.top".to_string()]);
    }

    #[test]
    fn a_dropped_key_without_a_value_is_not_a_loss() {
        let input = json!({ "n": null, "s": "", "l": [], "o": {}, "zero": 0, "no": false });
        assert_eq!(paths(&input, &json!({})), vec!["d.no".to_string(), "d.zero".to_string()]);
    }

}
