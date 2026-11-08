//! A take or a drop writes two atoms, the one the thing leaves and the one it joins, and files
//! have no transaction. These tests pin what that costs: a failure between the two writes
//! leaves the thing in BOTH places (never in neither) and says so, and two players taking one
//! thing at once end with exactly one of them holding it.

mod common;

use std::sync::{Arc, Barrier};

use common::*;
use futures::executor::block_on;
use resmud::ikigai_core::{Capability, Error};

/// Whether the realm file `rel` lists the lamp.
fn lists_lamp(w: &World, rel: &str) -> bool {
    w.file(rel).contains("urn:resmud:thing:id:lamp")
}

#[test]
fn a_failure_between_the_two_writes_leaves_the_thing_in_both_places_and_says_so() {
    let w = World::new();
    // Alice's capability, less writing the rooms' contents: the first write of a take (what
    // she carries) succeeds, and the second (taking the lamp out of the kitchen) fails.
    let contents = w.realm.fs_scope("write", "content");
    let half = Capability::scoped(
        w.alice()
            .scopes()
            .unwrap()
            .iter()
            .filter(|s| **s != contents)
            .cloned(),
    );
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &half,
        resmud::corridor("alice"),
    );
    match &refused {
        Err(Error::Conflict(said)) => {
            assert!(resmud::is_plain(said), "{said}");
            assert_eq!(
                resmud::refusal(refused.as_ref().unwrap_err()),
                "The move did not finish: the lamp is in two places now, with you and in the \
                 kitchen; drop it to put it right."
            );
        }
        other => panic!("expected the unfinished move to be told, got {other:?}"),
    }
    // Listed twice, never nowhere: the destination was written first.
    assert!(lists_lamp(&w, "carries/alice.ttl"), "alice carries it");
    assert!(
        lists_lamp(&w, "content/kitchen.ttl"),
        "the kitchen still lists it"
    );
    assert_eq!(
        w.play("source urn:resmud:look").unwrap(),
        "A warm kitchen. Bread is cooling on the sill.\n\
         You see the lamp. A brass lamp, dull with age.\n\
         Exits: east.\n\
         You carry the lamp. A brass lamp, dull with age."
    );
    // And the word the refusal names puts it right.
    assert_eq!(
        w.play("sink urn:resmud:verb:name:drop lamp").unwrap(),
        "You drop the lamp."
    );
    assert!(!lists_lamp(&w, "carries/alice.ttl"));
    assert!(lists_lamp(&w, "content/kitchen.ttl"));
}

#[test]
fn a_destination_that_cannot_be_written_moves_nothing() {
    let w = World::new();
    // Less writing what characters carry: the FIRST write fails, so nothing was written.
    let carried = w.realm.fs_scope("write", "carries");
    let cap = Capability::scoped(
        w.alice()
            .scopes()
            .unwrap()
            .iter()
            .filter(|s| **s != carried)
            .cloned(),
    );
    let (kitchen, mine) = (w.file("content/kitchen.ttl"), w.file("carries/alice.ttl"));
    let refused = w.play_as(
        "sink urn:resmud:verb:name:take lamp",
        &cap,
        resmud::corridor("alice"),
    );
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(w.file("content/kitchen.ttl"), kitchen);
    assert_eq!(w.file("carries/alice.ttl"), mine);
}

#[test]
fn two_players_taking_one_thing_at_once_leave_exactly_one_holding_it() {
    let w = World::new();
    // A second player, bob, in the kitchen with alice.
    std::fs::write(
        w.root().join("character/bob.ttl"),
        "@prefix rm: <https://iriref.org/resmud/core#> .\n\
         @prefix rms: <https://iriref.org/resmud/space#> .\n\
         @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
         <urn:resmud:character:id:bob> a rm:Character ; rdfs:label \"Bob\" ;\n    \
         rm:description \"A cook.\" .\n\
         <urn:resmud:room:id:kitchen> rms:contains <urn:resmud:character:id:bob> .\n",
    )
    .unwrap();
    std::fs::write(
        w.root().join("carries/bob.ttl"),
        "@prefix rm: <https://iriref.org/resmud/core#> .\n",
    )
    .unwrap();
    let players = [
        ("alice", w.alice()),
        ("bob", resmud::player(&w.realm, "bob")),
    ];
    for round in 0..12 {
        let barrier = Arc::new(Barrier::new(players.len()));
        let answers: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = players
                .iter()
                .map(|(who, cap)| {
                    let (barrier, kernel) = (Arc::clone(&barrier), Arc::clone(&w.kernel));
                    s.spawn(move || {
                        barrier.wait();
                        let answer = block_on(kernel.issue_in(
                            repl("sink urn:resmud:verb:name:take lamp"),
                            cap,
                            resmud::corridor(who),
                        ));
                        (*who, answer)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let winners: Vec<_> = answers
            .iter()
            .filter(|(_, answer)| answer.is_ok())
            .map(|(who, _)| *who)
            .collect();
        assert_eq!(winners.len(), 1, "round {round}: {answers:?}");
        for (who, answer) in &answers {
            if let Err(e) = answer {
                // The loser is refused in plain words, and holds nothing.
                assert!(
                    matches!(e, Error::Conflict(_)),
                    "round {round}: {who}: {e:?}"
                );
                assert!(resmud::is_plain(&resmud::refusal(e)), "{e:?}");
                assert!(
                    !lists_lamp(&w, &format!("carries/{who}.ttl")),
                    "round {round}"
                );
            }
        }
        let winner = winners[0];
        assert!(
            lists_lamp(&w, &format!("carries/{winner}.ttl")),
            "round {round}"
        );
        assert!(!lists_lamp(&w, "content/kitchen.ttl"), "round {round}");
        // The winner puts it back for the next round.
        let cap = &players.iter().find(|(who, _)| *who == winner).unwrap().1;
        w.play_as(
            "sink urn:resmud:verb:name:drop lamp",
            cap,
            resmud::corridor(winner),
        )
        .unwrap();
        assert!(lists_lamp(&w, "content/kitchen.ttl"), "round {round}");
    }
}
