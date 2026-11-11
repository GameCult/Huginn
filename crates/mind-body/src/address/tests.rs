//! The one addressing rule: a tag, or a name as a whole word; a listed name exactly as written, a derived one-word repo name as the repo spells it, a split phrase or a Mind's own in any case.

use super::*;
use crate::chat_client::{Basis, Form};

fn said(text: &str, mentions: &[&str]) -> ChatDelivery {
    ChatDelivery {
        seq: 1,
        event_id: "$e".into(),
        room_id: "!r:chat".into(),
        room_alias: "swarm".into(),
        is_direct: false,
        image: None,
        basis: None,
        form: Form::Message,
        sender: "@op:chat".into(),
        sender_display: "Op".into(),
        at_unix_ms: 0,
        text: text.into(),
        mentions: mentions.iter().map(|id| (*id).to_string()).collect(),
        context: vec![],
    }
}

fn names(texts: &[&str]) -> Vec<Short> {
    texts.iter().map(|text| Short((*text).to_string())).collect()
}

fn steward(repo: &str, listed: &[&str]) -> Addressee {
    Addressee::new("@nibu:chat", ["Nibu"]).with_projects(project_names(&OrgRepo(repo.into()), &names(listed)))
}

#[test]
fn a_tag_her_name_or_one_of_her_project_names_as_a_whole_word_addresses_her_and_a_substring_does_not() {
    let nibu = steward("GameCult/BadDaySimulator", &["BDS"]);
    assert!(addresses(&nibu, &said("BDS is down", &[])));
    assert!(addresses(&nibu, &said("is the BDS's build green?", &[])));
    assert!(!addresses(&nibu, &said("BDSX is down", &[])));
    assert!(!addresses(&nibu, &said("XBDS is down", &[])));
    assert!(!addresses(&nibu, &said("hello", &["@libby:chat"])));
    assert!(addresses(&nibu, &said("hello", &["@nibu:chat"])), "a tag alone addresses her");
    assert!(addresses(&nibu, &said("NIBU, are you there", &[])), "her own name matches in any case");
    assert!(!addresses(&nibu, &said("Nibus", &[])));
}

#[test]
fn project_names_derives_the_repo_name_and_its_split_form_and_listed_names_replace_them() {
    let derived = |repo: &str| project_names(&OrgRepo(repo.into()), &[]).into_iter().map(|name| (name.text, name.exact)).collect::<Vec<_>>();
    let spelled = |whole: &str, split: &[&str]| {
        let mut names = vec![(whole.to_string(), true)];
        names.extend(split.iter().map(|text| ((*text).to_string(), false)));
        names
    };
    assert_eq!(derived("GameCult/BadDaySimulator"), spelled("BadDaySimulator", &["bad day simulator"]));
    assert_eq!(derived("GameCult/moonlight-ios"), spelled("moonlight-ios", &["moonlight ios"]));
    assert_eq!(derived("GameCult/Huginn"), spelled("Huginn", &[]));
    assert_eq!(derived("GameCult/Thing"), spelled("Thing", &[]));
    assert_eq!(derived("GameCult/snake_case.rs"), spelled("snake_case.rs", &["snake case rs"]));
    assert_eq!(derived("GameCult/Vili2D"), spelled("Vili2D", &["vili2 d"]), "a digit before a capital splits");
    assert_eq!(derived("GameCult/Vili2Dev"), spelled("Vili2Dev", &["vili2 dev"]));
    let listed = project_names(&OrgRepo("GameCult/BadDaySimulator".into()), &names(&["BDS", "Bad Day Simulator"]));
    assert_eq!(listed.into_iter().map(|name| (name.text, name.exact)).collect::<Vec<_>>(), [("BDS".to_string(), true), ("Bad Day Simulator".to_string(), true)]);
}

#[test]
fn a_listed_name_matches_exactly_as_written_a_derived_one_word_name_as_spelled_and_a_split_phrase_in_any_case() {
    let libby = Addressee::new("@libby:chat", ["Libby"]).with_projects(project_names(&OrgRepo("GameCult/Thing".into()), &names(&["Thing"])));
    assert!(addresses(&libby, &said("Thing is down", &[])));
    assert!(!addresses(&libby, &said("one thing at a time", &[])));
    assert!(!addresses(&libby, &said("THING", &[])));
    let derived_thing = steward("GameCult/Thing", &[]);
    assert!(addresses(&derived_thing, &said("Thing is down", &[])));
    for text in ["one thing at a time", "THING", "a thing"] {
        assert!(!addresses(&derived_thing, &said(text, &[])), "{text}");
    }
    let nibu = steward("GameCult/BadDaySimulator", &[]);
    for text in ["BadDaySimulator is red", "BAD DAY SIMULATOR is red", "Bad Day Simulator is red", "bad day simulator is red"] {
        assert!(addresses(&nibu, &said(text, &[])), "{text}");
    }
    for text in ["baddaysimulator is red", "BADDAYSIMULATOR is red", "a bad day, simulated"] {
        assert!(!addresses(&nibu, &said(text, &[])), "{text}");
    }
    let hyphen = steward("GameCult/moonlight-ios", &[]);
    assert!(addresses(&hyphen, &said("moonlight-ios", &[])) && addresses(&hyphen, &said("Moonlight iOS", &[])) && !addresses(&hyphen, &said("Moonlight-iOS", &[])));
}

#[test]
fn one_name_addresses_every_mind_that_lists_it_each_on_her_own_record() {
    let lore = steward("GameCult/AetheriaLore", &[]);
    let aetheria = Addressee::new("@aqua:chat", ["Aqua"]).with_projects(project_names(&OrgRepo("GameCult/Aetheria".into()), &[]));
    let both = said("Aetheria lore needs a pass", &[]);
    assert!(addresses(&aetheria, &both), "a name inside another Mind's longer name addresses her too");
    assert!(addresses(&lore, &both));
    let two_stewards = [steward("GameCult/Thing", &["Thing"]), Addressee::new("@libby:chat", ["Libby"]).with_projects(project_names(&OrgRepo("GameCult/Thing".into()), &[]))];
    assert!(two_stewards.iter().all(|who| addresses(who, &said("Thing is broken", &[]))), "a project with two stewards addresses each");
    assert!(!addresses(&two_stewards[0], &said("a thing", &[])), "listed Thing is exact");
    assert!(!addresses(&two_stewards[1], &said("a thing", &[])), "derived Thing is exact as the repo spells it");
}

#[test]
fn a_name_that_overlaps_itself_is_found_where_a_whole_word_hit_follows_a_failed_one() {
    assert!(names_whole_word("xab ab ab", "ab ab", false));
    assert!(names_whole_word("xab ab ab", "AB AB", false));
    assert!(names_whole_word("xab ab ab", "ab ab", true));
    assert!(!names_whole_word("xab ab abx", "ab ab", true));
    assert!(names_whole_word("é ab", "ab", true), "multibyte text steps by whole characters");
}

#[test]
fn the_localpart_is_the_name_between_the_at_and_the_colon() {
    assert_eq!(localpart("@eureka:chat.gamecult.org"), "eureka");
    assert_eq!(localpart("eureka"), "eureka");
}

#[test]
fn only_a_message_can_address_and_the_mouths_basis_stands_for_the_polling_mind_alone() {
    let nibu = steward("GameCult/Thing", &[]);
    let with = |basis, form, text: &str, mentions: &[&str]| ChatDelivery { basis, form, ..said(text, mentions) };
    for basis in [Basis::Direct, Basis::Tag, Basis::Reply] {
        assert!(is_for(&nibu, &with(Some(basis), Form::Message, "nothing named", &[])), "{basis:?}");
        assert!(!addresses(&nibu, &with(Some(basis), Form::Message, "nothing named", &[])), "a basis is a fact about the Persona that polled, not about every Mind: {basis:?}");
        assert!(!is_for(&nibu, &with(Some(basis), Form::Notice, "nothing named", &[])), "a notice is for no one: {basis:?}");
        assert!(!is_for(&nibu, &with(Some(basis), Form::Edit, "nibu, look", &["@nibu:chat"])), "nor an edit, tagged and named: {basis:?}");
        assert!(!addresses(&nibu, &with(None, Form::Edit, "nibu, look", &["@nibu:chat"])), "{basis:?}");
    }
    assert!(!is_for(&nibu, &with(Some(Basis::Room), Form::Message, "nothing named", &[])), "@room is for no one");
    assert!(!is_for(&nibu, &with(None, Form::Message, "nothing named", &[])), "unproven and unnamed");
    assert!(is_for(&nibu, &with(None, Form::Message, "nibu, look", &[])), "unproven is not 'not for her': her name decides");
    assert!(is_for(&nibu, &with(Some(Basis::Room), Form::Message, "nibu, look", &[])), "an @room that also names her is named");
}
