//! Which joined rooms and which senders are in the chat corpus. The only code
//! that decides it, and the only file outside tests that names the bridged
//! platform whose content is refused.
//!
//! Scope (ruling `gamecult-chat:ruling:chat-index-minds-rooms`): the rooms the
//! reader user has joined, minus this exclusion. A room's join rule is read by
//! nobody (`RoomFacts` has no field for it): a joined invite-only room qualifies
//! like any other. The reader roster is the first lock; this file is the second.

use std::collections::BTreeSet;

use super::chat::RoomEvent;

/// The estate rooms by canonical alias, ASCII case-insensitive. Code owns the
/// aliases and the rule; the unit supplies the room ids (`Estate`), and either alone excludes.
pub const ESTATE_ALIASES: [&str; 3] = ["#users:gamecult.org", "#members:gamecult.org", "#patrons:gamecult.org"];

/// What the server says about a room, as far as admission needs it. The Matrix
/// client fills it from the room's state and decides nothing. It has no `Default`: a
/// room whose state could not be read is a port error, never a plain room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomFacts {
    pub room_id: String,
    pub alias: Option<String>,
    pub alt_aliases: Vec<String>,
    pub name: Option<String>,
    /// An `m.room.encryption` state event exists.
    pub encrypted: bool,
    /// Any `m.room.member` in the state has `is_direct` true.
    pub direct: bool,
    /// The state type (`m.bridge` or `uk.half-shot.bridge`) of a bridge event, when the room has one.
    pub bridge: Option<String>,
}

/// The estate room ids the caller knows (they exist only on the live server): exactly one per
/// estate alias, so a job that was handed none, or too few, cannot be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estate {
    ids: BTreeSet<String>,
}

impl Estate {
    pub fn new(ids: BTreeSet<String>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            ids.len() == ESTATE_ALIASES.len() && ids.iter().all(|id| id.starts_with('!')),
            "the estate is exactly {} room ids ({}), got {}",
            ESTATE_ALIASES.len(),
            ESTATE_ALIASES.join(", "),
            ids.len()
        );
        Ok(Self { ids })
    }

    /// Whether `room_id` is a named estate room. Decided before the room's state is read.
    pub fn holds_id(&self, room_id: &str) -> bool {
        self.ids.contains(room_id)
    }
}

/// Why a joined room, or an event in it, is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Estate,
    Direct,
    Encrypted,
    Bridged(String),
    DiscordSender,
    DiscordContent,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Estate => formatter.write_str("estate room"),
            Self::Direct => formatter.write_str("direct room"),
            Self::Encrypted => formatter.write_str("encrypted room"),
            Self::Bridged(state_type) => write!(formatter, "bridged room ({state_type})"),
            Self::DiscordSender => formatter.write_str("discord sender"),
            Self::DiscordContent => formatter.write_str("discord content"),
        }
    }
}

/// Whether a joined room may be read at all.
pub fn admit_room(facts: &RoomFacts, estate: &Estate) -> Result<(), Refusal> {
    let estate_alias = |alias: &str| ESTATE_ALIASES.iter().any(|estate| estate.eq_ignore_ascii_case(alias));
    if estate.holds_id(&facts.room_id) || facts.alias.iter().chain(&facts.alt_aliases).any(|alias| estate_alias(alias)) {
        return Err(Refusal::Estate);
    }
    if facts.direct {
        return Err(Refusal::Direct);
    }
    if facts.encrypted {
        return Err(Refusal::Encrypted);
    }
    if let Some(state_type) = &facts.bridge {
        return Err(Refusal::Bridged(state_type.clone()));
    }
    Ok(())
}

/// Whether an event refuses its whole room: a sender whose localpart, lowercased, starts with the
/// platform's name after any leading underscores, has it after an underscore (`mautrix_discord`,
/// `_bridge_discordbot`), or has it followed by `bot` or an underscore; or content with a top-level key
/// naming it. Deterministic identifier rules, not language reading; `discourse` is not it.
pub fn refuse_event(event: &RoomEvent) -> Option<Refusal> {
    let localpart = event.sender.trim_start_matches('@').split(':').next().unwrap_or("").to_ascii_lowercase();
    let name = ["disc", "ord"].concat();
    if localpart.trim_start_matches('_').starts_with(&name) || localpart.contains(&format!("_{name}")) || localpart.contains(&format!("{name}bot")) || localpart.contains(&format!("{name}_")) {
        return Some(Refusal::DiscordSender);
    }
    event.content_keys.iter().any(|key| key.to_ascii_lowercase().contains("discord")).then_some(Refusal::DiscordContent)
}

#[cfg(test)]
mod tests {
    use super::super::chat::EventKind;
    use super::*;

    fn facts(id: &str) -> RoomFacts {
        RoomFacts { room_id: id.into(), alias: None, alt_aliases: vec![], name: None, encrypted: false, direct: false, bridge: None }
    }

    fn estate(ids: &[&str]) -> Estate {
        Estate::new(ids.iter().chain(["!pad1:s", "!pad2:s", "!pad3:s"].iter()).take(3).map(|id| (*id).to_owned()).collect()).unwrap()
    }

    fn event(sender: &str, keys: &[&str]) -> RoomEvent {
        RoomEvent { event_id: "$e".into(), sender: sender.into(), ts_ms: 1, content_keys: keys.iter().map(|key| (*key).into()).collect(), kind: EventKind::Other }
    }

    #[test]
    fn a_direct_encrypted_or_bridged_room_is_refused_and_a_plain_room_is_admitted() {
        let estate = estate(&[]);
        assert_eq!(admit_room(&RoomFacts { direct: true, ..facts("!a:s") }, &estate), Err(Refusal::Direct));
        assert_eq!(admit_room(&RoomFacts { encrypted: true, ..facts("!a:s") }, &estate), Err(Refusal::Encrypted));
        for state_type in ["m.bridge", "uk.half-shot.bridge"] {
            assert_eq!(admit_room(&RoomFacts { bridge: Some(state_type.into()), ..facts("!a:s") }, &estate), Err(Refusal::Bridged(state_type.into())));
        }
        assert_eq!(admit_room(&RoomFacts { alias: Some("#swarm:gamecult.org".into()), name: Some("Swarm".into()), ..facts("!a:s") }, &estate), Ok(()));
    }

    #[test]
    fn the_estate_is_refused_by_id_and_by_alias_in_any_case_and_a_lookalike_is_not() {
        let named = estate(&["!ids:s"]);
        assert!(named.holds_id("!ids:s") && !named.holds_id("!other:s"));
        assert_eq!(admit_room(&facts("!ids:s"), &named), Err(Refusal::Estate));
        for alias in ESTATE_ALIASES.iter().map(|alias| alias.to_string()).chain(["#USERS:Gamecult.Org".to_owned()]) {
            assert_eq!(admit_room(&RoomFacts { alias: Some(alias.clone()), ..facts("!x:s") }, &estate(&[])), Err(Refusal::Estate), "{alias}");
            assert_eq!(admit_room(&RoomFacts { alt_aliases: vec!["#a:s".into(), alias.clone()], ..facts("!x:s") }, &estate(&[])), Err(Refusal::Estate), "alt {alias}");
        }
        for lookalike in ["#users-guide:gamecult.org", "#patrons-club:gamecult.org", "#users:gamecult.org.evil", "#users:other.org"] {
            assert_eq!(admit_room(&RoomFacts { alias: Some(lookalike.into()), ..facts("!x:s") }, &estate(&[])), Ok(()), "{lookalike}");
        }
    }

    #[test]
    fn a_bridged_platform_sender_or_content_key_refuses_and_a_lookalike_does_not() {
        for sender in ["@discord_123:s", "@_discord_9:s", "@__DiscordBot:s", "@discordbot:s", "@mautrix_discord:s", "@_bridge_discordbot:s", "@a_discord:s"] {
            assert_eq!(refuse_event(&event(sender, &[])), Some(Refusal::DiscordSender), "{sender}");
        }
        for key in ["fi.mau.discord", "uk.half-shot.discord", "X.DISCORD.y"] {
            assert_eq!(refuse_event(&event("@a:s", &["body", key])), Some(Refusal::DiscordContent), "{key}");
        }
        for sender in ["@discourse:s", "@disco:s", "@eureka:s", "@x:discord.com"] {
            assert_eq!(refuse_event(&event(sender, &["discourse", "body"])), None, "{sender}");
        }
    }

    #[test]
    fn the_estate_needs_exactly_its_room_ids() {
        for ids in [vec![], vec!["!a:s"], vec!["!a:s", "!b:s", "!c:s", "!d:s"], vec!["!a:s", "!b:s", "notaroom"]] {
            assert!(Estate::new(ids.iter().map(|id| (*id).to_owned()).collect()).is_err(), "{ids:?}");
        }
        assert!(Estate::new(["!a:s", "!b:s", "!c:s"].map(str::to_owned).into()).is_ok());
    }

    #[test]
    fn a_room_facts_carries_no_join_rule() {
        let source = include_str!("room_scope.rs");
        let code = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(!code.contains(&["join", "_rule"].concat()));
    }
}
