//! The design's "Done means": one test plays the smallest playable thing through, on the
//! text face, the way a player at the `ikigai` REPL would (`source urn:resmud:look`).

mod common;

use common::*;
use resmud::ikigai_core::{Error, Request, Verb};

#[test]
fn the_smallest_playable_thing_plays_through_on_the_text_face() {
    let w = World::new();
    let room_view = || Request::new(Verb::Source, iri("urn:resmud:view:room:id:kitchen"));
    let cached = |w: &World| {
        w.kernel
            .is_cached_in(&room_view(), &w.alice(), &resmud::corridor("alice"))
    };

    // 1. `look` lists the room's description, its exits, and the lamp.
    let look = w.play("source urn:resmud:look").unwrap();
    assert_eq!(
        look,
        "A warm kitchen. Bread is cooling on the sill.\n\
         You see the lamp. A brass lamp, dull with age.\n\
         Exits: east.\n\
         You carry nothing."
    );
    assert!(cached(&w), "the room text is cached after a look");

    // 2. `take lamp`; `look` again: the lamp is gone from the room's text, and the
    //    character carries it. The room text was recomputed because its thread was cut,
    //    not because anything rewrote it: the room's own atom and the lamp's are
    //    byte-for-byte unchanged, and only the kitchen's contents and what alice carries
    //    were written.
    let kitchen_before = w.file("room/kitchen.ttl");
    let lamp_before = w.file("thing/lamp.ttl");
    assert_eq!(
        w.play("sink urn:resmud:verb:name:take lamp").unwrap(),
        "You take the lamp."
    );
    assert!(!cached(&w), "taking the lamp cut the room text's thread");
    assert_eq!(
        w.file("room/kitchen.ttl"),
        kitchen_before,
        "nothing rewrote the room"
    );
    assert_eq!(
        w.file("thing/lamp.ttl"),
        lamp_before,
        "nothing rewrote the lamp"
    );
    assert!(w.file("carries/alice.ttl").contains("thing:id:lamp"));
    assert!(!w.file("content/kitchen.ttl").contains("thing:id:lamp"));
    let look = w.play("source urn:resmud:look").unwrap();
    assert_eq!(
        look,
        "A warm kitchen. Bread is cooling on the sill.\n\
         Nothing here can be taken.\n\
         Exits: east.\n\
         You carry the lamp. A brass lamp, dull with age."
    );
    assert!(cached(&w), "the recomputed room text is cached again");

    // 3. A player publishes `polish`, signed. `polish lamp` changes the lamp's description.
    w.publish("polish", POLISH);
    assert_eq!(
        w.play("sink urn:resmud:verb:id:polish lamp").unwrap(),
        "You polish the lamp."
    );
    let look = w.play("source urn:resmud:look").unwrap();
    assert!(
        look.ends_with("You carry the lamp. A brass lamp, polished until it shines."),
        "{look}"
    );

    // 4. A copy of `polish` with one byte changed is refused (Denied).
    let tampered = POLISH.replace("shines", "shinez");
    assert_eq!(tampered.len(), POLISH.len());
    w.publish_with("polish-copy", &tampered, &w.sign(POLISH));
    let lamp = w.file("thing/lamp.ttl");
    let refused = w.play("sink urn:resmud:verb:id:polish-copy lamp");
    assert!(
        is(&refused, |e| matches!(e, Error::Denied(_))),
        "{refused:?}"
    );
    assert_eq!(
        w.file("thing/lamp.ttl"),
        lamp,
        "the refused verb wrote nothing"
    );

    // 5. A verb that tries to write the realm's shapes is refused (Denied).
    let shapes = w.file("shape.ttl");
    w.publish(
        "repeal",
        r#"(graph
  (prefix (sh "http://www.w3.org/ns/shacl#"))
  ((iri "urn:resmud:realm:id:first:shape:item:description") sh:minCount 0))"#,
    );
    let refused = w.play("sink urn:resmud:verb:id:repeal");
    assert!(
        is(&refused, |e| matches!(e, Error::Denied(_))),
        "{refused:?}"
    );
    assert_eq!(w.file("shape.ttl"), shapes, "the shapes are unchanged");

    // 6. Creating a thing with no description is refused by the shapes.
    w.publish(
        "conjure",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#") (rdfs "http://www.w3.org/2000/01/rdf-schema#"))
  ((iri "urn:resmud:thing:id:pebble") a rm:Item)
  ((iri "urn:resmud:thing:id:pebble") rdfs:label "pebble"))"#,
    );
    let refused = w.play("sink urn:resmud:verb:id:conjure");
    match &refused {
        Err(Error::InvalidArgument { detail, .. }) => {
            // Named by its label, never its IRI.
            assert_eq!(
                detail,
                "the realm's rules refuse the verb conjure: the pebble has no description, and \
                 every thing needs a text description, so give it one"
            );
        }
        other => panic!("expected the shapes to refuse it, got {other:?}"),
    }
    assert!(
        !w.root().join("thing/pebble.ttl").exists(),
        "nothing was created"
    );
    assert!(!w.file("world.txt").contains("pebble"));
}

#[test]
fn every_answer_of_the_text_face_is_plain_text() {
    let w = World::new();
    w.publish("polish", POLISH);
    for line in [
        "source urn:resmud:look",
        "source urn:resmud:affordance",
        "source urn:resmud:view:room:id:kitchen",
        "sink urn:resmud:verb:name:take lamp",
        "sink urn:resmud:verb:id:polish lamp",
        "sink urn:resmud:verb:name:go east",
        "sink urn:resmud:verb:name:drop lamp",
    ] {
        let answer = w
            .issue(repl(line), &w.alice(), resmud::corridor("alice"))
            .unwrap_or_else(|e| panic!("{line}: {e:?}"));
        assert_eq!(answer.repr_type.media_type, "text/plain", "{line}");
        let said = String::from_utf8(answer.bytes).expect("UTF-8 text");
        assert!(!said.is_empty(), "{line} said nothing");
        // Plain text only: no terminal escape (so no color), no markup, no IRI, and no
        // angle brackets (the affordance list included).
        assert!(!said.contains('\u{1b}'), "{line}: {said:?}");
        assert!(resmud::is_plain(&said), "{line}: {said:?}");
    }
}

#[test]
fn the_affordance_is_the_realms_actions_under_the_callers_capability() {
    let w = World::new();
    assert_eq!(
        w.play("source urn:resmud:affordance").unwrap(),
        "a verb a player wrote, then a thing: run it on that thing, such as polish lamp\n\
         drop a thing: put down a thing you carry\n\
         go a direction: walk through an exit (north, south, east, west, up or down)\n\
         take a thing: pick up a thing that is here"
    );
    // A visitor who may read but not play is offered nothing to do, and is refused the
    // moves it was not offered: the list is the parser.
    let visitor = resmud::ikigai_core::Capability::scoped([w.realm.fs_scope("read", "")])
        .with_principal(&resmud::character_iri("alice"))
        .unwrap();
    assert_eq!(
        w.play_as(
            "source urn:resmud:affordance",
            &visitor,
            resmud::corridor("alice")
        )
        .unwrap(),
        ""
    );
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &visitor,
        resmud::corridor("alice"),
    );
    assert!(
        is(&refused, |e| matches!(e, Error::Denied(_))),
        "{refused:?}"
    );
}

#[test]
fn moving_and_dropping_change_what_everyone_sees() {
    let w = World::new();
    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    assert_eq!(
        w.play("sink urn:resmud:verb:name:go east").unwrap(),
        "You go east, to the garden."
    );
    assert_eq!(
        w.play("sink urn:resmud:verb:name:drop lamp").unwrap(),
        "You drop the lamp."
    );
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A small walled garden. Mint grows between the stones.\n\
         You see the lamp. A brass lamp, dull with age.\n\
         Exits: west.\n\
         You carry nothing."
    );
    // The garden's public text, read by anyone who may read the realm, says the same.
    let reader = resmud::ikigai_core::Capability::scoped([w.realm.fs_scope("read", "")]);
    assert!(w
        .play_as(
            "source urn:resmud:view:room:id:garden",
            &reader,
            resmud::ikigai_core::Scope::empty()
        )
        .unwrap()
        .contains("You see the lamp."));
}

#[test]
fn a_verb_can_create_a_thing_that_has_a_description() {
    let w = World::new();
    w.publish(
        "conjure",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#") (rdfs "http://www.w3.org/2000/01/rdf-schema#"))
  ((iri "urn:resmud:thing:id:pebble") a rm:Item)
  ((iri "urn:resmud:thing:id:pebble") rdfs:label "pebble")
  ((iri "urn:resmud:thing:id:pebble") rm:description "A smooth grey pebble."))"#,
    );
    assert_eq!(
        w.play("sink urn:resmud:verb:id:conjure").unwrap(),
        "You conjure."
    );
    assert!(w.file("world.txt").contains("urn:resmud:thing:id:pebble"));
    assert_eq!(
        w.play("source urn:resmud:thing:id:pebble:description")
            .unwrap(),
        "A smooth grey pebble."
    );
    // It is nowhere: nothing placed it, so no room lists it.
    assert!(!w.play("source urn:resmud:look").unwrap().contains("pebble"));
}

#[test]
fn a_verb_can_create_a_thing_and_place_it_here() {
    let w = World::new();
    let kitchen = w.file("room/kitchen.ttl");
    // A triple that places a thing writes the room's CONTENTS atom, not the room's own, so
    // alice's grant (things, and what a room contains) covers it without covering the room.
    w.publish(
        "conjure-here",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#") (rms "https://iriref.org/resmud/space#")
          (rdfs "http://www.w3.org/2000/01/rdf-schema#"))
  ((iri "urn:resmud:thing:id:pebble") a rm:Item)
  ((iri "urn:resmud:thing:id:pebble") rdfs:label "pebble")
  ((iri "urn:resmud:thing:id:pebble") rm:description "A smooth grey pebble.")
  ((iri "urn:resmud:room:id:kitchen") rms:contains (iri "urn:resmud:thing:id:pebble")))"#,
    );
    assert_eq!(
        w.play("sink urn:resmud:verb:id:conjure-here").unwrap(),
        "You conjure-here."
    );
    // The room lists both things, by label.
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A warm kitchen. Bread is cooling on the sill.\n\
         You see the lamp. A brass lamp, dull with age. \
         You see the pebble. A smooth grey pebble.\n\
         Exits: east.\n\
         You carry nothing."
    );
    assert_eq!(
        w.file("room/kitchen.ttl"),
        kitchen,
        "the room itself is unchanged"
    );
    assert!(w.file("world.txt").contains("urn:resmud:thing:id:pebble"));
    // A placed thing is a thing like any other: it can be taken.
    assert_eq!(
        w.play("sink urn:resmud:verb:name:take pebble").unwrap(),
        "You take the pebble."
    );
}

#[test]
fn a_take_recomputes_only_what_read_the_two_atoms_it_wrote() {
    let w = World::new();
    let alice = w.alice();
    let scope = || resmud::corridor("alice");
    let read = |name: &str| {
        w.issue(Request::new(Verb::Source, iri(name)), &alice, scope())
            .unwrap();
    };
    let cached = |name: &str| {
        w.kernel
            .is_cached_in(&Request::new(Verb::Source, iri(name)), &alice, &scope())
    };
    // A take writes the kitchen's contents and what alice carries, and nothing else, so only
    // what read one of those two atoms recomputes: a room elsewhere, its text included, stays
    // cached, and so does the taken thing's own description.
    let kept = [
        "urn:resmud:room:id:garden:description",
        "urn:resmud:room:id:garden:exit",
        "urn:resmud:room:id:garden:content:line",
        "urn:resmud:view:room:id:garden",
        "urn:resmud:room:id:kitchen:description",
        "urn:resmud:room:id:kitchen:exit",
        "urn:resmud:character:id:alice:place",
        "urn:resmud:thing:id:lamp:description",
    ];
    let recomputed = [
        "urn:resmud:room:id:kitchen:content",
        "urn:resmud:room:id:kitchen:content:line",
        "urn:resmud:view:room:id:kitchen",
        "urn:resmud:character:id:alice:carries",
        "urn:resmud:character:id:alice:inventory",
    ];
    for name in kept.iter().chain(&recomputed) {
        read(name);
        assert!(cached(name), "{name} is cached once read");
    }
    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    for name in kept {
        assert!(
            cached(name),
            "{name} did not read either atom the take wrote, and stays cached"
        );
    }
    for name in recomputed {
        assert!(
            !cached(name),
            "{name} read an atom the take wrote, and its thread was cut"
        );
    }
}

#[test]
fn a_room_and_an_inventory_list_their_things_by_label_then_by_name() {
    let w = World::new();
    // Five things whose keys, labels and descriptions each sort differently, two of them
    // with the same label: the line lists them by label, and two with the same label by
    // their names (`stone-1` before `stone-9`), whatever order they were written in.
    w.publish(
        "stock",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#") (rms "https://iriref.org/resmud/space#")
          (rdfs "http://www.w3.org/2000/01/rdf-schema#"))
  ((iri "urn:resmud:thing:id:zz") a rm:Item)
  ((iri "urn:resmud:thing:id:zz") rdfs:label "apple")
  ((iri "urn:resmud:thing:id:zz") rm:description "A red apple.")
  ((iri "urn:resmud:thing:id:stone-9") a rm:Item)
  ((iri "urn:resmud:thing:id:stone-9") rdfs:label "stone")
  ((iri "urn:resmud:thing:id:stone-9") rm:description "A grey stone.")
  ((iri "urn:resmud:thing:id:aa") a rm:Item)
  ((iri "urn:resmud:thing:id:aa") rdfs:label "zither")
  ((iri "urn:resmud:thing:id:aa") rm:description "A zither with one string.")
  ((iri "urn:resmud:thing:id:stone-1") a rm:Item)
  ((iri "urn:resmud:thing:id:stone-1") rdfs:label "stone")
  ((iri "urn:resmud:thing:id:stone-1") rm:description "A white stone.")
  ((iri "urn:resmud:thing:id:mm") a rm:Item)
  ((iri "urn:resmud:thing:id:mm") rdfs:label "mug")
  ((iri "urn:resmud:thing:id:mm") rm:description "A chipped mug.")
  ((iri "urn:resmud:room:id:kitchen") rms:contains (iri "urn:resmud:thing:id:zz"))
  ((iri "urn:resmud:room:id:kitchen") rms:contains (iri "urn:resmud:thing:id:stone-9"))
  ((iri "urn:resmud:room:id:kitchen") rms:contains (iri "urn:resmud:thing:id:aa"))
  ((iri "urn:resmud:room:id:kitchen") rms:contains (iri "urn:resmud:thing:id:stone-1"))
  ((iri "urn:resmud:room:id:kitchen") rms:contains (iri "urn:resmud:thing:id:mm")))"#,
    );
    w.play("sink urn:resmud:verb:id:stock").unwrap();
    let things = "You see the apple. A red apple. \
                  You see the lamp. A brass lamp, dull with age. \
                  You see the mug. A chipped mug. \
                  You see the stone. A white stone. \
                  You see the stone. A grey stone. \
                  You see the zither. A zither with one string.";
    assert_eq!(
        w.play("source urn:resmud:room:id:kitchen:content:line")
            .unwrap(),
        things
    );
    // Everything taken, in another order again: the inventory lists them the same way.
    for key in ["mm", "lamp", "stone-9", "aa", "stone-1", "zz"] {
        w.play(&format!("sink urn:resmud:verb:name:take {key}"))
            .unwrap();
    }
    assert_eq!(
        w.play("source urn:resmud:character:id:alice:inventory")
            .unwrap(),
        things.replace("You see", "You carry")
    );
    assert_eq!(
        w.play("source urn:resmud:room:id:kitchen:content:line")
            .unwrap(),
        "Nothing here can be taken."
    );
}
