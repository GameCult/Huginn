//! Whether a chat delivery is for a Mind: the one rule the heartbeat's off-beat wake and beat score and the Persona's `to_you` and
//! stagger rank all call. A delivery addresses a Mind when its `m.mentions` holds her Matrix id (a tag), or its text holds one of her
//! names as a whole word. Her names are her own (handle, public name; case does not matter) and her projects': the names her
//! stewardship records list, matched exactly as written, or, when a record lists none, the repo's name matched exactly as the repo
//! spells it and, for a multi-word name, its split form matched in any case. A deterministic identity match on names, not a reading of meaning.

use eureka_pipeline::{OrgRepo, Short};

use crate::chat_client::{Basis, ChatDelivery, Form};

/// A word a message may call a Mind or her project by.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Name {
    pub(crate) text: String,
    /// Matched case-sensitively, as written; otherwise in any case.
    pub(crate) exact: bool,
}

/// Whom a delivery may be addressing: one Mind's Matrix id and her names.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Addressee {
    matrix_id: String,
    names: Vec<Name>,
}

impl Addressee {
    /// A Mind by her id and her own names, each matched in any case.
    pub(crate) fn new<'a>(matrix_id: &str, own_names: impl IntoIterator<Item = &'a str>) -> Self {
        Self { matrix_id: matrix_id.to_string(), names: own_names.into_iter().map(|text| Name { text: text.to_string(), exact: false }).collect() }
    }

    /// The same Mind, also addressed by her projects' names.
    pub(crate) fn with_projects(mut self, names: impl IntoIterator<Item = Name>) -> Self {
        self.names.extend(names);
        self
    }

    pub(crate) fn matrix_id(&self) -> &str {
        &self.matrix_id
    }
}

/// The part of a Matrix id between `@` and `:`, the name the chat calls a Mind by.
pub(crate) fn localpart(matrix_id: &str) -> &str {
    let id = matrix_id.strip_prefix('@').unwrap_or(matrix_id);
    id.split_once(':').map_or(id, |(local, _)| local)
}

/// Whether a delivery is for `who` by what it says: only a message can be (a notice or an edit never is, whatever else it says), and
/// then a tag of hers in its mentions or a word of hers in the text. What Matrix says about the Persona that polled (`Basis`) is not
/// here: it is a fact about that one Persona, not about every Mind in the table (`is_for`).
pub(crate) fn addresses(who: &Addressee, delivery: &ChatDelivery) -> bool {
    delivery.form == Form::Message && (delivery.mentions.contains(&who.matrix_id) || who.names.iter().any(|name| names_whole_word(&delivery.text, &name.text, name.exact)))
}

/// Whether a delivery the mouth gave `own` is for her: her DM (`is_direct`, which every mouth sends, so an old mouth's DM is hers too),
/// Matrix's word that it is a tag of hers or a reply to her, or what it says (`addresses`). The basis `Room` (`@room`) is for no one, and no basis is unproven, not "not for her": the names still decide.
pub(crate) fn is_for(own: &Addressee, delivery: &ChatDelivery) -> bool {
    (delivery.form == Form::Message && (delivery.is_direct || matches!(delivery.basis, Some(Basis::Direct | Basis::Tag | Basis::Reply)))) || addresses(own, delivery)
}

/// The chat names of one stewarded repo. Listed names are exactly those, each matched exactly as the steward wrote it (capitalization
/// tells a project from a common word). With none listed: the repo's name after the org, matched exactly as the repo spells it
/// (`Thing`, not `thing`), and that name split at `-`, `_`, `.` and where a lower-case letter or digit meets a capital, when the split
/// reads differently, matched in any case (a split phrase is not a common word).
pub(crate) fn project_names(repo: &OrgRepo, listed: &[Short]) -> Vec<Name> {
    if !listed.is_empty() {
        return listed.iter().map(|name| Name { text: name.0.clone(), exact: true }).collect();
    }
    let whole = repo.0.split_once('/').map_or(repo.0.as_str(), |(_, name)| name);
    let mut split = String::new();
    let mut previous: Option<char> = None;
    for ch in whole.chars() {
        if matches!(ch, '-' | '_' | '.') {
            split.push(' ');
        } else {
            if ch.is_uppercase() && previous.is_some_and(|before| before.is_lowercase() || before.is_ascii_digit()) {
                split.push(' ');
            }
            split.push(ch);
        }
        previous = Some(ch);
    }
    let split = split.to_lowercase();
    let mut names = vec![Name { text: whole.to_string(), exact: true }];
    if split != whole.to_lowercase() {
        names.push(Name { text: split, exact: false });
    }
    names
}

/// Whether `text` holds `name` as a whole word, in any case or exactly as written: the characters on either side are not letters or
/// digits. An identity match on a name, not a judgement of meaning.
pub(crate) fn names_whole_word(text: &str, name: &str, exact: bool) -> bool {
    let (text, name) = if exact { (text.to_string(), name.to_string()) } else { (text.to_lowercase(), name.to_lowercase()) };
    if name.is_empty() {
        return false;
    }
    // Every character offset, not every non-overlapping match: a name may overlap itself ("ab ab" in "xab ab ab").
    text.char_indices().any(|(start, _)| {
        text[start..].starts_with(&name)
            && !text[..start].chars().next_back().is_some_and(char::is_alphanumeric)
            && !text[start + name.len()..].chars().next().is_some_and(char::is_alphanumeric)
    })
}

#[cfg(test)]
mod tests;
