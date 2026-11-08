//! The design's "Done means", replayed THROUGH THE DOOR: the `ikigai` REPL (`ikigai-engine`)
//! opened for alice, fed the words a player types. Same world, same expected text as
//! `tests/play.rs`, which issues the same requests to the kernel directly and stays.
//!
//! Publishing a verb is the author's act, not a line at the prompt (a verb's source is several
//! lines, and signing it needs the author's key, which the test host holds), so the harness
//! publishes through the kernel as `tests/play.rs` does; every line a PLAYER types goes
//! through the door.

mod common;

use common::*;
use resmud::door::{Door, Turn, LOOK_AT_ONE, PROMPT, WITHHELD};
use resmud::ikigai_core::{Capability, Error, Request, Scope, Verb};

/// The answer a turn printed, or a panic naming the refusal.
fn said(turn: Turn) -> String {
    match turn {
        Turn::Said(text) => text,
        other => panic!("expected an answer, got {other:?}"),
    }
}

/// The refusal a turn printed: its sentence and the typed error behind it.
fn refused(turn: Turn) -> (String, Option<Error>) {
    match turn {
        Turn::Refused { said, error } => (said, error),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Every answer and refusal the door prints is plain text: no terminal escape (so no color),
/// no markup, no IRI, no angle brackets.
fn assert_printable(line: &str, turn: &Turn) {
    let text = turn
        .text()
        .unwrap_or_else(|| panic!("{line}: printed nothing"));
    assert!(!text.contains('\u{1b}'), "{line}: {text:?}");
    assert!(
        text.chars().all(|c| !c.is_control() || c == '\n'),
        "{line}: {text:?}"
    );
    assert!(resmud::is_plain(text), "{line}: {text:?}");
}

#[test]
fn the_smallest_playable_thing_plays_through_the_door() {
    let w = World::new();
    let door = w.door();
    let room_view = || Request::new(Verb::Source, iri("urn:resmud:view:room:id:kitchen"));
    let cached = |w: &World| {
        w.kernel
            .is_cached_in(&room_view(), &w.alice(), &resmud::corridor("alice"))
    };

    // 1. `look` lists the room's description, its exits, and the lamp. The REPL's own
    //    spelling of it says the same.
    let first = "A warm kitchen. Bread is cooling on the sill.\n\
                 You see the lamp. A brass lamp, dull with age.\n\
                 Exits: east.\n\
                 You carry nothing.";
    assert_eq!(said(door.eval("look")), first);
    assert_eq!(said(door.eval("source urn:resmud:look")), first);
    assert_eq!(
        said(door.eval("Look")),
        first,
        "the door's words are read in any case"
    );
    assert_eq!(said(door.eval("LOOK")), first);
    assert!(cached(&w), "the room text is cached after a look");
    // `look lamp` is the lamp's own description.
    assert_eq!(said(door.eval("look lamp")), "A brass lamp, dull with age.");

    // 2. `take lamp`; `look` again: the lamp is gone from the room's text, and the
    //    character carries it, recomputed because its thread was cut.
    let kitchen_before = w.file("room/kitchen.ttl");
    assert_eq!(said(door.eval("take lamp")), "You take the lamp.");
    assert!(!cached(&w), "taking the lamp cut the room text's thread");
    assert_eq!(
        w.file("room/kitchen.ttl"),
        kitchen_before,
        "nothing rewrote the room"
    );
    assert!(w.file("carries/alice.ttl").contains("thing:id:lamp"));
    assert!(!w.file("content/kitchen.ttl").contains("thing:id:lamp"));
    assert_eq!(
        said(door.eval("look")),
        "A warm kitchen. Bread is cooling on the sill.\n\
         Nothing here can be taken.\n\
         Exits: east.\n\
         You carry the lamp. A brass lamp, dull with age."
    );
    assert!(cached(&w), "the recomputed room text is cached again");

    // 3. A player publishes `polish`, signed. `polish lamp` changes the lamp's description.
    w.publish("polish", POLISH);
    assert_eq!(said(door.eval("polish lamp")), "You polish the lamp.");
    let look = said(door.eval("look"));
    assert!(
        look.ends_with("You carry the lamp. A brass lamp, polished until it shines."),
        "{look}"
    );
    assert_eq!(
        said(door.eval("look lamp")),
        "A brass lamp, polished until it shines.",
        "the lamp's description recomputed when the verb rewrote it"
    );

    // 4. A copy of `polish` with one byte changed is refused (Denied).
    let tampered = POLISH.replace("shines", "shinez");
    assert_eq!(tampered.len(), POLISH.len());
    w.publish_with("polish-copy", &tampered, &w.sign(POLISH));
    let lamp = w.file("thing/lamp.ttl");
    let (sentence, error) = refused(door.eval("polish-copy lamp"));
    assert!(matches!(error, Some(Error::Denied(_))), "{error:?}");
    assert_eq!(
        sentence,
        "The verb polish-copy is not what alice signed, so it does not run: it was changed \
         after it was signed, or someone else signed it."
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
    let (sentence, error) = refused(door.eval("repeal"));
    assert!(matches!(error, Some(Error::Denied(_))), "{error:?}");
    assert_eq!(
        sentence,
        "The verb repeal would change the realm's rules, and no verb may change the realm, its \
         rules or its authors: that is legislation, not play."
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
    let (sentence, error) = refused(door.eval("conjure"));
    assert!(
        matches!(error, Some(Error::InvalidArgument { .. })),
        "{error:?}"
    );
    assert_eq!(
        sentence,
        "The realm's rules refuse the verb conjure: the pebble has no description, and every \
         thing needs a text description, so give it one."
    );
    assert!(
        !w.root().join("thing/pebble.ttl").exists(),
        "nothing was created"
    );
    assert!(!w.file("world.txt").contains("pebble"));
}

#[test]
fn moving_and_dropping_through_the_door_change_what_everyone_sees() {
    let w = World::new();
    let door = w.door();
    assert_eq!(said(door.eval("take lamp")), "You take the lamp.");
    assert_eq!(said(door.eval("go east")), "You go east, to the garden.");
    assert_eq!(said(door.eval("drop lamp")), "You drop the lamp.");
    assert_eq!(
        said(door.eval("look")),
        "A small walled garden. Mint grows between the stones.\n\
         You see the lamp. A brass lamp, dull with age.\n\
         Exits: west.\n\
         You carry nothing."
    );
}

#[test]
fn asking_what_you_can_do_here_is_the_affordance_list() {
    let w = World::new();
    let list = "a verb a player wrote, then a thing: run it on that thing, such as polish lamp\n\
                drop a thing: put down a thing you carry\n\
                go a direction: walk through an exit (north, south, east, west, up or down)\n\
                take a thing: pick up a thing that is here";
    let door = w.door();
    assert_eq!(said(door.eval("help")), list);
    assert_eq!(said(door.eval("what")), list);
    assert_eq!(said(door.eval("source urn:resmud:affordance")), list);

    // A visitor who may read but not play is offered nothing, and refused what it was not
    // offered, in words.
    let visitor = Capability::scoped([w.realm.fs_scope("read", "")])
        .with_principal(&resmud::character_iri("alice"))
        .unwrap();
    let door = w.door_as(&visitor, resmud::corridor("alice"));
    assert_eq!(said(door.eval("help")), "");
    let (sentence, error) = refused(door.eval("take lamp"));
    assert!(matches!(error, Some(Error::Denied(_))), "{error:?}");
    assert_eq!(
        sentence,
        "You may not do that here: type help to see what you may."
    );
}

#[test]
fn the_door_shows_a_refusal_never_the_error() {
    let w = World::new();
    let door = w.door();
    for (line, sentence) in [
        (
            "go north",
            "There is no exit north from the kitchen: look to see the exits.",
        ),
        (
            "go sideways",
            "\"sideways\" is not a direction: say north, south, east, west, up or down.",
        ),
        // A direction is a name of the realm's, and keeps its case.
        (
            "go East",
            "\"East\" is not a direction: say north, south, east, west, up or down.",
        ),
        (
            "take zebra",
            "The zebra is not here: look to see what is in the kitchen.",
        ),
        (
            "take Lamp",
            "\"Lamp\" is not the name of a thing: say it the way look shows it, such as lamp.",
        ),
        (
            "drop lamp",
            "You do not carry the lamp: look to see what you carry.",
        ),
        // A word left out: the REPL sends a sink with nothing after its target as EMPTY
        // content, which the moves read as the word left out.
        (
            "go",
            "That needs one more word: a direction to go, or the thing to take, drop or run a \
             verb on, such as take lamp.",
        ),
        (
            "take",
            "That needs one more word: a direction to go, or the thing to take, drop or run a \
             verb on, such as take lamp.",
        ),
        // A first word the door does not know is a verb a player wrote, and a verb nobody
        // published is refused as one, before its author or its thing is read.
        ("dance", "There is no verb called dance."),
        ("dance lamp", "There is no verb called dance."),
        ("dance Lamp", "There is no verb called dance."),
        (
            "Polish lamp",
            "\"Polish\" is not the name of a verb: say it the way its author published it, \
             such as polish.",
        ),
        // The REPL's own grammar reaches the kernel's refusals, phrased.
        (
            "sink urn:resmud:thing:id:lamp gone",
            "That cannot be done to it: type help to see what you can do here.",
        ),
        (
            "source urn:file:thing/lamp.ttl",
            "There is nothing by that name in this realm.",
        ),
        // `look` and a thing: its name is checked as `take` checks it.
        (
            "look Lamp",
            "\"Lamp\" is not the name of a thing: say it the way look shows it, such as lamp.",
        ),
        ("look zebra", "There is nothing by that name in this realm."),
        ("look at lamp", LOOK_AT_ONE),
        // Words the door gives no meaning beyond themselves.
        ("help me", "Say help on its own."),
        ("Help me", "Say help on its own."),
    ] {
        let turn = door.eval(line);
        assert_printable(line, &turn);
        let (said, _) = refused(turn);
        assert_eq!(said, sentence, "{line}");
    }
    // A line the engine cannot read at all has no error from the kernel to phrase.
    let (sentence, error) = refused(door.eval("source \"urn:resmud:look"));
    assert!(error.is_none(), "{error:?}");
    assert_eq!(
        sentence,
        "That is not something this realm understands: type help to see what you can do here."
    );
}

#[test]
fn the_door_prints_only_plain_text() {
    let w = World::new();
    w.publish("polish", POLISH);
    let door = w.door();
    for line in [
        "look",
        "LOOK",
        "look lamp",
        "look Lamp",
        "look zebra",
        "help",
        "source urn:resmud:view:room:id:kitchen",
        "take lamp",
        "polish lamp",
        "go east",
        "drop lamp",
        "look",
        "go up",
        "take lamp now",
        "conjure",
        "source urn:resmud:look | urn:resmud:look",
        "exists urn:resmud:look",
        "cap",
        "sink urn:host:login urn:cap:root",
        "\"",
        "<urn:resmud:look>",
    ] {
        assert_printable(line, &door.eval(line));
    }
    // A resource that answers Turtle, not text a player reads, is withheld, not printed. (A
    // fresh door: the `login` above narrowed that session to what it named.)
    let door = w.door();
    for line in [
        "source urn:resmud:manifold",
        "source urn:resmud:thing:id:lamp",
        "source urn:resmud:verb:id:polish:signature",
    ] {
        let (sentence, error) = refused(door.eval(line));
        assert_eq!(sentence, WITHHELD, "{line}");
        assert!(error.is_none(), "{line}: {error:?}");
    }
}

#[test]
fn a_door_cannot_widen_its_players_authority() {
    let w = World::new();
    let visitor = Capability::scoped([w.realm.fs_scope("read", "")])
        .with_principal(&resmud::character_iri("alice"))
        .unwrap();
    let door = w.door_as(&visitor, resmud::corridor("alice"));
    // The engine's session commands are not a player's words: `cap` and `login` are verbs
    // no one wrote, and logging in through the REPL's grammar stays under the sealed floor.
    let carried = w.file("carries/alice.ttl");
    door.eval("cap reset");
    door.eval("sink urn:host:login urn:cap:resmud:play");
    let (_, error) = refused(door.eval("take lamp"));
    assert!(matches!(error, Some(Error::Denied(_))), "{error:?}");
    assert_eq!(w.file("carries/alice.ttl"), carried, "nothing was taken");
    assert!(w.file("content/kitchen.ttl").contains("thing:id:lamp"));
}

#[test]
fn serve_is_a_prompt_and_plain_lines() {
    let w = World::new();
    let door: Door = w.door();
    let mut out = Vec::new();
    resmud::door::serve(&door, &b"look\n\ntake lamp\nquit\nlook\n"[..], &mut out).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert_eq!(
        out,
        format!(
            "{PROMPT}A warm kitchen. Bread is cooling on the sill.\n\
             You see the lamp. A brass lamp, dull with age.\n\
             Exits: east.\n\
             You carry nothing.\n\
             {PROMPT}{PROMPT}You take the lamp.\n\
             {PROMPT}"
        ),
        "quit ends the session before the last look"
    );
    // The end of the input ends it too.
    let mut out = Vec::new();
    resmud::door::serve(&door, &b"go east"[..], &mut out).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        format!("{PROMPT}You go east, to the garden.\n{PROMPT}\n")
    );
    let _ = Scope::empty();
}
