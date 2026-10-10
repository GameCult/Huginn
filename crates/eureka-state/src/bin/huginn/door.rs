//! The text door: agent-written JSON becomes typed values here, strictly, and
//! nowhere else. The store's decode stays lenient (serde ignores unknown
//! keys), so a misspelled key would be dropped and the batch committed
//! without it. The door refuses instead: every key path present in the input
//! with a value and absent from the typed value's re-serialisation is named in
//! the refusal. Paths only, never values.

use std::io::Read;

use cultnet_rs::Selection;
use huginn_mind::eureka_pipeline::{PipelineDocument, PipelineKind};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, error::Category};

use crate::trouble::Trouble;

/// The text of an argument that is a literal, `@<file>` or `-` for stdin.
pub fn argument(arg: &str) -> Result<String, Trouble> {
    match arg.strip_prefix('@') {
        Some(path) => file(path),
        None if arg == "-" => stdin(),
        None => Ok(arg.to_string()),
    }
}

/// The text of a file argument, or stdin when it is `-`.
pub fn file_or_stdin(arg: &str) -> Result<String, Trouble> {
    if arg == "-" { stdin() } else { file(arg) }
}

fn file(path: &str) -> Result<String, Trouble> {
    std::fs::read_to_string(path).map_err(|error| Trouble::invalid(format!("cannot read {path}: {}", error.kind())))
}

fn stdin() -> Result<String, Trouble> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|error| Trouble::invalid(format!("cannot read stdin: {}", error.kind())))?;
    Ok(text)
}

/// A batch: a JSON array of `{kind, value}` documents, or one such object.
pub fn documents(text: &str) -> Result<Vec<PipelineDocument>, Trouble> {
    let value = parse(text)?;
    let (items, typed): (&[Value], Vec<PipelineDocument>) = match &value {
        Value::Array(items) => (items.as_slice(), objects(items).and_then(|()| from_text(text, "documents"))?),
        Value::Object(_) => (std::slice::from_ref(&value), vec![from_text(text, "documents[0]")?]),
        _ => return Err(Trouble::invalid("the documents must be a JSON array of {kind, value} objects")),
    };
    let kept = reserialise(&typed, "documents")?;
    let mut lost = Vec::new();
    for (index, (given, held)) in items.iter().zip(&kept).enumerate() {
        dropped(given, held, &format!("documents[{index}]"), &mut lost);
    }
    refuse_lost(&lost)?;
    Ok(typed)
}

pub fn selection(text: &str) -> Result<Selection, Trouble> {
    let value = parse(text)?;
    if !value.is_object() {
        return Err(Trouble::invalid("selection must be a JSON object"));
    }
    let typed: Selection = from_text(text, "selection")?;
    let kept = reserialise(std::slice::from_ref(&typed), "selection")?;
    let mut lost = Vec::new();
    dropped(&value, &kept[0], "selection", &mut lost);
    refuse_lost(&lost)?;
    Ok(typed)
}

pub fn selection_value(value: &Value) -> Result<Selection, Trouble> {
    selection(&value.to_string())
}

/// The kind a pipeline id names in its middle segment (`<root>:<kind>:<local>`).
pub fn kind_of(id: &str) -> Result<PipelineKind, Trouble> {
    let mut segments = id.splitn(3, ':');
    let (_, Some(name), Some(_)) = (segments.next(), segments.next(), segments.next()) else {
        return Err(Trouble::invalid("a pipeline id is <root>:<kind>:<local>"));
    };
    PipelineKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.name() == name)
        .ok_or_else(|| Trouble::invalid("the id's kind segment is not a pipeline kind (`huginn schema` lists them)"))
}

fn parse(text: &str) -> Result<Value, Trouble> {
    serde_json::from_str(text).map_err(|error| Trouble::invalid(format!("not JSON: {}", place(&error))))
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
fn objects(items: &[Value]) -> Result<(), Trouble> {
    match items.iter().position(|item| !item.is_object()) {
        None => Ok(()),
        Some(index) => Err(Trouble::invalid(format!("documents[{index}] must be a JSON object"))),
    }
}

/// The typed value of the text, decoded from the text itself so the failure
/// has a real line and column.
fn from_text<T: DeserializeOwned>(text: &str, at: &str) -> Result<T, Trouble> {
    serde_json::from_str(text).map_err(|error| Trouble::invalid(format!("{at} does not decode: {}", place(&error))))
}

/// What the typed values hold, as the JSON items to compare with the input.
fn reserialise<T: Serialize>(typed: &[T], at: &str) -> Result<Vec<Value>, Trouble> {
    typed
        .iter()
        .map(|item| serde_json::to_value(item).map_err(|_| Trouble::Internal(format!("{at} does not re-serialise"))))
        .collect()
}

fn refuse_lost(paths: &[String]) -> Result<(), Trouble> {
    if paths.is_empty() {
        Ok(())
    } else {
        Err(Trouble::invalid(format!("keys the schema does not know (`huginn schema` prints it): {}", paths.join(", "))))
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

    #[test]
    fn kinds_come_from_the_middle_segment() {
        assert_eq!(kind_of("eureka-substrate:cut_spec:cut-x.r1").ok(), Some(PipelineKind::CutSpec));
        assert!(kind_of("eureka-substrate:nonsense:x").is_err());
        assert!(kind_of("two:segments").is_err());
        assert!(kind_of("campaign").is_err());
    }
}
