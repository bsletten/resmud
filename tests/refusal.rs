//! Every refusal in the design's table, and the ones its edges add, each on its own: the
//! error is part of the contract, and a refused request writes nothing. Each test pins the
//! exact sentence a player reads (`resmud::refusal` of the error, what a door shows), so a
//! refusal stays plain words: no IRI, no angle brackets, no backticks.

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

/// The sentence a player reads for a refusal: `resmud::refusal` of the error, which is what
/// a door shows. Every one is plain words.
fn said(answer: &resmud::ikigai_core::Result<String>) -> String {
    match answer {
        Err(e) => {
            let said = resmud::refusal(e);
            assert!(resmud::is_plain(&said), "not plain: {said}");
            said
        }
        Ok(text) => panic!("expected a refusal, got {text:?}"),
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
        "There is no exit north from the kitchen: look to see the exits."
    );
    assert_eq!(w.file("character/alice.ttl"), alice);
}

#[test]
fn go_a_bearing_that_is_not_a_direction_is_an_invalid_argument() {
    let w = World::new();
    for bearing in ["sideways", "North", "east!"] {
        let refused = w.play(&format!("sink urn:resmud:verb:name:go {bearing}"));
        assert!(is(&refused, invalid), "{bearing}: {refused:?}");
        assert_eq!(
            said(&refused),
            format!("\"{bearing}\" is not a direction: say north, south, east, west, up or down.")
        );
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
        "The zebra is not here: look to see what is in the kitchen."
    );
    // Already carried.
    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    let refused = w.play("sink urn:resmud:verb:name:take lamp");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(said(&refused), "You already carry the lamp.");
    // In another room.
    w.play("sink urn:resmud:verb:name:drop lamp").unwrap();
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play("sink urn:resmud:verb:name:take lamp");
    assert!(is(&refused, conflict), "{refused:?}");
    assert_eq!(
        said(&refused),
        "The lamp is not here: look to see what is in the garden."
    );
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn take_or_drop_a_thing_that_is_not_a_name_is_an_invalid_argument() {
    let w = World::new();
    for verb in ["take", "drop"] {
        for thing in ["Lamp", "lamp!", "-lamp"] {
            let refused = w.play(&format!("sink urn:resmud:verb:name:{verb} {thing}"));
            assert!(is(&refused, invalid), "{verb} {thing}: {refused:?}");
            assert_eq!(
                said(&refused),
                format!(
                    "\"{thing}\" is not the name of a thing: say it the way look shows it, \
                     such as lamp."
                )
            );
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
        "You do not carry the lamp: look to see what you carry."
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
    assert_eq!(said(&refused), NOT_PLAYING);
    // Names a principal that is not a character: the refusal does not echo it.
    let stranger = anonymous.with_principal("urn:example:person:bob").unwrap();
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &stranger,
        Scope::empty(),
    );
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(said(&refused), NOT_PLAYING);
    // Names a character the realm has no atom for.
    let bob = resmud::player(&w.realm, "bob");
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &bob,
        resmud::corridor("bob"),
    );
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(said(&refused), NOT_PLAYING);
}

const NOT_PLAYING: &str = "You are not playing a character in this realm, so you cannot act in it.";

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
        // The kernel's own denial names the capability by IRI; a player reads this.
        assert_eq!(
            said(&refused),
            "You may not do that here: ask what you can do here to see what you may.",
            "{line}"
        );
    }
}

#[test]
fn a_move_given_nothing_to_act_on_says_what_it_needs() {
    let w = World::new();
    for line in [
        "sink urn:resmud:verb:name:go",
        "sink urn:resmud:verb:name:take",
        "sink urn:resmud:verb:name:drop",
    ] {
        let refused = w.play(line);
        assert!(
            matches!(refused, Err(Error::MissingArgument(_))),
            "{line}: {refused:?}"
        );
        assert_eq!(
            said(&refused),
            "That needs one more word: a direction to go, or the thing to take, drop or run a \
             verb on, such as take lamp.",
            "{line}"
        );
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
    assert_eq!(
        said(&refused),
        "That cannot be done to it: ask what you can do here."
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
        assert_eq!(
            said(&refused),
            "There is nothing by that name in this realm.",
            "{line}"
        );
    }
    // A room the realm does not have: its file's absence names the file, which a player
    // does not read.
    let refused = w.play("source urn:resmud:view:room:id:attic");
    assert!(matches!(refused, Err(Error::NotFound(_))), "{refused:?}");
    assert_eq!(
        said(&refused),
        "There is nothing by that name in this realm."
    );
}

// ---- the verb runner ----------------------------------------------------------------------

#[test]
fn a_verb_whose_source_was_changed_after_signing_is_denied() {
    let w = World::new();
    w.publish_with("polish", &POLISH.replace("brass", "brasz"), &w.sign(POLISH));
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(said(&refused), NOT_SIGNED);
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn a_verb_signed_by_someone_other_than_its_author_is_denied() {
    let w = World::new();
    w.publish_with("polish", POLISH, &w.sign_with(MALLORY_PRIVATE_KEY, POLISH));
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    // The signature's own verdict (the keys, the hashes) is not shown to a player.
    assert_eq!(said(&refused), NOT_SIGNED);
}

const NOT_SIGNED: &str = "The verb polish is not what alice signed, so it does not run: it was \
                          changed after it was signed, or someone else signed it.";

fn not_found(e: &Error) -> bool {
    matches!(e, Error::NotFound(_))
}

#[test]
fn a_verb_nobody_published_is_not_found_before_its_author_or_thing_is_read() {
    let w = World::new();
    for line in [
        "sink urn:resmud:verb:id:dance",
        "sink urn:resmud:verb:id:dance lamp",
        // The thing's name is not checked first: there is no verb to run on it.
        "sink urn:resmud:verb:id:dance Lamp",
    ] {
        let refused = w.play(line);
        assert!(is(&refused, not_found), "{line}: {refused:?}");
        assert_eq!(said(&refused), "There is no verb called dance.", "{line}");
    }
    // A verb whose source is published but whose author is not still reaches the author
    // check (the next test), so the existence check reads the source alone.
}

#[test]
fn a_verb_that_names_no_author_or_an_unknown_one_is_denied() {
    let w = World::new();
    w.publish_part("polish", "source", POLISH);
    w.publish_part("polish", "signature", &w.sign(POLISH));
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(
        said(&refused),
        "The verb polish names no author, so its signature cannot be checked: its author can \
         publish it again with their name."
    );
    // An author the realm has not registered has no key to check against.
    w.publish_part("polish", "author", "mallory");
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(
        said(&refused),
        "The verb polish names mallory as its author, and this realm has no author called \
         mallory, so its signature cannot be checked."
    );
    // Nor is an author's name allowed to be anything but a key.
    w.publish_part("polish", "author", "../alice");
    let refused = w.play("sink urn:resmud:verb:id:polish lamp");
    assert!(is(&refused, denied), "{refused:?}");
    assert_eq!(
        said(&refused),
        "The verb polish names \"../alice\" as its author, and no author has that name, so \
         its signature cannot be checked."
    );
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
    assert_eq!(
        said(&refused),
        "The verb redecorate writes the kitchen, and it runs under no more than both alice's \
         grant and yours, which together do not cover writing a room."
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
    assert_eq!(
        said(&refused),
        "The verb polish writes the lamp, and it runs under no more than both alice's grant \
         and yours, which together do not cover writing a thing."
    );
    assert_eq!(w.file("thing/lamp.ttl"), lamp);
}

#[test]
fn a_verb_that_writes_a_sealed_name_is_denied() {
    let w = World::new();
    for (name, subject, what) in [
        ("annex", "urn:resmud:realm:id:first", "the realm itself"),
        (
            "repeal",
            "urn:resmud:realm:id:first:shape:item",
            "the realm's rules",
        ),
        (
            "usurp",
            "urn:resmud:author:id:alice",
            "who the realm's authors are",
        ),
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
        assert_eq!(
            said(&refused),
            format!(
                "The verb {name} would change {what}, and no verb may change the realm, its \
                 rules or its authors: that is legislation, not play."
            )
        );
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
        "The realm's rules refuse the verb conjure: the pebble has no description, and every \
         thing needs a text description, so give it one."
    );
    assert!(!w.root().join("thing/pebble.ttl").exists());
}

#[test]
fn a_verb_on_a_thing_that_is_not_a_name_is_an_invalid_argument() {
    let w = World::new();
    w.publish("polish", POLISH);
    let refused = w.play("sink urn:resmud:verb:id:polish Lamp!");
    assert!(is(&refused, invalid), "{refused:?}");
    assert_eq!(
        said(&refused),
        "\"Lamp!\" is not the name of a thing: say it the way look shows it, such as lamp."
    );
    // Nor is a verb's own name anything but a key.
    let refused = w.play("sink urn:resmud:verb:id:Polish lamp");
    assert!(is(&refused, invalid), "{refused:?}");
    assert_eq!(
        said(&refused),
        "\"Polish\" is not the name of a verb: say it the way its author published it, such as \
         polish."
    );
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
    assert_eq!(
        said(&refused),
        "That needs one more word: a direction to go, or the thing to take, drop or run a verb \
         on, such as take lamp."
    );
}

#[test]
fn a_verb_whose_source_is_not_a_graph_is_an_invalid_argument() {
    let w = World::new();
    w.publish("broken", "(select (?x) (where (?x ?y ?z)))");
    let refused = w.play("sink urn:resmud:verb:id:broken");
    assert!(is(&refused, invalid), "{refused:?}");
    // The transreptor's own words name itself by IRI; the runner says it plainly.
    assert_eq!(
        said(&refused),
        "The verb broken cannot run, because its source is not a graph of changes: its author \
         can fix it and publish it again."
    );
}

#[test]
fn a_verb_that_writes_outside_the_realm_or_nothing_is_an_invalid_argument() {
    let w = World::new();
    w.publish(
        "elsewhere",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#"))
  ((iri "urn:example:lamp") rm:description "A lamp somewhere else."))"#,
    );
    let refused = w.play("sink urn:resmud:verb:id:elsewhere");
    assert!(is(&refused, invalid), "{refused:?}");
    assert_eq!(
        said(&refused),
        "The verb elsewhere writes something that is not a thing, character or room of this \
         realm, so it does not run."
    );
    w.publish("idle", "(graph)");
    let refused = w.play("sink urn:resmud:verb:id:idle");
    assert!(is(&refused, invalid), "{refused:?}");
    assert_eq!(said(&refused), "The verb idle changes nothing.");
}

#[test]
fn a_blank_word_is_a_word_left_out() {
    // The `ikigai` REPL sends a `sink` with nothing after its target as EMPTY content, never
    // as none, so a blank `content` must read as the word left out, not as a word that is
    // not a direction or a thing.
    let w = World::new();
    w.publish("polish", POLISH);
    for target in [
        "urn:resmud:verb:name:go",
        "urn:resmud:verb:name:take",
        "urn:resmud:verb:name:drop",
        "urn:resmud:verb:id:polish",
    ] {
        for blank in ["", "  "] {
            let request =
                resmud::ikigai_core::Request::new(resmud::ikigai_core::Verb::Sink, iri(target))
                    .with_arg(
                        "content",
                        resmud::ikigai_core::ArgRef::Inline(blank.as_bytes().to_vec()),
                    );
            let refused = w
                .issue(request, &w.alice(), resmud::corridor("alice"))
                .map(|_| String::new());
            assert!(
                matches!(refused, Err(Error::MissingArgument(ref name)) if name == "content"),
                "{target} {blank:?}: {refused:?}"
            );
        }
    }
}
