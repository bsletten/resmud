//! `urn:resmud:verb:id:{verb}`: run a verb a player wrote.
//!
//! **Why this is code.** It checks a signature, intersects two capabilities and writes a
//! patch, and every step can refuse. Each step exists already, and the runner only joins
//! them, in this order, refusing before anything is written:
//!
//! 0. **The verb**: its source (`…:source`) must exist. A verb nobody published is
//!    `NotFound` ("there is no verb called dance"), before its author or its thing is read.
//! 1. **The signature** (`urn:sign:verify`): the verb's source (`…:source`) against its
//!    signature graph (`…:signature`) and the key of the author it names (`…:author`), the
//!    one the realm registered (`urn:resmud:author:id:{author}:key`). Anything but `valid` is
//!    `Denied`.
//! 2. **The clamp**: the caller's capability attenuated to the author's grant
//!    (`urn:resmud:author:id:{author}`, `rml:grant`): the kernel's meet, so the verb runs
//!    under no more than BOTH hold. A verb that writes a kind of atom the clamp does not cover
//!    is `Denied`, and the file write runs under the clamp too, so the file module's own
//!    path-ACL refuses it a second time.
//! 3. **The plan** (`…:plan`): the source compiled to a graph by ikigai-sexpr's transreptor,
//!    golden-threaded from the source. Its subjects name the atoms it rewrites; one under
//!    `urn:resmud:realm:` or `urn:resmud:author:` is SEALED, and the whole verb is `Denied`.
//! 4. **The shapes**: each rewritten atom (the stored `patch` CONSTRUCT) is checked against
//!    the realm's shapes in a corridor holding only the candidate and the shapes (the stored
//!    `validate` query). A violation is `InvalidArgument`, naming the thing and the rule.
//! 5. **The write**: every atom, under the clamp. A new atom also joins the world's list. A
//!    room or a character the verb creates gets its atom of where things are too (the room's
//!    contents, what the character carries), written after it, so the clamp must cover both.
//!
//! Every refusal names things by the word a player sees (the verb's name, its author's, `the
//! lamp`), never by IRI. A signature's own verdict (which hash, which key) is not shown to a
//! player: it says only that the verb is not what its author signed.

use std::sync::Arc;

use ikigai_core::{
    ActionSpec, ArgRef, ArgSpec, AsyncFnEndpoint, Description, EndpointSpace, Error, Exact,
    FailureClass, Fallback, FnEndpoint, Invocation, ReprType, Representation, Request, Result,
    Verb,
};
use serde_json::json;

use crate::query::{construct, select, text, uri};
use crate::{
    iri, is_key, Realm, CAP_VERB_RUN, CAP_WRITE_PREFIX, SPACE_VOCABULARY, TARGET, VOCABULARY,
    WRITABLE_KINDS,
};

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// One atom a plan rewrites: its kind (`thing`, or `content` for a room's contents), its key
/// (`lamp`, `kitchen`) and its name.
struct Atom {
    kind: String,
    key: String,
    name: String,
}

impl Atom {
    fn file(&self) -> String {
        format!("urn:file:{}/{}.ttl", self.kind, self.key)
    }

    /// For an atom of where things are, the holder and how it holds them (`rms:contains`
    /// for a room's contents, `rm:carries` for what a character carries).
    fn holding(&self) -> Option<(String, String)> {
        match self.kind.as_str() {
            "content" => Some((
                format!("urn:resmud:room:id:{}", self.key),
                format!("{SPACE_VOCABULARY}contains"),
            )),
            "carries" => Some((
                crate::character_iri(&self.key),
                format!("{VOCABULARY}carries"),
            )),
            _ => None,
        }
    }

    /// For a room or a character, the atom of where things are that goes with it: the
    /// room's contents, or what the character carries. A verb that creates one creates the
    /// other, so the realm never has to read a missing one as empty.
    fn holding_atom(&self) -> Option<Atom> {
        let (kind, name) = match self.kind.as_str() {
            "room" => ("content", format!("{}:content", self.name)),
            "character" => ("carries", format!("{}:carries", self.name)),
            _ => return None,
        };
        Some(Atom {
            kind: kind.to_string(),
            key: self.key.clone(),
            name,
        })
    }

    /// What the refusal calls it: the key, or where the thing goes.
    fn said(&self) -> String {
        match self.kind.as_str() {
            "content" => format!("{}'s contents", self.key),
            "carries" => format!("things {} carries", self.key),
            _ => self.key.clone(),
        }
    }
}

/// The atom `subject` names, or the refusal for verb `verb` writing it: `Denied` for a
/// sealed name, `InvalidArgument` for a name that is not an atom of the realm.
fn atom_of(verb: &str, subject: &str) -> Result<Atom> {
    let sealed = if subject.starts_with("urn:resmud:author:") {
        Some("who the realm's authors are")
    } else if subject.starts_with("urn:resmud:realm:") && subject.contains(":shape") {
        Some("the realm's rules")
    } else if subject.starts_with("urn:resmud:realm:") {
        Some("the realm itself")
    } else {
        None
    };
    if let Some(what) = sealed {
        return Err(Error::Denied(format!(
            "the verb {verb} would change {what}, and no verb may change the realm, its rules \
             or its authors: that is legislation, not play"
        )));
    }
    // Where things are has atoms of its own: `…:room:id:kitchen:content` is the file
    // `content/kitchen.ttl`, and `…:character:id:alice:carries` is `carries/alice.ttl`.
    let parsed = subject
        .strip_prefix("urn:resmud:")
        .and_then(|rest| rest.split_once(":id:"))
        .map(|(kind, key)| match (kind, key.split_once(':')) {
            ("room", Some((room, "content"))) => ("content", room),
            ("character", Some((character, "carries"))) => ("carries", character),
            _ => (kind, key),
        })
        .filter(|(kind, key)| WRITABLE_KINDS.contains(kind) && is_key(key));
    match parsed {
        Some((kind, key)) => Ok(Atom {
            kind: kind.to_string(),
            key: key.to_string(),
            name: subject.to_string(),
        }),
        None if subject == TARGET => Err(Error::MissingArgument("content".into())),
        None => Err(Error::InvalidArgument {
            name: "plan".into(),
            detail: format!(
                "the verb {verb} writes something that is not a thing, character or room of this \
                 realm, so it does not run"
            ),
        }),
    }
}

/// A kind of atom as a refusal says it: "a thing", "a room's contents".
fn kind_said(kind: &str) -> &'static str {
    match kind {
        "thing" => "a thing",
        "character" => "a character",
        "room" => "a room",
        "content" => "what a room contains",
        "carries" => "what a character carries",
        _ => "that",
    }
}

/// The scopes the author's grant tokens license: each token itself, and the file-system
/// write it stands for under the realm's directory. A new atom joins the world's list, so a
/// grant to write any kind also covers that list.
fn licensed(realm: &Realm, grant: &[String]) -> Vec<String> {
    let mut scopes = Vec::new();
    for token in grant {
        if let Some(kind) = token.strip_prefix(CAP_WRITE_PREFIX) {
            if WRITABLE_KINDS.contains(&kind) {
                scopes.push(token.clone());
                scopes.push(realm.fs_scope("write", kind));
            }
        }
    }
    if !scopes.is_empty() {
        scopes.push(realm.fs_scope("write", "world.txt"));
    }
    scopes
}

/// A verb's own text atom, read as trimmed text.
async fn read_text(inv: &Invocation<'_>, name: &str) -> Result<String> {
    let answer = inv.source(&iri(name)).await?;
    Ok(String::from_utf8_lossy(&answer.bytes).trim().to_string())
}

/// Whether the realm keeps a file for `atom` yet. The file, not the atom's public name: a
/// room's contents read as empty when their file is not there, but they do not exist yet.
async fn exists(inv: &Invocation<'_>, atom: &Atom) -> Result<bool> {
    inv.issue(Request::new(Verb::Exists, iri(&atom.file())))
        .await
        .map(|answer| answer.bytes == b"true")
        .or_else(|e| match e {
            Error::NotFound(_) => Ok(false),
            e => Err(e),
        })
}

/// Step 0: whether verb `verb` was published, which is whether its source exists. A verb
/// nobody published is `NotFound`, in words.
async fn published(inv: &Invocation<'_>, verb: &str) -> Result<()> {
    let source = iri(&format!("urn:resmud:verb:id:{verb}:source"));
    let exists = inv
        .issue(Request::new(Verb::Exists, source))
        .await
        .map(|answer| answer.bytes == b"true")
        .or_else(|e| match e {
            Error::NotFound(_) | Error::Unresolved(_) => Ok(false),
            e => Err(e),
        })?;
    if exists {
        Ok(())
    } else {
        Err(Error::NotFound(format!("there is no verb called {verb}")))
    }
}

/// Step 1: verify the verb's signature against its author's registered key, and answer the
/// author's key.
async fn verify(inv: &Invocation<'_>, verb: &str) -> Result<String> {
    let base = format!("urn:resmud:verb:id:{verb}");
    let author = match read_text(inv, &format!("{base}:author")).await {
        Ok(author) if is_key(&author) => author,
        Ok(author) => {
            return Err(Error::Denied(format!(
                "the verb {verb} names \"{author}\" as its author, and no author has that name, \
                 so its signature cannot be checked"
            )))
        }
        Err(Error::NotFound(_)) => {
            return Err(Error::Denied(format!(
                "the verb {verb} names no author, so its signature cannot be checked: its author \
                 can publish it again with their name"
            )))
        }
        Err(e) => return Err(e),
    };
    let request = Request::new(Verb::Source, iri("urn:sign:verify"))
        .with_arg("in", ArgRef::Reference(iri(&format!("{base}:source"))))
        .with_arg("sig", ArgRef::Reference(iri(&format!("{base}:signature"))))
        .with_arg(
            "key",
            ArgRef::Inline(format!("urn:resmud:author:id:{author}:key").into_bytes()),
        );
    let verdict = match inv.issue(request).await {
        Ok(answer) => String::from_utf8_lossy(&answer.bytes).trim().to_string(),
        Err(Error::NotFound(_) | Error::Unresolved(_)) => {
            return Err(Error::Denied(format!(
                "the verb {verb} names {author} as its author, and this realm has no author \
                 called {author}, so its signature cannot be checked"
            )))
        }
        Err(_) => {
            return Err(Error::Denied(format!(
                "the verb {verb}'s signature cannot be checked against {author}'s key, so it \
                 does not run"
            )))
        }
    };
    if verdict == "valid" || verdict.starts_with("valid:") {
        Ok(author)
    } else {
        Err(Error::Denied(format!(
            "the verb {verb} is not what {author} signed, so it does not run: it was changed \
             after it was signed, or someone else signed it"
        )))
    }
}

/// Step 4: the realm's shapes against one candidate atom, in a corridor that holds only the
/// candidate, the shapes and the query module. Each violation is one line.
async fn violations(
    inv: &Invocation<'_>,
    shapes: Vec<u8>,
    candidate: String,
) -> Result<Vec<String>> {
    let turtle = || ReprType::new("text/turtle").with_param("charset", "utf-8");
    let data = candidate.into_bytes();
    let corridor_id = Representation::new(turtle(), data.clone()).content_id();
    let corridor = EndpointSpace::new()
        // Uncacheable on purpose: the candidate is this call's, so nothing read from it may
        // be kept for the next one.
        .bind(
            Exact::new("urn:resmud:candidate:data"),
            FnEndpoint::new("resmud-candidate", move |_| {
                Ok(Representation::new(turtle(), data.clone()))
            }),
        )
        .bind(
            Exact::new("urn:resmud:candidate:shape"),
            FnEndpoint::new("resmud-candidate-shape", move |_| {
                Ok(Representation::new(turtle(), shapes.clone()))
            }),
        );
    let query = read_text(inv, "urn:resmud:query:id:validate").await?;
    let confined = inv.confine(
        iri(&format!("urn:resmud:corridor:candidate:{corridor_id}")),
        Arc::new(Fallback::new(vec![
            Arc::new(corridor),
            Arc::new(ikigai_sparql::space()),
        ])),
    );
    select(
        &confined,
        ArgRef::Inline(query.into_bytes()),
        "urn:resmud:candidate:data urn:resmud:candidate:shape",
        json!({}),
    )
    .await
}

/// The runner, bound at `urn:resmud:verb:id:{verb}` in `realm`.
pub fn runner(realm: Realm) -> AsyncFnEndpoint {
    let realm = Arc::new(realm);
    AsyncFnEndpoint::new("resmud-verb", move |inv| {
        let realm = Arc::clone(&realm);
        Box::pin(async move {
            let verb = inv.bindings.get("verb").unwrap_or_default().to_string();
            if !is_key(&verb) {
                return Err(Error::InvalidArgument {
                    name: "verb".into(),
                    detail: format!(
                        "\"{verb}\" is not the name of a verb: say it the way its author \
                         published it, such as polish"
                    ),
                });
            }
            // 0. The verb: one nobody published is refused as such, before its author or the
            //    thing it would run on is read, so a typo is told there is no such verb.
            published(inv, &verb).await?;
            // No thing given, or a blank one (the REPL's `sink` with nothing after its
            // target sends empty content): the target stays unbound, and a plan that names
            // it is refused as missing a word.
            let target = match crate::edge::word(inv) {
                Ok(raw) if is_key(raw) => format!("urn:resmud:thing:id:{raw}"),
                Ok(raw) => return Err(crate::edge::not_a_thing(raw)),
                Err(Error::MissingArgument(_)) => TARGET.to_string(),
                Err(e) => return Err(e),
            };

            // 1. The signature.
            let author = verify(inv, &verb).await?;

            // 2. The clamp: the caller's capability, attenuated to the author's grant.
            let author_iri = format!("urn:resmud:author:id:{author}");
            let grant = select(
                inv,
                ArgRef::Reference(iri("urn:resmud:query:id:grant")),
                &author_iri,
                json!({ "subject": uri(&author_iri) }),
            )
            .await?;
            let scopes = licensed(&realm, &grant);
            let clamp = inv.capability.attenuate(scopes.iter().map(String::as_str));

            // 3. The plan, and the atoms it rewrites. Every refusal comes before any write.
            let plan = format!("urn:resmud:verb:id:{verb}:plan");
            // The plan is the source's transreption, so a source that is not a graph of
            // changes is refused there, in the transreptor's words; a player reads this.
            let subjects = select(
                inv,
                ArgRef::Reference(iri("urn:resmud:query:id:subject")),
                &plan,
                json!({ "plan": uri(&plan), "target": uri(&target) }),
            )
            .await
            .map_err(|e| match e {
                Error::InvalidArgument { .. } => Error::InvalidArgument {
                    name: "plan".into(),
                    detail: format!(
                        "the verb {verb} cannot run, because its source is not a graph of \
                         changes: its author can fix it and publish it again"
                    ),
                },
                e => e,
            })?;
            let mut atoms = subjects
                .iter()
                .map(|s| atom_of(&verb, s))
                .collect::<Result<Vec<_>>>()?;
            if atoms.is_empty() {
                return Err(Error::InvalidArgument {
                    name: "plan".into(),
                    detail: format!("the verb {verb} changes nothing"),
                });
            }
            // A room or a character the verb creates comes with its atom of where things are
            // (an empty one, unless the plan places something there), so it is a write like
            // any other: the clamp below must cover it too.
            let mut implied = Vec::new();
            for atom in &atoms {
                if let Some(holding) = atom.holding_atom() {
                    if !atoms.iter().any(|a| a.name == holding.name) && !exists(inv, atom).await? {
                        implied.push(holding);
                    }
                }
            }
            atoms.extend(implied);
            for atom in &atoms {
                let token = format!("{CAP_WRITE_PREFIX}{}", atom.kind);
                if !clamp.allows(&token) {
                    return Err(Error::Denied(format!(
                        "the verb {verb} writes the {}, and it runs under no more than both \
                         {author}'s grant and yours, which together do not cover writing {}",
                        atom.said(),
                        kind_said(&atom.kind)
                    )));
                }
            }

            // 4. The shapes, against every rewritten atom.
            let shapes = inv.source(&iri(&realm.shape_iri())).await?.bytes;
            let mut written = Vec::new();
            for atom in atoms {
                let exists = exists(inv, &atom).await?;
                let graph = if exists {
                    format!("{plan} {}", atom.name)
                } else {
                    plan.clone()
                };
                // An atom of where things are takes the plan's placements and loses nothing;
                // every other atom takes the plan's triples about its subject.
                let candidate = match atom.holding() {
                    Some((holder, how)) => {
                        construct(
                            inv,
                            "patch-holding",
                            &graph,
                            json!({
                                "atom": uri(&atom.name),
                                "holder": uri(&holder),
                                "how": uri(&how),
                                "plan": uri(&plan),
                                "target": uri(&target),
                            }),
                        )
                        .await?
                    }
                    None => {
                        construct(
                            inv,
                            "patch",
                            &graph,
                            json!({
                                "atom": uri(&atom.name),
                                "plan": uri(&plan),
                                "target": uri(&target),
                            }),
                        )
                        .await?
                    }
                };
                let refused = violations(inv, shapes.clone(), candidate.clone()).await?;
                if !refused.is_empty() {
                    return Err(Error::InvalidArgument {
                        name: "plan".into(),
                        detail: format!(
                            "the realm's rules refuse the verb {verb}: {}",
                            refused.join("; ")
                        ),
                    });
                }
                written.push((atom, candidate, exists));
            }

            // 5. The write, under the clamp. Where things are is written last, so a thing a
            //    verb creates and places exists before any room lists it: a failure between
            //    the writes leaves a thing nowhere, never a room listing a thing that is not.
            written.sort_by_key(|(atom, _, _)| atom.holding().is_some());
            let mut created = Vec::new();
            for (atom, candidate, exists) in &written {
                let write = Request::new(Verb::Sink, iri(&atom.file()))
                    .with_arg("content", ArgRef::Inline(candidate.clone().into_bytes()));
                inv.issue_attenuated(write, scopes.iter().map(String::as_str))
                    .await?;
                if !exists {
                    created.push(atom.name.clone());
                }
            }
            if !created.is_empty() {
                let mut world = read_text(inv, "urn:resmud:world").await?;
                for name in &created {
                    world.push('\n');
                    world.push_str(name);
                }
                world.push('\n');
                let write = Request::new(Verb::Sink, iri("urn:file:world.txt"))
                    .with_arg("content", ArgRef::Inline(world.into_bytes()));
                inv.issue_attenuated(write, scopes.iter().map(String::as_str))
                    .await?;
            }
            Ok(text(match target.strip_prefix("urn:resmud:thing:id:") {
                Some(thing) => format!("You {verb} the {thing}."),
                None => format!("You {verb}."),
            }))
        })
    })
    .with_description(
        Description::new("resmud-verb")
            .title("Run a verb a player wrote")
            .summary(
                "Runs a published verb: checks its signature against its author's key, clamps \
                 it to its author's grant, checks its patch against the realm's shapes, and \
                 writes it.",
            )
            .verb(Verb::Sink)
            .verb(Verb::Meta)
            .action(
                ActionSpec::new(Verb::Sink)
                    .summary("run a verb a player wrote")
                    .requires(CAP_VERB_RUN)
                    .input(
                        ArgSpec::new("verb")
                            .summary("the verb's key, such as polish")
                            .class(XSD_STRING)
                            .binding(),
                    )
                    .input(
                        ArgSpec::new("content")
                            .summary("the key of the thing to run the verb on, such as lamp")
                            .class(XSD_STRING)
                            .optional(),
                    )
                    .output("text/plain;charset=utf-8")
                    .fails(FailureClass::Denied)
                    .fails(FailureClass::Permanent),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_name_is_denied() {
        for sealed in [
            "urn:resmud:realm:id:first:shape",
            "urn:resmud:realm:id:first:shape:item",
            "urn:resmud:realm:id:first",
            "urn:resmud:author:id:alice",
        ] {
            assert!(
                matches!(atom_of("annex", sealed), Err(Error::Denied(_))),
                "{sealed}"
            );
        }
    }

    #[test]
    fn a_name_that_is_not_an_atom_is_an_invalid_argument() {
        for name in [
            "urn:resmud:verb:id:polish",
            "urn:resmud:thing:id:Lamp",
            "urn:example:lamp",
        ] {
            assert!(
                matches!(atom_of("polish", name), Err(Error::InvalidArgument { .. })),
                "{name}"
            );
        }
        assert!(matches!(
            atom_of("polish", TARGET),
            Err(Error::MissingArgument(_))
        ));
    }

    #[test]
    fn an_atom_maps_to_its_file() {
        let atom = atom_of("polish", "urn:resmud:thing:id:lamp").unwrap();
        assert_eq!(atom.file(), "urn:file:thing/lamp.ttl");
        assert!(atom.holding().is_none());
    }

    #[test]
    fn where_things_are_maps_to_its_own_atoms() {
        let content = atom_of("conjure", "urn:resmud:room:id:kitchen:content").unwrap();
        assert_eq!(content.kind, "content");
        assert_eq!(content.file(), "urn:file:content/kitchen.ttl");
        assert_eq!(
            content.holding(),
            Some((
                "urn:resmud:room:id:kitchen".to_string(),
                format!("{SPACE_VOCABULARY}contains")
            ))
        );
        let carries = atom_of("conjure", "urn:resmud:character:id:alice:carries").unwrap();
        assert_eq!(carries.file(), "urn:file:carries/alice.ttl");
        assert_eq!(
            carries.holding().map(|(holder, _)| holder),
            Some("urn:resmud:character:id:alice".to_string())
        );
        // Any other suffix is not an atom of the realm.
        for name in [
            "urn:resmud:room:id:kitchen:description",
            "urn:resmud:room:id:kitchen:carries",
            "urn:resmud:character:id:alice:content",
        ] {
            assert!(
                matches!(atom_of("conjure", name), Err(Error::InvalidArgument { .. })),
                "{name}"
            );
        }
    }

    #[test]
    fn a_grant_licenses_only_writable_kinds() {
        let realm = Realm::new("first", "/r").unwrap();
        let scopes = licensed(
            &realm,
            &[
                "urn:cap:resmud:write:thing".to_string(),
                "urn:cap:resmud:write:shape".to_string(),
                "urn:cap:fs:write:/".to_string(),
            ],
        );
        assert_eq!(
            scopes,
            [
                "urn:cap:resmud:write:thing",
                "urn:cap:fs:write:/r/thing",
                "urn:cap:fs:write:/r/world.txt",
            ]
        );
        assert!(licensed(&realm, &[]).is_empty());
    }
}
