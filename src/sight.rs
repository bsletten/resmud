//! What a character sees, and what looking at one name of it says.
//!
//! What a character can see is one resource, `urn:resmud:character:id:{c}:sees`: a stored query
//! over the atoms of where that character and its room are. `look` and a name reads that
//! resource and nothing else about the world, so a corridor that rebinds or extends it for one
//! character (a magic eye, a camera) changes what that character can look at, and the realm's
//! rule stays as it is.
//!
//! **Why this is code.** Two things no composition can say:
//!
//! - [`seen`] says *no*. The answer for a name the looker does not see is a refusal, the same
//!   for a thing in another room as for a name that exists nowhere, so `look` never reveals
//!   that a thing exists elsewhere. `urn:iki:fn:conditional` could choose between two names, but
//!   it cannot raise a typed refusal naming the word the player typed.
//! - [`characters`] picks the characters out of `urn:resmud:world`, which is a list of names in
//!   a text file. A character's place is said by its own atom alone, so who is here is a question
//!   over every character's atom, and nothing in a named request filters the lines of a text.
//!   The query reads those atoms and no other, never the world.

use ikigai_core::{ArgSpec, AsyncFnEndpoint, Description, Error, Verb};

use crate::query::text;
use crate::{edge, iri, is_key};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The kinds of name `look` can be given, in the order a name shared by two is read: a thing
/// first, as `look` read it before it could name anything else.
pub(crate) const KINDS: [&str; 3] = ["thing", "character", "room"];

/// The refusal for a name the looker does not see, whether it exists elsewhere or nowhere.
pub(crate) fn not_seen(name: &str) -> Error {
    Error::NotFound(format!("you see no {name} here"))
}

/// The endpoint at `urn:resmud:seen`: the description of `name`, if `sees` (the list of names
/// a character sees) holds a thing, character or room of that name, or the same refusal for
/// every other name.
pub fn seen() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("resmud-seen", |inv| {
        Box::pin(async move {
            let name = inv.inline_str("name")?.trim().to_string();
            if !is_key(&name) {
                return Err(edge::not_a_thing(&name));
            }
            let sees = inv.inline_str("sees")?;
            let found = KINDS
                .iter()
                .map(|kind| format!("urn:resmud:{kind}:id:{name}"))
                .find(|candidate| sees.lines().any(|line| line.trim() == candidate));
            let Some(found) = found else {
                return Err(not_seen(&name));
            };
            // A contents atom can name a thing with no atom of its own; the room's text does
            // not show one, so it is not something to see either.
            match inv.source(&iri(&format!("{found}:description"))).await {
                Err(Error::NotFound(_)) => Err(not_seen(&name)),
                answer => answer,
            }
        })
    })
    .with_description(
        Description::new("resmud-seen")
            .title("The description of something a character sees")
            .summary(
                "Answers the description of the thing, character or room called `name` if it is \
                 in `sees`, and refuses any other name the same way, whether it exists elsewhere \
                 or nowhere.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("name")
                    .summary("the key of what to look at, such as lamp")
                    .class(XSD_STRING),
            )
            .input(
                ArgSpec::new("sees")
                    .summary("the names the looker sees, one per line")
                    .class(XSD_STRING),
            )
            .output("text/plain;charset=utf-8"),
    )
}

/// The endpoint at `urn:resmud:world:character`: the name of each character the realm's
/// world lists, one per line, in the world's order.
pub fn characters() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("resmud-world-character", |inv| {
        Box::pin(async move {
            let world = inv.source(&iri("urn:resmud:world")).await?;
            let world = String::from_utf8_lossy(&world.bytes);
            let names: Vec<&str> = world
                .lines()
                .map(str::trim)
                .filter(|line| {
                    line.strip_prefix("urn:resmud:character:id:")
                        .is_some_and(is_key)
                })
                .collect();
            Ok(text(names.join("\n")).cacheable())
        })
    })
    .with_description(
        Description::new("resmud-world-character")
            .title("The realm's characters")
            .summary("The name of each character the realm's world lists, one per line.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .output("text/plain;charset=utf-8"),
    )
}
