//! `go`, `take` and `drop`: the three moves, each of which rewrites the atoms it touches or
//! refuses.
//!
//! **Why this is code.** Each changes state and can refuse, and a refusal is part of its
//! contract: a generic composer cannot say *no*. Everything else each one needs is a name:
//! the actor's place (`urn:resmud:character:id:{c}:place`), where an exit leads and whether an
//! atom holds a thing (stored queries), and each rewritten atom (the stored `relocate`, `put`
//! and `remove` CONSTRUCTs). The only things an edge does itself are decide, order the writes
//! and write the answers to the atoms' files.
//!
//! **Where things are.** A room's contents are an atom of their own
//! (`urn:resmud:room:id:{room}:content`, the file `content/{room}.ttl`), and so is what a
//! character carries (`urn:resmud:character:id:{c}:carries`, `carries/{c}.ttl`). A thing's own
//! atom says only what it is. A character's place stays in the character's atom, so `go`
//! rewrites one atom. `take` and `drop` rewrite two, the atom the thing leaves and the atom
//! it joins, and the kernel's cut of those two files is what makes the views that read them
//! recompute, and nothing else.
//!
//! **The two writes.** Files have no transaction, so a failure between the writes must be
//! visible, never silent:
//!
//! 1. The destination is written FIRST, then the thing is removed from the source, so the
//!    worst case is a thing listed in two places, never a thing nowhere. A player who meets
//!    it is told so, in words, and told the move that puts it right.
//! 2. Every write carries `if-match`: the content id of the atom as the edge read it, which
//!    `ikigai-fs` (0.4.1) checks under a lock and refuses with `Conflict` when the file has
//!    changed since. A destination that changed is read again and the move retried, a few
//!    times; nothing has been written yet. A source that changed is read again too, and if the
//!    thing is still there it is removed from the new version. If it is not, someone else moved
//!    it first: the thing is taken back out of the destination, and the move is refused as
//!    a lost race.
//!
//! | case | error |
//! |---|---|
//! | a caller with no character (no principal) | `Denied` |
//! | no direction or thing given, or a blank one | `MissingArgument` |
//! | `go` a bearing that is not a direction | `InvalidArgument` |
//! | `take`/`drop` a thing whose name is not a key | `InvalidArgument` |
//! | `go` where there is no exit that way | `Conflict` |
//! | `take` a thing that is not here, or does not exist | `Conflict` |
//! | `drop` a thing the actor does not carry | `Conflict` |
//! | `take`/`drop` that loses a race for the thing, or for an atom that kept changing | `Conflict` |
//! | `take`/`drop` whose second write failed: the thing is in two places | `Conflict` |
//!
//! Every refusal names things by the word a player sees (`the lamp`, `the kitchen`), never
//! by IRI; a door shows a player [`crate::refusal`] of it.

use ikigai_core::{
    ActionSpec, ArgRef, ArgSpec, AsyncFnEndpoint, Description, Error, FailureClass, Invocation,
    ReprType, Representation, Request, Result, Verb,
};
use serde_json::json;

use crate::query::{construct, select, text, uri};
use crate::{character_iri, iri, is_key, CAP_PLAY, DIRECTIONS, SPACE_VOCABULARY, VOCABULARY};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The refusal for a caller who plays no character of this realm.
const NOT_PLAYING: &str = "you are not playing a character in this realm, so you cannot act in it";

/// The character the caller plays: its principal, which must be a character's name.
pub(crate) fn actor(inv: &Invocation<'_>) -> Result<String> {
    inv.capability
        .principal()
        .and_then(|principal| principal.strip_prefix("urn:resmud:character:id:"))
        .filter(|key| is_key(key))
        .map(str::to_string)
        .ok_or_else(|| Error::Denied(NOT_PLAYING.into()))
}

/// The key of the room `character` is in, read through its `:place` composition. A
/// character the realm has no atom for is not playing in it.
pub(crate) async fn place(inv: &Invocation<'_>, character: &str) -> Result<String> {
    let answer = match inv
        .source(&iri(&format!("{}:place", character_iri(character))))
        .await
    {
        Ok(answer) => answer,
        Err(Error::NotFound(_) | Error::Unresolved(_)) => {
            return Err(Error::Denied(NOT_PLAYING.into()))
        }
        Err(e) => return Err(e),
    };
    let room = String::from_utf8_lossy(&answer.bytes).trim().to_string();
    if is_key(&room) {
        Ok(room)
    } else {
        Err(Error::Conflict(
            "you are nowhere: no room in this realm holds you".into(),
        ))
    }
}

/// The refusal for `raw`, given where the name of a thing was wanted.
pub(crate) fn not_a_thing(raw: &str) -> Error {
    Error::InvalidArgument {
        name: "content".into(),
        detail: format!(
            "\"{raw}\" is not the name of a thing: say it the way look shows it, such as lamp"
        ),
    }
}

/// The content argument, trimmed. A blank one is a word left out (`MissingArgument`), as an
/// absent one is: the `ikigai` REPL sends a `sink` with nothing after its target as EMPTY
/// content, never as none, so `go` alone at the door reaches an edge as `content=""`.
pub(crate) fn word<'a>(inv: &'a Invocation<'_>) -> Result<&'a str> {
    let raw = inv.inline_str("content")?.trim();
    if raw.is_empty() {
        Err(Error::MissingArgument("content".into()))
    } else {
        Ok(raw)
    }
}

/// The content argument as a key (a thing's name), or `InvalidArgument` saying why not.
fn thing_arg(inv: &Invocation<'_>) -> Result<String> {
    let raw = word(inv)?;
    if is_key(raw) {
        Ok(raw.to_string())
    } else {
        Err(not_a_thing(raw))
    }
}

/// Rewrite `atom` (whose subject is `subject`) so that `holder` holds `subject` by `how`,
/// and write it to `file`.
async fn relocate(
    inv: &Invocation<'_>,
    atom: &str,
    holder: &str,
    how: &str,
    file: &str,
) -> Result<()> {
    let turtle = construct(
        inv,
        "relocate",
        atom,
        json!({
            "atom": uri(atom),
            "subject": uri(atom),
            "holder": uri(holder),
            "how": uri(how),
        }),
    )
    .await?;
    let write = Request::new(Verb::Sink, iri(file))
        .with_arg("content", ArgRef::Inline(turtle.into_bytes()));
    inv.issue(write).await?;
    Ok(())
}

/// How many times a move reads an atom again after `if-match` found it changed.
const ATTEMPTS: usize = 4;

/// One atom of where things are: a room's contents, or what a character carries.
struct Holding {
    /// The holder's name: `urn:resmud:room:id:kitchen`, `urn:resmud:character:id:alice`.
    holder: String,
    /// How it holds a thing: `rms:contains` or `rm:carries`, as an IRI.
    how: String,
    /// The atom's public name, which every query reads.
    atom: String,
    /// The atom's file, which the edge writes.
    file: String,
    /// Where a thing it holds is, in a player's words: "in the kitchen", "with you".
    place: String,
}

impl Holding {
    /// What room `room` contains.
    fn room(room: &str) -> Holding {
        let holder = format!("urn:resmud:room:id:{room}");
        Holding {
            atom: format!("{holder}:content"),
            holder,
            how: format!("{SPACE_VOCABULARY}contains"),
            file: format!("urn:file:content/{room}.ttl"),
            place: format!("in the {room}"),
        }
    }

    /// What character `character` carries, said to that character.
    fn carried(character: &str) -> Holding {
        let holder = character_iri(character);
        Holding {
            atom: format!("{holder}:carries"),
            holder,
            how: format!("{VOCABULARY}carries"),
            file: format!("urn:file:carries/{character}.ttl"),
            place: "with you".into(),
        }
    }

    /// The bindings naming this atom and `thing` in it, for `put` and `remove`. A query is
    /// given exactly the variables it names: `urn:sparql:*` refuses any other.
    fn bindings(&self, thing: &str) -> serde_json::Value {
        json!({
            "atom": uri(&self.atom),
            "holder": uri(&self.holder),
            "how": uri(&self.how),
            "thing": uri(thing),
        })
    }
}

/// Whether `at` holds `thing` (a thing's name), read from that one atom.
async fn holds(inv: &Invocation<'_>, at: &Holding, thing: &str) -> Result<bool> {
    let lines = select(
        inv,
        ArgRef::Reference(iri("urn:resmud:query:id:holds")),
        &at.atom,
        json!({ "subject": uri(&at.holder), "how": uri(&at.how), "thing": uri(thing) }),
    )
    .await?;
    Ok(!lines.is_empty())
}

/// The content id of `at`'s file as it is now: the `if-match` its next write carries. It is
/// the id of the bytes alone, which `ikigai-fs` accepts whatever media type it reads the
/// file as.
async fn version(inv: &Invocation<'_>, at: &Holding) -> Result<String> {
    let answer = inv.source(&iri(&at.file)).await.map_err(|e| match e {
        Error::NotFound(_) => Error::Conflict(format!(
            "the realm keeps no list of what is {}, so nothing can move there",
            at.place
        )),
        e => e,
    })?;
    Ok(
        Representation::new(ReprType::new("application/octet-stream"), answer.bytes)
            .content_id()
            .to_string(),
    )
}

/// Rewrite `at` by the stored CONSTRUCT `query` (`put` or `remove`) with `thing`, and write
/// it, but only while the file is still the version `if_match`.
async fn rewrite(
    inv: &Invocation<'_>,
    at: &Holding,
    query: &str,
    thing: &str,
    if_match: &str,
) -> Result<()> {
    let turtle = construct(inv, query, &at.atom, at.bindings(thing)).await?;
    let write = Request::new(Verb::Sink, iri(&at.file))
        .with_arg("content", ArgRef::Inline(turtle.into_bytes()))
        .with_arg("if-match", ArgRef::Inline(if_match.as_bytes().to_vec()));
    inv.issue(write).await?;
    Ok(())
}

/// Put `thing` in `at`, reading it again while `if-match` finds it changed. Nothing else has
/// been written when this fails.
async fn put(inv: &Invocation<'_>, at: &Holding, thing: &str, key: &str) -> Result<()> {
    for _ in 0..ATTEMPTS {
        let id = version(inv, at).await?;
        match rewrite(inv, at, "put", thing, &id).await {
            Err(Error::Conflict(_)) => continue,
            done => return done,
        }
    }
    Err(Error::Conflict(format!(
        "everything {} kept changing while you moved the {key}, so nothing moved: try again",
        at.place
    )))
}

/// Take `thing` out of `at`. `Ok(false)` when it is not there any more (someone else moved
/// it first); an error when the write failed.
async fn remove(inv: &Invocation<'_>, at: &Holding, thing: &str) -> Result<bool> {
    for _ in 0..ATTEMPTS {
        // The version first, then whether the thing is in it: a later version than the one
        // read fails the `if-match` below, so the removal is judged on what it rewrites.
        let id = version(inv, at).await?;
        if !holds(inv, at, thing).await? {
            return Ok(false);
        }
        match rewrite(inv, at, "remove", thing, &id).await {
            Err(Error::Conflict(_)) => continue,
            Err(e) => return Err(e),
            Ok(()) => return Ok(true),
        }
    }
    Err(Error::Conflict("it kept changing".into()))
}

/// Move the thing `key` from `from` to `to`: the destination first, then the source, so a
/// failure between them leaves it in both, never in neither. `mend` is the word that puts a
/// thing listed in both places right (`drop` after a take), and `lost` the refusal when
/// someone else moved it first.
async fn shift(
    inv: &Invocation<'_>,
    key: &str,
    from: &Holding,
    to: &Holding,
    mend: &str,
    lost: String,
) -> Result<()> {
    let thing = format!("urn:resmud:thing:id:{key}");
    put(inv, to, &thing, key).await?;
    let two_places = || {
        Error::Conflict(format!(
            "the move did not finish: the {key} is in two places now, {} and {}; {mend} it to \
             put it right",
            to.place, from.place
        ))
    };
    match remove(inv, from, &thing).await {
        Ok(true) => Ok(()),
        // Someone else moved it first: take it back out of the destination, so a lost race
        // changes nothing.
        Ok(false) => match remove(inv, to, &thing).await {
            Ok(_) => Err(Error::Conflict(lost)),
            Err(_) => Err(two_places()),
        },
        Err(_) => Err(two_places()),
    }
}

fn description(id: &str, summary: &str, content: &str) -> Description {
    Description::new(id)
        .title(id)
        .summary(summary)
        .verb(Verb::Sink)
        .verb(Verb::Meta)
        .action(
            ActionSpec::new(Verb::Sink)
                .summary(summary)
                .requires(CAP_PLAY)
                .input(ArgSpec::new("content").summary(content).class(XSD_STRING))
                .output("text/plain;charset=utf-8")
                .fails(FailureClass::Denied)
                .fails(FailureClass::Permanent),
        )
}

/// `urn:resmud:verb:name:go`: walk through the exit on one bearing (`content`: `east`).
pub fn go() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("resmud-go", |inv| {
        Box::pin(async move {
            let me = actor(inv)?;
            let bearing = word(inv)?.to_string();
            if !DIRECTIONS.contains(&bearing.as_str()) {
                return Err(Error::InvalidArgument {
                    name: "content".into(),
                    detail: format!(
                        "\"{bearing}\" is not a direction: say north, south, east, west, up or down"
                    ),
                });
            }
            let room = place(inv, &me).await?;
            let room_iri = format!("urn:resmud:room:id:{room}");
            let to = select(
                inv,
                ArgRef::Reference(iri("urn:resmud:query:id:exit-to")),
                &room_iri,
                json!({
                    "subject": uri(&room_iri),
                    "bearing": uri(&format!("{SPACE_VOCABULARY}{bearing}")),
                }),
            )
            .await?;
            let Some(to) = to.into_iter().next() else {
                return Err(Error::Conflict(format!(
                    "there is no exit {bearing} from the {room}: look to see the exits"
                )));
            };
            let me_iri = character_iri(&me);
            relocate(
                inv,
                &me_iri,
                &to,
                &format!("{SPACE_VOCABULARY}contains"),
                &format!("urn:file:character/{me}.ttl"),
            )
            .await?;
            let to_key = to.strip_prefix("urn:resmud:room:id:").unwrap_or(&to);
            Ok(text(format!("You go {bearing}, to the {to_key}.")))
        })
    })
    .with_description(description(
        "resmud-go",
        "Walk through the exit on one bearing, out of the room you are in.",
        "the bearing to walk: north, south, east, west, up or down",
    ))
}

/// `urn:resmud:verb:name:take`: pick up a thing that is in your room (`content`: `lamp`).
pub fn take() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("resmud-take", |inv| {
        Box::pin(async move {
            let me = actor(inv)?;
            let thing = thing_arg(inv)?;
            let room = place(inv, &me).await?;
            let here = Holding::room(&room);
            let mine = Holding::carried(&me);
            let name = format!("urn:resmud:thing:id:{thing}");
            if !holds(inv, &here, &name).await? {
                return Err(Error::Conflict(if holds(inv, &mine, &name).await? {
                    format!("you already carry the {thing}")
                } else {
                    format!("the {thing} is not here: look to see what is in the {room}")
                }));
            }
            shift(
                inv,
                &thing,
                &here,
                &mine,
                "drop",
                format!("someone else took the {thing} first: look to see what is in the {room}"),
            )
            .await?;
            Ok(text(format!("You take the {thing}.")))
        })
    })
    .with_description(description(
        "resmud-take",
        "Pick up a thing that is in the room you are in.",
        "the key of the thing to take, such as lamp",
    ))
}

/// `urn:resmud:verb:name:drop`: put down a thing you carry (`content`: `lamp`).
pub fn drop() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("resmud-drop", |inv| {
        Box::pin(async move {
            let me = actor(inv)?;
            let thing = thing_arg(inv)?;
            let room = place(inv, &me).await?;
            let mine = Holding::carried(&me);
            if !holds(inv, &mine, &format!("urn:resmud:thing:id:{thing}")).await? {
                return Err(Error::Conflict(format!(
                    "you do not carry the {thing}: look to see what you carry"
                )));
            }
            shift(
                inv,
                &thing,
                &mine,
                &Holding::room(&room),
                "take",
                format!(
                    "the {thing} left your hands before you could drop it: look to see where it is"
                ),
            )
            .await?;
            Ok(text(format!("You drop the {thing}.")))
        })
    })
    .with_description(description(
        "resmud-drop",
        "Put down a thing you carry, in the room you are in.",
        "the key of the thing to drop, such as lamp",
    ))
}
