//! `look` and a name describes only what the looker can see (resmud#56): the room it is in, what
//! that room contains, the characters there and what it carries. Anything else is refused
//! exactly as a name that exists nowhere is, so `look` never reveals that a thing exists
//! elsewhere in the realm.

mod common;

use std::sync::Arc;

use common::*;
use resmud::door::Turn;
use resmud::ikigai_core::{
    Alias, AliasTable, EndpointSpace, Error, Exact, FnEndpoint, ReprType, Representation, Request,
    Verb,
};

const LAMP: &str = "A brass lamp, dull with age.";
const KITCHEN: &str = "A warm kitchen. Bread is cooling on the sill.";
const GARDEN: &str = "A small walled garden. Mint grows between the stones.";
const ALICE: &str = "A traveler with ink on her fingers.";
const BOB: &str = "A cook with flour on his apron.";

/// The answer a door printed for `line`, or a panic naming the refusal.
fn said(door: &resmud::door::Door, line: &str) -> String {
    match door.eval(line) {
        Turn::Said(text) => text,
        other => panic!("{line}: expected an answer, got {other:?}"),
    }
}

/// The sentence and the typed error of the refusal a door printed for `line`.
fn refused(door: &resmud::door::Door, line: &str) -> (String, Option<Error>) {
    match door.eval(line) {
        Turn::Refused { said, error } => (said, error),
        other => panic!("{line}: expected a refusal, got {other:?}"),
    }
}

/// The `first` realm with a second character, bob, in `room`.
fn with_bob(room: &str) -> World {
    let w = World::new();
    std::fs::write(
        w.root().join("character/bob.ttl"),
        format!(
            "@prefix rm:   <https://iriref.org/resmud/core#> .\n\
             @prefix rms:  <https://iriref.org/resmud/space#> .\n\
             @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
             <urn:resmud:character:id:bob> a rm:Character ; rdfs:label \"Bob\" ;\n    \
             rm:description \"{BOB}\" .\n\
             <urn:resmud:room:id:{room}> rms:contains <urn:resmud:character:id:bob> .\n"
        ),
    )
    .unwrap();
    let world = w.file("world.txt");
    std::fs::write(
        w.root().join("world.txt"),
        format!("{world}\nurn:resmud:character:id:bob\n"),
    )
    .unwrap();
    w
}

/// The `first` realm with no lamp in it at all.
fn without_lamp() -> World {
    let w = World::new();
    std::fs::remove_file(w.root().join("thing/lamp.ttl")).unwrap();
    std::fs::remove_file(w.root().join("content/kitchen.ttl")).unwrap();
    let world: Vec<String> = w
        .file("world.txt")
        .lines()
        .filter(|line| *line != "urn:resmud:thing:id:lamp")
        .map(str::to_string)
        .collect();
    std::fs::write(w.root().join("world.txt"), world.join("\n")).unwrap();
    w
}

#[test]
fn a_thing_in_another_room_answers_as_a_name_that_exists_nowhere_does() {
    let elsewhere = World::new();
    let nowhere = without_lamp();
    for w in [&elsewhere, &nowhere] {
        assert_eq!(said(&w.door(), "go east"), "You go east, to the garden.");
    }
    let lamp_elsewhere = refused(&elsewhere.door(), "look lamp");
    let lamp_nowhere = refused(&nowhere.door(), "look lamp");
    assert_eq!(lamp_elsewhere.0, "You see no lamp here.");
    assert_eq!(
        lamp_elsewhere.0.as_bytes(),
        lamp_nowhere.0.as_bytes(),
        "byte for byte the same"
    );
    assert!(matches!(lamp_elsewhere.1, Some(Error::NotFound(_))));
    assert_eq!(
        format!("{:?}", lamp_elsewhere.1),
        format!("{:?}", lamp_nowhere.1),
        "the typed error reveals nothing either"
    );
}

#[test]
fn things_here_carried_things_and_the_room_can_be_looked_at() {
    let w = World::new();
    let door = w.door();
    assert_eq!(said(&door, "look lamp"), LAMP);
    assert_eq!(said(&door, "look kitchen"), KITCHEN);
    // Carried things are with the looker, wherever it goes.
    said(&door, "take lamp");
    assert_eq!(said(&door, "look lamp"), LAMP);
    said(&door, "go east");
    assert_eq!(
        said(&door, "look lamp"),
        LAMP,
        "the carried lamp came along"
    );
    assert_eq!(said(&door, "look garden"), GARDEN);
    // The room left behind, and the thing left in it, are out of sight.
    assert_eq!(refused(&door, "look kitchen").0, "You see no kitchen here.");
    said(&door, "drop lamp");
    assert_eq!(
        said(&door, "look lamp"),
        LAMP,
        "dropped here, so still here"
    );
    said(&door, "go west");
    assert_eq!(refused(&door, "look lamp").0, "You see no lamp here.");
    assert_eq!(said(&door, "look kitchen"), KITCHEN);
}

#[test]
fn a_character_present_can_be_looked_at_and_one_elsewhere_cannot() {
    let w = with_bob("kitchen");
    let door = w.door();
    assert_eq!(said(&door, "look bob"), BOB);
    assert_eq!(
        said(&door, "look alice"),
        ALICE,
        "the looker is present too"
    );
    said(&door, "go east");
    assert_eq!(refused(&door, "look bob").0, "You see no bob here.");
    assert_eq!(said(&door, "look alice"), ALICE);
}

#[test]
fn what_a_character_sees_is_one_resource_naming_what_is_with_it() {
    let w = with_bob("kitchen");
    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    assert_eq!(
        w.play("source urn:resmud:character:id:alice:sees").unwrap(),
        "urn:resmud:character:id:alice\n\
         urn:resmud:character:id:bob\n\
         urn:resmud:room:id:kitchen\n\
         urn:resmud:thing:id:lamp"
    );
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    assert_eq!(
        w.play("source urn:resmud:character:id:bob:sees").unwrap(),
        "urn:resmud:character:id:bob\nurn:resmud:room:id:kitchen",
        "the lamp went with alice"
    );
}

#[test]
fn a_take_or_a_drop_recomputes_only_what_read_the_atoms_it_wrote() {
    let w = with_bob("garden");
    let scope = || resmud::corridor("alice");
    let alice = w.alice();
    let sees = |who: &str| {
        Request::new(
            Verb::Source,
            iri(&format!("urn:resmud:character:id:{who}:sees")),
        )
    };
    let read = |who: &str| {
        w.issue(sees(who), &alice, scope()).unwrap();
    };
    let cached = |who: &str| w.kernel.is_cached_in(&sees(who), &alice, &scope());
    read("alice");
    read("bob");
    assert!(
        cached("alice") && cached("bob"),
        "what a character sees is cached"
    );

    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    assert!(
        !cached("alice"),
        "the take cut the kitchen and alice's carries"
    );
    assert!(cached("bob"), "nothing bob sees was written");
    read("alice");
    assert!(cached("alice"));

    w.play("sink urn:resmud:verb:name:drop lamp").unwrap();
    assert!(!cached("alice"), "the drop cut them again");
    assert!(cached("bob"), "bob is in another room, carrying nothing");
    read("alice");

    // A character's place is said by its own atom alone, so who is here reads every
    // character's atom: a go recomputes what each character sees, even one elsewhere.
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    assert!(!cached("alice"));
    assert!(!cached("bob"));
}

#[test]
fn a_corridor_that_rebinds_what_a_character_sees_changes_what_it_can_look_at() {
    let w = World::new();
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    let eye = EndpointSpace::new().bind(
        Exact::new("urn:resmud:character:id:alice:sees"),
        FnEndpoint::new("test-magic-eye", |_| {
            Ok(Representation::new(
                ReprType::new("text/plain").with_param("charset", "utf-8"),
                b"urn:resmud:room:id:garden\nurn:resmud:thing:id:lamp".to_vec(),
            ))
        }),
    );
    let corridor = resmud::corridor("alice").with_named(
        iri("urn:resmud:corridor:name:magic-eye"),
        Arc::new(Alias::new(Arc::new(AliasTable::new()), Arc::new(eye))),
    );
    let door = w.door_as(&w.alice(), corridor);
    assert_eq!(
        said(&door, "look lamp"),
        LAMP,
        "the eye sees the kitchen's lamp"
    );
}

#[test]
fn looking_answers_only_in_plain_text() {
    let w = with_bob("kitchen");
    let door = w.door();
    for line in ["look lamp", "look kitchen", "look bob", "look zebra"] {
        let turn = door.eval(line);
        let text = turn.text().unwrap();
        assert!(resmud::is_plain(text), "{line}: {text:?}");
    }
}

/// What a raw REPL line answers for a name the looker does not see is what `look` answers for
/// it, the same sentence and the same typed error, whether the name exists elsewhere or nowhere
/// (resmud#63).
#[test]
fn a_raw_read_of_a_thing_elsewhere_answers_as_look_does() {
    let elsewhere = World::new();
    let nowhere = without_lamp();
    for w in [&elsewhere, &nowhere] {
        assert_eq!(said(&w.door(), "go east"), "You go east, to the garden.");
    }
    for line in [
        "source urn:resmud:thing:id:lamp:description",
        "source urn:resmud:thing:id:lamp",
        "exists urn:resmud:thing:id:lamp",
        "sink urn:resmud:thing:id:lamp gone",
    ] {
        let (door, ghost) = (elsewhere.door(), nowhere.door());
        let there = refused(&door, line);
        let look = refused(&door, "look lamp");
        assert_eq!(there.0, "You see no lamp here.", "{line}");
        assert_eq!(there.0, look.0, "{line}");
        assert_eq!(format!("{:?}", there.1), format!("{:?}", look.1), "{line}");
        let gone = refused(&ghost, line);
        assert_eq!(there.0.as_bytes(), gone.0.as_bytes(), "{line}");
        assert_eq!(format!("{:?}", there.1), format!("{:?}", gone.1), "{line}");
    }
}

#[test]
fn a_raw_read_of_what_is_with_the_looker_still_answers() {
    let w = with_bob("kitchen");
    let door = w.door();
    for (line, answer) in [
        ("source urn:resmud:thing:id:lamp:description", LAMP),
        ("source urn:resmud:room:id:kitchen:description", KITCHEN),
        ("source urn:resmud:character:id:bob:description", BOB),
        ("source urn:resmud:character:id:alice:description", ALICE),
        ("source urn:resmud:character:id:alice:place", "kitchen"),
        ("exists urn:resmud:thing:id:lamp", "true"),
    ] {
        assert_eq!(said(&door, line), answer, "{line}");
    }
    let view = said(&door, "source urn:resmud:view:room:id:kitchen");
    assert!(view.starts_with(KITCHEN), "{view}");
    // A carried thing is with the looker wherever it goes, so its raw name goes along.
    said(&door, "take lamp");
    said(&door, "go east");
    assert_eq!(
        said(&door, "source urn:resmud:thing:id:lamp:description"),
        LAMP
    );
}

#[test]
fn a_raw_read_of_a_room_or_a_character_elsewhere_is_refused_as_look_refuses_it() {
    let w = with_bob("kitchen");
    let door = w.door();
    said(&door, "go east");
    for (line, name) in [
        ("source urn:resmud:room:id:kitchen:description", "kitchen"),
        ("source urn:resmud:room:id:kitchen:content:line", "kitchen"),
        ("source urn:resmud:room:id:kitchen:exit", "kitchen"),
        ("source urn:resmud:view:room:id:kitchen", "kitchen"),
        ("source urn:resmud:character:id:bob:description", "bob"),
        ("source urn:resmud:character:id:bob:place", "bob"),
        ("source urn:resmud:character:id:bob:inventory", "bob"),
        ("source urn:resmud:character:id:bob:look", "bob"),
        ("source urn:resmud:character:id:zebra:look", "zebra"),
    ] {
        let (sentence, error) = refused(&door, line);
        assert_eq!(sentence, format!("You see no {name} here."), "{line}");
        assert!(
            matches!(error, Some(Error::NotFound(_))),
            "{line}: {error:?}"
        );
    }
}

#[test]
fn seeing_a_character_does_not_read_what_it_carries_or_where_it_looks() {
    let w = with_bob("kitchen");
    let door = w.door();
    for line in [
        "source urn:resmud:character:id:bob:inventory",
        "source urn:resmud:character:id:bob:carries",
        "source urn:resmud:character:id:bob:sees",
        "source urn:resmud:character:id:bob:look",
        "source urn:resmud:character:id:bob:look:name:lamp",
    ] {
        let (sentence, _) = refused(&door, line);
        assert_eq!(
            sentence, "There is nothing by that name in this realm.",
            "{line}"
        );
    }
}

/// `urn:resmud:seen` takes the list it judges by as an argument, and the published modules read
/// any name they are given, so none of these may be reached from a prompt.
#[test]
fn no_raw_line_reaches_a_thing_out_of_sight_by_another_road() {
    let w = World::new();
    let door = w.door();
    said(&door, "go east");
    for line in [
        "source urn:resmud:seen name=lamp sees=urn:resmud:thing:id:lamp",
        "source urn:resmud:query query=urn:resmud:query:id:description \
         subject=urn:resmud:thing:id:lamp atom=urn:resmud:thing:id:lamp",
        "source urn:sparql:select graph=urn:resmud:thing:id:lamp \
         \"query=SELECT ?o WHERE { ?s ?p ?o }\"",
        "source urn:resmud:world",
        "source urn:kernel:actions",
        // A reference is read for its answer before the endpoint runs, so a refusal that
        // echoes the word it was given would print the description.
        "sink urn:resmud:verb:name:take content=<urn:resmud:thing:id:lamp:description>",
        "sink urn:resmud:verb:name:go content=<urn:resmud:thing:id:lamp:description>",
        "source urn:resmud:look x=<urn:resmud:thing:id:lamp:description>",
        // Through a pipe.
        "source urn:resmud:thing:id:lamp:description | sink urn:resmud:verb:name:take",
        // An argument the binding fixes stays the binding's.
        "source urn:resmud:look:name:lamp sees=urn:resmud:thing:id:lamp",
        "source urn:resmud:character:id:alice:look:name:lamp sees=urn:resmud:thing:id:lamp",
    ] {
        let turn = door.eval(line);
        let text = turn.text().unwrap_or_default().to_string();
        assert!(!text.contains("brass"), "{line}: {text}");
        assert!(matches!(turn, Turn::Refused { .. }), "{line}: {turn:?}");
        assert!(resmud::is_plain(&text), "{line}: {text}");
    }
}

/// A name passed by reference is read for its answer before the endpoint runs, so the gate
/// judges it as it judges a target, and answers as `look` does.
#[test]
fn a_reference_to_a_thing_out_of_sight_is_refused_as_look_refuses_it() {
    let w = World::new();
    let door = w.door();
    said(&door, "go east");
    assert_eq!(
        refused(
            &door,
            "source urn:resmud:look x=<urn:resmud:thing:id:lamp:description>"
        )
        .0,
        "You see no lamp here."
    );
}

/// The rule is the door's, for a player: the host that runs the realm reads its own kernel.
#[test]
fn the_operator_reads_the_realm_on_the_kernel_not_through_a_players_door() {
    let w = World::new();
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    assert_eq!(
        w.play("source urn:resmud:thing:id:lamp:description")
            .unwrap(),
        LAMP
    );
    let root = Request::new(Verb::Source, iri("urn:resmud:room:id:kitchen:content:line"));
    let line = w.text(w.issue(
        root,
        &resmud::ikigai_core::Capability::root(),
        resmud::ikigai_core::Scope::empty(),
    ));
    assert!(line.contains(LAMP), "{line}");
}

#[test]
fn a_corridor_that_widens_what_a_character_sees_widens_its_raw_reads_too() {
    let w = World::new();
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    let eye = EndpointSpace::new().bind(
        Exact::new("urn:resmud:character:id:alice:sees"),
        FnEndpoint::new("test-magic-eye", |_| {
            Ok(Representation::new(
                ReprType::new("text/plain").with_param("charset", "utf-8"),
                b"urn:resmud:room:id:garden\nurn:resmud:thing:id:lamp".to_vec(),
            ))
        }),
    );
    let corridor = resmud::corridor("alice").with_named(
        iri("urn:resmud:corridor:name:magic-eye"),
        Arc::new(Alias::new(Arc::new(AliasTable::new()), Arc::new(eye))),
    );
    let door = w.door_as(&w.alice(), corridor);
    assert_eq!(
        said(&door, "source urn:resmud:thing:id:lamp:description"),
        LAMP,
        "what the eye sees, the raw name reads"
    );
    assert_eq!(
        refused(&door, "source urn:resmud:room:id:kitchen:description").0,
        "You see no kitchen here.",
        "the eye sees the lamp, not the room it lies in"
    );
}
