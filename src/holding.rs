//! The atoms of where things are, by their public names: what a room contains
//! (`urn:resmud:room:id:{room}:content`, the file `content/{room}.ttl`) and what a character
//! carries (`urn:resmud:character:id:{character}:carries`, `carries/{character}.ttl`).
//!
//! **Why this is code.** Every other atom is a Source-only view onto its file. These two are
//! the same view, with one difference: a file that is not there reads as an EMPTY graph, so
//! a room the realm keeps no contents atom for holds nothing, and a character with no carries
//! atom carries nothing (resmud#57). The fallback is taken on the FILE's own NotFound and
//! nothing else, so the empty answer hangs from that file's thread, and the first write that
//! creates the file cuts it: whatever read the empty answer recomputes. Any other failure (a
//! denial, a file too large to read) still fails. No composition on ikigai-core 0.3 can say
//! that: `urn:iki:fn:compose`'s `||` falls back on ANY failure of its marker, a denial
//! included, so a caller who may not read the file would be told the room is empty.

use ikigai_core::{ArgSpec, AsyncFnEndpoint, Description, Error, ReprType, Representation, Verb};

use crate::{iri, is_key};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The atom of where things are for one kind of holder: `content` for a room, bound at
/// `urn:resmud:room:id:{room}:content`, or `carries` for a character, bound at
/// `urn:resmud:character:id:{character}:carries`. `key` is the template variable naming the
/// holder.
pub fn holding(kind: &'static str, key: &'static str) -> AsyncFnEndpoint {
    let id = format!("resmud-holding-{kind}");
    AsyncFnEndpoint::new(id.clone(), move |inv| {
        Box::pin(async move {
            let holder = inv.bindings.get(key).unwrap_or_default().to_string();
            if !is_key(&holder) {
                return Err(Error::InvalidArgument {
                    name: key.into(),
                    detail: format!("\"{holder}\" is not the name of a {key}"),
                });
            }
            match inv
                .source(&iri(&format!("urn:file:{kind}/{holder}.ttl")))
                .await
            {
                Ok(atom) => Ok(atom),
                // The file's own absence, and nothing else: it holds nothing yet.
                Err(Error::NotFound(_)) => Ok(Representation::new(
                    ReprType::new("text/turtle").with_param("charset", "utf-8"),
                    Vec::new(),
                )
                .cacheable()),
                Err(e) => Err(e),
            }
        })
    })
    .with_description(
        Description::new(id)
            .title("An atom of where things are")
            .summary(
                "The Turtle atom saying what a room contains or what a character carries, read \
                 from its file; a file that is not there reads as an empty graph.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new(key)
                    .summary("the key of the room or character that holds the things")
                    .class(XSD_STRING)
                    .binding(),
            )
            .output("text/turtle"),
    )
}
