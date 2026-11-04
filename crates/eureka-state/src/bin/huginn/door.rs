//! The CLI's argument reading. The strict decode of agent-written JSON is
//! `eureka_state::door`, shared with the MCP server.

use std::io::Read;

use huginn_mind::eureka_pipeline::PipelineKind;

pub use eureka_state::door::{documents, selection, selection_value};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_come_from_the_middle_segment() {
        assert_eq!(kind_of("eureka-substrate:cut_spec:cut-x.r1").ok(), Some(PipelineKind::CutSpec));
        assert!(kind_of("eureka-substrate:nonsense:x").is_err());
        assert!(kind_of("two:segments").is_err());
        assert!(kind_of("campaign").is_err());
    }
}
