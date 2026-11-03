//! Every refusal in the design's table, and the ones its edges add, each on its own: the
//! error is part of the contract, and a refused request writes nothing.

mod common;

use common::*;
use resmud::ikigai_core::{Capability, Error, Scope};

fn conflict(e: &Error) -> bool {
    matches!(e, Error::Conflict(_))
}
fn invalid(e: &Error) -> bool {
    matches!(e, Error::InvalidArgument { .. })
}
fn denied(e: &Error) -> bool {
    matches!(e, Error::Denied(_))
}

/// The detail of a refusal, which a player reads.
fn said(answer: &resmud::ikigai_core::Result<String>) -> String {
    match answer {
        Err(Error::Conflict(m)) | Err(Error::Denied(m)) => m.clone(),
        Err(Error::InvalidArgument { detail, .. }) => detail.clone(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ---- go, take, drop ---------------------------------------------------------------------

#[test]
fn go_where_there_is_no_exit_is_a_conflict() {
    let w = World::new();
    let alice = w.file("character/alice.ttl");
    let refused = w.play("sink urn:resmud:verb:name:go north");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(
        said(&refused),
        "there is no exit north from the kitchen: look to see the exits"
    );
    assert_eq!(w.file("character/alice.ttl"), alice);
}

#[test]
fn go_a_bearing_that_is_not_a_direction_is_an_invalid_argument() {
    let w = World::new();
    for bearing in ["sideways", "North", "east!"] {
        let refused = w.play(&format!("sink urn:resmud:verb:name:go {bearing}"));
        assert!(is(&refused, invalid), "{bearing}: {refused:?}");
        assert!(said(&refused).contains("is not a direction"), "{bearing}");
    }
}

#[test]
fn take_a_thing_that_is_not_here_is_a_conflict() {
    let w = World::new();
    // Not anywhere at all.
    let refused = w.play("sink urn:resmud:verb:name:take zebra");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(
        said(&refused),
        "the zebra is not here: look to see what is in the kitchen"
    );
    // Already carried.
    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    let refused = w.play("sink urn:resmud:verb:name:take lamp");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(said(&refused), "you already carry the lamp");
    // In another room.
    w.play("sink urn:resmud:verb:name:drop lamp").unwrap();
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play("sink urn:resmud:verb:name:take lamp");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn take_or_drop_a_thing_that_is_not_a_name_is_an_invalid_argument() {
    let w = World::new();
    for verb in ["take", "drop"] {
        for thing in ["Lamp", "lamp!", "-lamp"] {
            let refused = w.play(&format!("sink urn:resmud:verb:name:{verb} {thing}"));
            assert!(is(&refused, invalid), "{verb} {thing}: {refused:?}");
            assert!(said(&refused).contains("is not the name of a thing"));
        }
    }
}

#[test]
fn drop_a_thing_you_do_not_carry_is_a_conflict() {
    let w = World::new();
    let refused = w.play("sink urn:resmud:verb:name:drop lamp");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(
        said(&refused),
        "you do not carry the lamp: look to see what you carry"
    );
}

#[test]
fn a_caller_who_plays_no_character_is_denied() {
    let w = World::new();
    // Holds every grant a player holds, but names no character.
    let anonymous = Capability::scoped(
        w.alice()
            .scopes()
            .unwrap()
            .iter()
            .filter(|s| !s.starts_with("urn:cap:principal:"))
            .cloned(),
    );
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &anonymous,
        Scope::empty(),
    );
    assert!(is(&refused, denied), "{refused:?}");
    // Names a principal that is not a character.
    let stranger = anonymous.with_principal("urn:example:person:bob").unwrap();
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &stranger,
        Scope::empty(),
    );
    assert!(is(&refused, denied), "{refused:?}");
}

#[test]
fn a_move_without_the_play_capability_is_denied_by_the_declared_floor() {
    let w = World::new();
    let reader = Capability::scoped([w.realm.fs_scope("read", "")])
        .with_principal(&resmud::character_iri("alice"))
        .unwrap();
    for line in [
        "sink urn:resmud:verb:name:go east",
        "sink urn:resmud:verb:name:take lamp",
        "sink urn:resmud:verb:name:drop lamp",
        "sink urn:resmud:verb:id:polish lamp",
    ] {
        let refused = w.play_as(line, &reader, resmud::corridor("alice"));
        assert!(is(&refused, denied), "{line}: {refused:?}");
    }
}

// ---- the realm's own names ----------------------------------------------------------------

#[test]
fn an_atom_cannot_be_written_by_its_public_name() {
    let w = World::new();
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play("sink urn:resmud:thing:id:lamp gone");
    assert!(
        matches!(refused, Err(Error::Unsupported { .. })),
        "a public atom name is a Source-only view: {refused:?}"
    );
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn the_realms_files_are_unreachable_from_outside_it() {
    let w = World::new();
    for line in [
        "source urn:file:thing/lamp.ttl",
        "sink urn:file:thing/lamp.ttl gone",
        "source urn:file:shape.ttl",
    ] {
        let refused = w.play(line);
        assert!(
            matches!(refused, Err(Error::Unresolved(_))),
            "{line}: {refused:?}"
        );
    }
}

// ---- the verb runner ----------------------------------------------------------------------

#[test]
fn a_verb_whose_source_was_changed_after_signing_is_denied() {
    let w = World::new();
    w.publish_with("polish", &POLISH.replace("brass", "brasz"), &w.sign(POLISH));
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    assert!(
        said(&refused).contains("is not what alice signed"),
        "{}",
        said(&refused)
    );
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn a_verb_signed_by_someone_other_than_its_author_is_denied() {
    let w = World::new();
    w.publish_with("polish", POLISH, &w.sign_with(MALLORY_PRIVATE_KEY, POLISH));
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
}

#[test]
fn a_verb_that_names_no_author_or_an_unknown_one_is_denied() {
    let w = World::new();
    w.publish_part("polish", "source", POLISH);
    w.publish_part("polish", "signature", &w.sign(POLISH));
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    assert!(said(&refused).contains("names no author"));
    // An author the realm has not registered has no key to check against.
    w.publish_part("polish", "author", "mallory");
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    // Nor is an author's name allowed to be anything but a key.
    w.publish_part("polish", "author", "../alice");
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
}

#[test]
fn a_verb_runs_under_no_more_than_its_authors_grant() {
    let w = World::new();
    // Alice may write rooms as a player, but her grant as an AUTHOR covers things only, so
    // a verb she wrote that rewrites the kitchen is refused.
    w.publish(
        "redecorate",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#"))
  ((iri "urn:resmud:room:id:kitchen") rm:description "A cold kitchen."))"#,
    );
    let kitchen = w.file("room/kitchen.ttl");
    let refused = w.play("sink urn:resmud:verb:id:redecorate");
    assert!(is(&refused, denied), "{refused:?}");
    assert!(
        said(&refused).contains("do not cover writing a room"),
        "{}",
        said(&refused)
    );
    assert_eq!(w.file("room/kitchen.ttl"), kitchen);
    // And no more than its CALLER holds: a caller who may run verbs but write nothing
    // gets nothing written, whatever the author's grant says.
    w.publish("polish", POLISH);
    let runner_only = Capability::scoped([
        resmud::CAP_VERB_RUN.to_string(),
        w.realm.fs_scope("read", ""),
    ])
    .with_principal(&resmud::character_iri("alice"))
    .unwrap();
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play_as(
        "sink urn:resmud:verb:id:polish lamp",
        &runner_only,
        resmud::corridor("alice"),
    );
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn a_verb_that_writes_a_sealed_name_is_denied() {
    let w = World::new();
    for (name, subject) in [
        ("annex", "urn:resmud:realm:id:first"),
        ("repeal", "urn:resmud:realm:id:first:shape:item"),
        ("usurp", "urn:resmud:author:id:alice"),
    ] {
        let before = (
            w.file("realm.ttl"),
            w.file("shape.ttl"),
            w.file("author/alice.ttl"),
        );
        w.publish(
            name,
            &format!(
                r#"(graph
  (prefix (rml "urn:resmud:term:name:"))
  ((iri "{subject}") rml:grant "urn:cap:resmud:write:room"))"#
            ),
        );
        let refused = w.play(&format!("sink urn:resmud:verb:id:{name}"));
        assert!(is(&refused, denied), "{name}: {refused:?}");
        assert!(said(&refused).contains("is sealed"), "{name}");
        let after = (
            w.file("realm.ttl"),
            w.file("shape.ttl"),
            w.file("author/alice.ttl"),
        );
        assert_eq!(before, after, "{name} changed a sealed atom");
    }
}

#[test]
fn a_thing_without_a_text_description_is_refused_by_the_shapes() {
    let w = World::new();
    w.publish(
        "conjure",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#"))
  ((iri "urn:resmud:thing:id:pebble") a rm:Item))"#,
    );
    let refused = w.play("sink urn:resmud:verb:id:conjure");
    assert!(is(&refused, invalid), "{refused:?}");
    assert_eq!(
        said(&refused),
        "the realm's shapes refuse the verb conjure: urn:resmud:thing:id:pebble has no \
         rm:description, and every thing needs a text description: give it one"
    );
    assert!(!w.root().join("thing/pebble.ttl").exists());
}

#[test]
fn a_verb_on_a_thing_that_is_not_a_name_is_an_invalid_argument() {
    let w = World::new();
    w.publish("polish", POLISH);
    let refused = w.play("sink urn:resmud:verb:id:polish Lamp!");
    assert!(is(&refused, invalid), "{refused:?}");
}

#[test]
fn a_verb_that_needs_a_thing_and_is_given_none_is_refused() {
    let w = World::new();
    w.publish("polish", POLISH);
    let refused = w.play("sink urn:resmud:verb:id:polish");
    assert!(
        matches!(refused, Err(Error::MissingArgument(ref name)) if name == "content"),
        "{refused:?}"
    );
}

#[test]
fn a_verb_whose_source_is_not_a_graph_is_an_invalid_argument() {
    let w = World::new();
    w.publish("broken", "(select (?x) (where (?x ?y ?z)))");
    let refused = w.play("sink urn:resmud:verb:id:broken");
    assert!(is(&refused, invalid), "{refused:?}");
}
