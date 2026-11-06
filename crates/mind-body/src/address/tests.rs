//! The one addressing rule: a tag, or a name as a whole word; a listed name exactly as written, a derived or a Mind's own in any case.

use super::*;

fn said(text: &str, mentions: &[&str]) -> ChatDelivery {
    ChatDelivery {
        seq: 1,
        event_id: "$e".into(),
        room_id: "!r:chat".into(),
        room_alias: "swarm".into(),
        is_direct: false,
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
    let folded = |texts: &[&str]| texts.iter().map(|text| ((*text).to_string(), false)).collect::<Vec<_>>();
    assert_eq!(derived("GameCult/BadDaySimulator"), folded(&["baddaysimulator", "bad day simulator"]));
    assert_eq!(derived("GameCult/moonlight-ios"), folded(&["moonlight-ios", "moonlight ios"]));
    assert_eq!(derived("GameCult/Huginn"), folded(&["huginn"]));
    assert_eq!(derived("GameCult/snake_case.rs"), folded(&["snake_case.rs", "snake case rs"]));
    assert_eq!(derived("GameCult/Vili2D"), folded(&["vili2d", "vili2 d"]), "a digit before a capital splits");
    assert_eq!(derived("GameCult/Vili2Dev"), folded(&["vili2dev", "vili2 dev"]));
    let listed = project_names(&OrgRepo("GameCult/BadDaySimulator".into()), &names(&["BDS", "Bad Day Simulator"]));
    assert_eq!(listed.into_iter().map(|name| (name.text, name.exact)).collect::<Vec<_>>(), [("BDS".to_string(), true), ("Bad Day Simulator".to_string(), true)]);
}

#[test]
fn a_listed_name_matches_exactly_as_written_and_a_derived_name_in_any_case() {
    let libby = Addressee::new("@libby:chat", ["Libby"]).with_projects(project_names(&OrgRepo("GameCult/Thing".into()), &names(&["Thing"])));
    assert!(addresses(&libby, &said("Thing is down", &[])));
    assert!(!addresses(&libby, &said("one thing at a time", &[])));
    assert!(!addresses(&libby, &said("THING", &[])));
    let nibu = steward("GameCult/BadDaySimulator", &[]);
    for text in ["BADDAYSIMULATOR is red", "Bad Day Simulator is red", "bad day simulator is red", "is BadDaySimulator red"] {
        assert!(addresses(&nibu, &said(text, &[])), "{text}");
    }
    assert!(!addresses(&nibu, &said("a bad day, simulated", &[])));
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
    assert!(!addresses(&two_stewards[0], &said("a thing", &[])), "each by her own record: listed exact, derived in any case");
    assert!(addresses(&two_stewards[1], &said("a thing", &[])));
}

#[test]
fn the_localpart_is_the_name_between_the_at_and_the_colon() {
    assert_eq!(localpart("@eureka:chat.gamecult.org"), "eureka");
    assert_eq!(localpart("eureka"), "eureka");
}
