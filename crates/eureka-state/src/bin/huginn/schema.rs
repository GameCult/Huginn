//! `huginn schema`: the published schema of every document kind and of a
//! selection, compiled in, so the schema an agent reads is the schema of the
//! binary it runs.

use crate::trouble::Trouble;

/// CultLib's published shape of a selection, vendored because the selection
/// type carries no schema of its own.
const SELECTION: &str = include_str!("../../../schemas/cultnet.selection.schema.json");

macro_rules! kinds {
    ($($name:ident),* $(,)?) => {
        /// Each kind's file in `schemas/cultnet`, named by `PipelineKind::name`.
        const KINDS: &[(&str, &str)] = &[$((
            stringify!($name),
            include_str!(concat!("../../../../../schemas/cultnet/epiphany.pipeline.", stringify!($name), ".v2.schema.json")),
        )),*];
    };
}

kinds!(
    campaign, target, question, ruling, cut_spec, cut_report, verdict, finding, follow_up, resolution, instance, stewardship,
    hand_off, run,
);

/// The schema named, or the names when none is: the exit code is 2 for a name
/// that is neither a kind nor `selection`.
pub fn text(name: Option<&str>) -> Result<String, Trouble> {
    match name {
        None => Ok(KINDS.iter().map(|(kind, _)| *kind).chain(["selection"]).map(|kind| format!("{kind}\n")).collect()),
        Some("selection") => Ok(SELECTION.to_string()),
        Some(name) => KINDS
            .iter()
            .find(|(kind, _)| *kind == name)
            .map(|(_, schema)| (*schema).to_string())
            .ok_or_else(|| Trouble::invalid("no such schema; `huginn schema` lists the names")),
    }
}

#[cfg(test)]
mod tests {
    use huginn_mind::eureka_pipeline::PipelineKind;

    use super::*;

    #[test]
    fn every_kind_has_exactly_its_schema() {
        let names: Vec<&str> = KINDS.iter().map(|(kind, _)| *kind).collect();
        let kinds: Vec<&str> = PipelineKind::ALL.iter().map(|kind| kind.name()).collect();
        assert_eq!(names, kinds);
    }
}
