//! A generated realm of a few hundred rooms and things, to measure what a move costs the
//! next `look`. Ignored by default (it is a measurement, not a check):
//!
//! ```text
//! cargo test --release --test scale -- --ignored --nocapture
//! ```
//!
//! The realm is generated here, never the repository's sample: `ROOMS` rooms in a line, each
//! with an exit east and west, `THINGS` things spread over them (each room's contents an atom
//! of its own), and alice in the first room.
//!
//! Measured 2026-11-07 on an Apple M5 Max (18 cores, 128 GB), release build, 300 rooms and
//! 300 things, medians of 9 rounds. Before the contents atoms (each thing's atom said where it
//! was, and a room's contents were a query over the whole world): a look right after a take
//! 46.2 ms, after a drop 46.3 ms, a room elsewhere right after a take 23.1 ms. The numbers
//! after are in `docs/design/smallest-playable.md`, "Decided: option 1".

mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use resmud::ikigai_core::{CacheBound, Lru, Request, Verb};

const ROOMS: usize = 300;
const THINGS: usize = 300;

const PREFIXES: &str = "@prefix rm:   <https://iriref.org/resmud/core#> .\n\
                        @prefix rms:  <https://iriref.org/resmud/space#> .\n\
                        @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\n";

/// Write a realm of `rooms` rooms and `things` things under `root`.
fn generate(root: &Path, rooms: usize, things: usize) {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("realm/first");
    for dir in ["room", "content", "thing", "character", "carries", "author"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    for file in ["realm.ttl", "shape.ttl", "author/alice.ttl"] {
        std::fs::copy(sample.join(file), root.join(file)).unwrap();
    }
    let mut world = String::new();
    for r in 0..rooms {
        let mut exits = Vec::new();
        if r > 0 {
            exits.push(("west", r - 1));
        }
        if r + 1 < rooms {
            exits.push(("east", r + 1));
        }
        let mut ttl = format!(
            "{PREFIXES}<urn:resmud:room:id:r{r}> a rms:Room ;\n    rdfs:label \"room {r}\" ;\n    \
             rm:description \"Room {r} of a long hall.\""
        );
        for (bearing, _) in &exits {
            ttl.push_str(&format!(
                " ;\n    rms:exit <urn:resmud:room:id:r{r}:exit:bearing:{bearing}>"
            ));
        }
        ttl.push_str(" .\n");
        for (bearing, to) in &exits {
            ttl.push_str(&format!(
                "\n<urn:resmud:room:id:r{r}:exit:bearing:{bearing}> a rms:Exit ;\n    \
                 rms:bearing rms:{bearing} ;\n    rms:leadsTo <urn:resmud:room:id:r{to}> .\n"
            ));
        }
        std::fs::write(root.join(format!("room/r{r}.ttl")), ttl).unwrap();
        // What the room contains: every thing whose number is the room's, modulo the rooms.
        let mut content = PREFIXES.to_string();
        for t in (r..things).step_by(rooms) {
            content.push_str(&format!(
                "<urn:resmud:room:id:r{r}> rms:contains <urn:resmud:thing:id:t{t}> .\n"
            ));
        }
        std::fs::write(root.join(format!("content/r{r}.ttl")), content).unwrap();
        world.push_str(&format!(
            "urn:resmud:room:id:r{r}\nurn:resmud:room:id:r{r}:content\n"
        ));
    }
    for t in 0..things {
        let ttl = format!(
            "{PREFIXES}<urn:resmud:thing:id:t{t}> a rm:Item ;\n    rdfs:label \"t{t}\" ;\n    \
             rm:description \"Thing {t}.\" .\n"
        );
        std::fs::write(root.join(format!("thing/t{t}.ttl")), ttl).unwrap();
        world.push_str(&format!("urn:resmud:thing:id:t{t}\n"));
    }
    std::fs::write(root.join("carries/alice.ttl"), PREFIXES).unwrap();
    std::fs::write(
        root.join("character/alice.ttl"),
        format!(
            "{PREFIXES}<urn:resmud:character:id:alice> a rm:Character ;\n    \
             rdfs:label \"Alice\" ;\n    rm:description \"A traveler.\" .\n\n\
             <urn:resmud:room:id:r0> rms:contains <urn:resmud:character:id:alice> .\n"
        ),
    )
    .unwrap();
    world.push_str("urn:resmud:character:id:alice\nurn:resmud:character:id:alice:carries\n");
    std::fs::write(root.join("world.txt"), world).unwrap();
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let out = f();
    (out, start.elapsed())
}

fn median(mut xs: Vec<Duration>) -> Duration {
    xs.sort();
    xs[xs.len() / 2]
}

/// One measurement over `w`: every room's text read (as in a realm where people are looking
/// everywhere), then alice's look, a take, her look, a room far from the move, a drop and her
/// look again, `rounds` times. Prints the medians under `label`.
fn measure(label: &str, w: &World, rounds: usize) {
    let view = |room: usize| {
        let request = Request::new(
            Verb::Source,
            iri(&format!("urn:resmud:view:room:id:r{room}")),
        );
        w.text(w.issue(request, &w.alice(), resmud::corridor("alice")));
    };
    // A sanity check: the first room lists the thing generated into it.
    let look = w.play("source urn:resmud:look").unwrap();
    assert!(look.contains("You see the t0."), "{look}");

    let (mut warm, mut after_take, mut after_drop, mut elsewhere) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut entries = 0;
    for _ in 0..rounds {
        for room in 0..ROOMS {
            view(room);
        }
        entries = entries.max(w.kernel.cache_len());
        warm.push(timed(|| w.play("source urn:resmud:look").unwrap()).1);
        w.play("sink urn:resmud:verb:name:take t0").unwrap();
        after_take.push(timed(|| w.play("source urn:resmud:look").unwrap()).1);
        elsewhere.push(timed(|| view(ROOMS / 2)).1);
        w.play("sink urn:resmud:verb:name:drop t0").unwrap();
        after_drop.push(timed(|| w.play("source urn:resmud:look").unwrap()).1);
    }
    println!(
        "{label}: rooms={ROOMS} things={THINGS} rounds={rounds} (medians), \
         most cache entries after a scan: {entries}\n  \
         look, nothing moved:          {:?}\n  \
         look right after a take:      {:?}\n  \
         look right after a drop:      {:?}\n  \
         a room elsewhere, after take: {:?}",
        median(warm),
        median(after_take),
        median(after_drop),
        median(elsewhere),
    );
}

#[test]
#[ignore = "a measurement: run with --release --ignored --nocapture"]
fn a_look_after_a_move_in_a_generated_realm() {
    // The kernel as a host builds it, with ikigai-core's default cache bound (4096 entries),
    // which a scan of every room can overflow, so some of what is measured is eviction.
    let w = World::prepared(|root| generate(root, ROOMS, THINGS));
    measure("default cache bound", &w, 9);
    // The same realm on a kernel whose cache evicts nothing, so what is measured is only what
    // a move's cut threads recompute.
    let mut w = World::prepared(|root| generate(root, ROOMS, THINGS));
    w.kernel = Arc::new(
        resmud::kernel(&w.realm, vec![])
            .unwrap()
            .with_cache_policy(Arc::new(Lru::with_bound(CacheBound::new(1 << 20, 4 << 30)))),
    );
    measure("unbounded cache", &w, 9);
}
