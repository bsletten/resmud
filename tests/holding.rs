//! Where things are, when a realm keeps no atom for it (resmud#57).
//!
//! A room's contents (`content/{room}.ttl`) and what a character carries (`carries/{c}.ttl`)
//! are atoms of their own. A realm written by hand, or one from before those atoms existed,
//! can lack one. Two rules cover it, each where the other cannot:
//!
//! - a missing holding atom READS as empty, on that atom's own NotFound only, so the answer
//!   hangs from the atom's thread and the first write to it recomputes whatever read it;
//! - creating a room or a character CREATES its holding atom, and so does the first take or
//!   drop into one that has none, so a realm the game itself writes never depends on the
//!   fallback.

mod common;

use common::*;
use resmud::ikigai_core::{Error, Request, Verb};

/// The `first` realm with one of its files removed.
fn without(rel: &str) -> World {
    let w = World::new();
    std::fs::remove_file(w.root().join(rel)).unwrap();
    w
}

#[test]
fn a_room_with_no_contents_atom_reads_as_empty_and_the_first_drop_creates_it() {
    let w = without("content/garden.ttl");
    w.play("sink urn:resmud:verb:name:take lamp").unwrap();
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A small walled garden. Mint grows between the stones.\n\
         Nothing here can be taken.\n\
         Exits: west.\n\
         You carry the lamp. A brass lamp, dull with age."
    );
    let garden = || Request::new(Verb::Source, iri("urn:resmud:view:room:id:garden"));
    assert!(
        w.kernel
            .is_cached_in(&garden(), &w.alice(), &resmud::corridor("alice")),
        "the empty garden is cached, hanging from its contents atom's thread"
    );

    // Dropping the lamp here creates the garden's contents atom, and the write cuts the
    // thread the empty answer hung from: the garden recomputes and lists the lamp.
    assert_eq!(
        w.play("sink urn:resmud:verb:name:drop lamp").unwrap(),
        "You drop the lamp."
    );
    assert!(w.file("content/garden.ttl").contains("thing:id:lamp"));
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A small walled garden. Mint grows between the stones.\n\
         You see the lamp. A brass lamp, dull with age.\n\
         Exits: west.\n\
         You carry nothing."
    );
    assert_eq!(
        w.play("sink urn:resmud:verb:name:take lamp").unwrap(),
        "You take the lamp."
    );
    assert!(!w.file("content/garden.ttl").contains("thing:id:lamp"));
}

#[test]
fn taking_from_a_room_with_no_contents_atom_says_the_thing_is_not_here() {
    let w = without("content/garden.ttl");
    w.play("sink urn:resmud:verb:name:go east").unwrap();
    let refused = w.play("sink urn:resmud:verb:name:take lamp");
    match &refused {
        Err(Error::Conflict(said)) => assert_eq!(
            said,
            "the lamp is not here: look to see what is in the garden"
        ),
        other => panic!("expected a Conflict, got {other:?}"),
    }
    assert!(
        !w.root().join("content/garden.ttl").exists(),
        "a refused take writes nothing"
    );
}

#[test]
fn a_character_with_no_carries_atom_carries_nothing_and_the_first_take_creates_it() {
    let w = without("carries/alice.ttl");
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A warm kitchen. Bread is cooling on the sill.\n\
         You see the lamp. A brass lamp, dull with age.\n\
         Exits: east.\n\
         You carry nothing."
    );
    let refused = w.play("sink urn:resmud:verb:name:drop lamp");
    match &refused {
        Err(Error::Conflict(said)) => assert_eq!(
            said,
            "you do not carry the lamp: look to see what you carry"
        ),
        other => panic!("expected a Conflict, got {other:?}"),
    }
    assert_eq!(
        w.play("sink urn:resmud:verb:name:take lamp").unwrap(),
        "You take the lamp."
    );
    assert!(w.file("carries/alice.ttl").contains("thing:id:lamp"));
    assert!(!w.file("content/kitchen.ttl").contains("thing:id:lamp"));
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A warm kitchen. Bread is cooling on the sill.\n\
         Nothing here can be taken.\n\
         Exits: east.\n\
         You carry the lamp. A brass lamp, dull with age."
    );
}

#[test]
fn only_the_holding_atoms_own_absence_reads_as_empty() {
    let w = World::new();
    // A room the realm has no atom for is still not there: its text reads the room's own
    // atom, whose NotFound is not a holding atom's, and fails.
    let refused = w.play("source urn:resmud:view:room:id:cellar");
    assert!(
        is(&refused, |e| matches!(e, Error::NotFound(_))),
        "{refused:?}"
    );
    // A thing the realm has no atom for is not there either.
    let refused = w.play("source urn:resmud:thing:id:pebble:description");
    assert!(
        is(&refused, |e| matches!(e, Error::NotFound(_))),
        "{refused:?}"
    );
    // A reader who may read the rooms but not what they contain is refused, never told the
    // kitchen is empty: only the file's absence reads as empty.
    let rooms_only = resmud::ikigai_core::Capability::scoped([w.realm.fs_scope("read", "room")]);
    let refused = w.play_as(
        "source urn:resmud:room:id:kitchen:content",
        &rooms_only,
        resmud::ikigai_core::Scope::empty(),
    );
    assert!(
        is(&refused, |e| matches!(e, Error::Denied(_))),
        "{refused:?}"
    );
}

/// Give alice, as an author, the grant `tokens` (the game's write tokens, by kind).
fn grant(w: &World, kinds: &[&str]) {
    let tokens = kinds
        .iter()
        .map(|kind| format!("\"urn:cap:resmud:write:{kind}\""))
        .collect::<Vec<_>>()
        .join(" , ");
    std::fs::write(
        w.root().join("author/alice.ttl"),
        format!(
            "@prefix rml: <urn:resmud:term:name:> .\n\
             <urn:resmud:author:id:alice> rml:key <urn:resmud:author:id:alice:key> ;\n    \
             rml:grant {tokens} .\n"
        ),
    )
    .unwrap();
}

/// A verb that digs a new room, the cellar.
const DIG: &str = r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#") (rms "https://iriref.org/resmud/space#")
          (rdfs "http://www.w3.org/2000/01/rdf-schema#"))
  ((iri "urn:resmud:room:id:cellar") a rms:Room)
  ((iri "urn:resmud:room:id:cellar") rdfs:label "cellar")
  ((iri "urn:resmud:room:id:cellar") rm:description "A cold cellar. Jars line the shelves."))"#;

#[test]
fn a_verb_that_creates_a_room_creates_its_contents_atom() {
    let w = World::new();
    grant(&w, &["thing", "room", "content"]);
    w.publish("dig", DIG);
    assert_eq!(w.play("sink urn:resmud:verb:id:dig").unwrap(), "You dig.");
    assert!(w.root().join("room/cellar.ttl").exists());
    assert!(
        w.root().join("content/cellar.ttl").exists(),
        "the new room has a contents atom of its own"
    );
    let world = w.file("world.txt");
    assert!(world.lines().any(|l| l == "urn:resmud:room:id:cellar"));
    assert!(world
        .lines()
        .any(|l| l == "urn:resmud:room:id:cellar:content"));
    assert_eq!(
        w.play("source urn:resmud:view:room:id:cellar").unwrap(),
        "A cold cellar. Jars line the shelves.\n\
         Nothing here can be taken.\n\
         There is no way out."
    );
}

#[test]
fn a_verb_that_creates_a_room_and_places_a_thing_there_writes_the_thing_into_its_contents() {
    let w = World::new();
    grant(&w, &["thing", "room", "content"]);
    // DIG without the `)` that closes its graph, and three more triples.
    let dig_and_stock = DIG.strip_suffix(')').unwrap().to_string()
        + r#"
  ((iri "urn:resmud:thing:id:jar") a rm:Item)
  ((iri "urn:resmud:thing:id:jar") rdfs:label "jar")
  ((iri "urn:resmud:thing:id:jar") rm:description "A jar of plum jam.")
  ((iri "urn:resmud:room:id:cellar") rms:contains (iri "urn:resmud:thing:id:jar")))"#;
    w.publish("dig-and-stock", &dig_and_stock);
    w.play("sink urn:resmud:verb:id:dig-and-stock").unwrap();
    assert!(w.file("content/cellar.ttl").contains("thing:id:jar"));
    assert_eq!(
        w.play("source urn:resmud:view:room:id:cellar").unwrap(),
        "A cold cellar. Jars line the shelves.\n\
         You see the jar. A jar of plum jam.\n\
         There is no way out."
    );
}

#[test]
fn a_verb_that_creates_a_character_creates_its_carries_atom() {
    let w = World::new();
    grant(&w, &["character", "carries"]);
    w.publish(
        "hire",
        r#"(graph
  (prefix (rm "https://iriref.org/resmud/core#") (rdfs "http://www.w3.org/2000/01/rdf-schema#"))
  ((iri "urn:resmud:character:id:bob") a rm:Character)
  ((iri "urn:resmud:character:id:bob") rdfs:label "Bob")
  ((iri "urn:resmud:character:id:bob") rm:description "A cook."))"#,
    );
    assert_eq!(w.play("sink urn:resmud:verb:id:hire").unwrap(), "You hire.");
    assert!(w.root().join("character/bob.ttl").exists());
    assert!(
        w.root().join("carries/bob.ttl").exists(),
        "the new character has a carries atom of its own"
    );
    assert!(w
        .file("world.txt")
        .lines()
        .any(|l| l == "urn:resmud:character:id:bob:carries"));
    assert_eq!(
        w.play("source urn:resmud:character:id:bob:inventory")
            .unwrap(),
        "You carry nothing."
    );
}

#[test]
fn creating_a_room_needs_a_grant_for_its_contents_too() {
    let w = World::new();
    // Rooms, but not what a room contains: the new room's contents atom is a write the grant
    // does not cover, so the verb is refused before anything is written.
    grant(&w, &["thing", "room"]);
    w.publish("dig", DIG);
    let refused = w.play("sink urn:resmud:verb:id:dig");
    match &refused {
        Err(Error::Denied(said)) => assert_eq!(
            said,
            "the verb dig writes the cellar's contents, and it runs under no more than both \
             alice's grant and yours, which together do not cover writing what a room contains"
        ),
        other => panic!("expected a Denied, got {other:?}"),
    }
    assert!(!w.root().join("room/cellar.ttl").exists());
    assert!(!w.root().join("content/cellar.ttl").exists());
    assert!(!w.file("world.txt").contains("cellar"));
}
