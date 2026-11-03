//! `go`, `take` and `drop`: the three moves, each of which rewrites ONE atom or refuses.
//!
//! **Why this is code.** Each changes state and can refuse, and a refusal is part of its
//! contract: a generic composer cannot say *no*. Everything else each one needs is a name:
//! the actor's place (`urn:resmud:character:id:{c}:place`), where an exit leads and who holds
//! a thing (stored queries), and the rewritten atom (the stored `relocate` CONSTRUCT). The
//! only thing an edge does itself is decide, and write the answer to the atom's file.
//!
//! The location of a thing is in its OWN atom (`<room> rms:contains <lamp>` lives in
//! `thing/lamp.ttl`), so a move rewrites only the mover's atom, and the kernel's cut of that
//! one file is what makes every view that read it recompute.
//!
//! | case | error |
//! |---|---|
//! | a caller with no character (no principal) | `Denied` |
//! | `go` a bearing that is not a direction | `InvalidArgument` |
//! | `take`/`drop` a thing whose name is not a key | `InvalidArgument` |
//! | `go` where there is no exit that way | `Conflict` |
//! | `take` a thing that is not here, or does not exist | `Conflict` |
//! | `drop` a thing the actor does not carry | `Conflict` |

use ikigai_core::{
    ActionSpec, ArgRef, ArgSpec, AsyncFnEndpoint, Description, Error, FailureClass, Invocation,
    Request, Result, Verb,
};
use serde_json::json;

use crate::query::{construct, select, text, uri};
use crate::{character_iri, iri, is_key, CAP_PLAY, DIRECTIONS, SPACE_VOCABULARY, VOCABULARY};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The character the caller plays: its principal, which must be a character's name.
pub(crate) fn actor(inv: &Invocation<'_>) -> Result<String> {
    let principal = inv.capability.principal().ok_or_else(|| {
        Error::Denied("you are not playing a character: the capability names no principal".into())
    })?;
    principal
        .strip_prefix("urn:resmud:character:id:")
        .filter(|key| is_key(key))
        .map(str::to_string)
        .ok_or_else(|| {
            Error::Denied(format!(
                "`{principal}` is not a character, so it cannot act in the world"
            ))
        })
}

/// The key of the room `character` is in, read through its `:place` composition.
pub(crate) async fn place(inv: &Invocation<'_>, character: &str) -> Result<String> {
    let answer = inv
        .source(&iri(&format!("{}:place", character_iri(character))))
        .await?;
    let room = String::from_utf8_lossy(&answer.bytes).trim().to_string();
    if is_key(&room) {
        Ok(room)
    } else {
        Err(Error::Conflict(format!(
            "you are nowhere: no room contains {}",
            character_iri(character)
        )))
    }
}

/// The content argument as a key (a thing's name), or `InvalidArgument` saying why not.
fn thing_arg(inv: &Invocation<'_>) -> Result<String> {
    let raw = inv.inline_str("content")?.trim();
    if is_key(raw) {
        Ok(raw.to_string())
    } else {
        Err(Error::InvalidArgument {
            name: "content".into(),
            detail: format!(
                "`{raw}` is not the name of a thing: use its key, such as `lamp` \
                 (lowercase letters, digits and `-`)"
            ),
        })
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

/// The name of the room or character that holds `thing` now, if anything does; a thing
/// that does not exist is held by nothing.
async fn holder(inv: &Invocation<'_>, thing: &str) -> Result<Option<String>> {
    let atom = format!("urn:resmud:thing:id:{thing}");
    match select(
        inv,
        ArgRef::Reference(iri("urn:resmud:query:id:holder")),
        &atom,
        json!({ "subject": uri(&atom) }),
    )
    .await
    {
        Ok(lines) => Ok(lines.into_iter().next()),
        Err(Error::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
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
            let bearing = inv.inline_str("content")?.trim().to_string();
            if !DIRECTIONS.contains(&bearing.as_str()) {
                return Err(Error::InvalidArgument {
                    name: "content".into(),
                    detail: format!(
                        "`{bearing}` is not a direction: say north, south, east, west, up or down"
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
            let me_iri = character_iri(&me);
            match holder(inv, &thing).await? {
                Some(h) if h == format!("urn:resmud:room:id:{room}") => {}
                Some(h) if h == me_iri => {
                    return Err(Error::Conflict(format!("you already carry the {thing}")))
                }
                _ => {
                    return Err(Error::Conflict(format!(
                        "the {thing} is not here: look to see what is in the {room}"
                    )))
                }
            }
            relocate(
                inv,
                &format!("urn:resmud:thing:id:{thing}"),
                &me_iri,
                &format!("{VOCABULARY}carries"),
                &format!("urn:file:thing/{thing}.ttl"),
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
            if holder(inv, &thing).await? != Some(character_iri(&me)) {
                return Err(Error::Conflict(format!(
                    "you do not carry the {thing}: look to see what you carry"
                )));
            }
            relocate(
                inv,
                &format!("urn:resmud:thing:id:{thing}"),
                &format!("urn:resmud:room:id:{room}"),
                &format!("{SPACE_VOCABULARY}contains"),
                &format!("urn:file:thing/{thing}.ttl"),
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
