//! What a player reads when a request is refused: one sentence in plain words, never an IRI.
//!
//! **Why this is code.** ResMUD's own endpoints refuse in plain words, naming things by the
//! word a player sees (`the lamp`, `the kitchen`), but some refusals a player can reach are the
//! KERNEL's, raised before any ResMUD code runs: the capability floor
//! (``capability does not grant `urn:cap:resmud:play` …``), a name nothing answers, a verb a
//! name does not answer, an argument left out. Those name resources by IRI, in backticks and
//! angle brackets, because their reader is a programmer. A door shows a player
//! [`refusal`]`(&error)` instead of the error itself, so the mapping happens once, at the edge,
//! for every refusal from every source.
//!
//! The rule is mechanical: a message that is already plain ([`is_plain`]) is passed through as
//! a sentence (its first letter capitalized, a period at the end); anything else is replaced by
//! a plain sentence for its kind of refusal, which says what to do instead where it can. So a
//! refusal never shows a player an IRI, even one that slipped into a message by accident.

use ikigai_core::Error;

/// Whether `s` reads as plain words: it names no IRI (no `urn:` starting a word, no `://`)
/// and carries no markup a screen reader would read aloud as symbols (`<`, `>`, a backtick).
///
/// ```
/// assert!(resmud::is_plain("there is no exit north from the kitchen: look to see the exits"));
/// assert!(resmud::is_plain("you cannot return: the door is shut"));
/// assert!(!resmud::is_plain("urn:resmud:thing:id:pebble has no description"));
/// assert!(!resmud::is_plain("capability does not grant `urn:cap:resmud:play`"));
/// assert!(!resmud::is_plain("see https://example.org/"));
/// assert!(!resmud::is_plain("take <thing>"));
/// ```
pub fn is_plain(s: &str) -> bool {
    if s.contains("://") || s.contains(['<', '>', '`']) {
        return false;
    }
    let mut rest = s;
    while let Some(at) = rest.find("urn:") {
        let starts_a_word = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if starts_a_word {
            return false;
        }
        rest = &rest[at + 4..];
    }
    true
}

/// `s` as a sentence: its first letter capitalized, and a period at the end.
fn sentence(s: &str) -> String {
    let s = s.trim();
    let mut chars = s.chars();
    let mut out = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
        None => String::new(),
    };
    if !out.ends_with(['.', '?', '!']) {
        out.push('.');
    }
    out
}

/// What a player reads when `error` refuses their request: one sentence in plain words.
///
/// Where a sentence points the player at what they can do here, it says "ask what you can do
/// here", because this function does not know what the player's door calls that. A door that
/// has a word for it calls [`refusal_with_help`] instead.
///
/// ```
/// use resmud::ikigai_core::Error;
///
/// // ResMUD's own refusals are already plain, and pass through as sentences.
/// let no_exit = Error::Conflict("there is no exit north from the kitchen: look to see the exits".into());
/// assert_eq!(resmud::refusal(&no_exit), "There is no exit north from the kitchen: look to see the exits.");
///
/// // The kernel's capability floor names the capability by IRI; a player reads this instead.
/// let floor = Error::Denied(
///     "capability does not grant `urn:cap:resmud:play` (declared by `urn:resmud:verb:name:take`)".into(),
/// );
/// assert_eq!(
///     resmud::refusal(&floor),
///     "You may not do that here: ask what you can do here to see what you may."
/// );
/// ```
pub fn refusal(error: &Error) -> String {
    phrase(error, None)
}

/// [`refusal`], for a door whose word `help` answers what you can do here: where the sentence
/// points the player there, it names the word ("type help to see what you can do here"). A
/// word that is not one plain word (empty, with a space, or not [`is_plain`]) is not named, and
/// the sentence says "ask what you can do here" as [`refusal`] does.
///
/// ```
/// use resmud::ikigai_core::Error;
///
/// let floor = Error::Denied("capability does not grant `urn:cap:resmud:play`".into());
/// assert_eq!(
///     resmud::refusal_with_help(&floor, "help"),
///     "You may not do that here: type help to see what you may."
/// );
/// let unsupported = Error::Unsupported { verb: None, detail: String::new() };
/// assert_eq!(
///     resmud::refusal_with_help(&unsupported, "help"),
///     "That cannot be done to it: type help to see what you can do here."
/// );
/// // A refusal that is already plain is the endpoint's own words, unchanged.
/// let no_exit = Error::Conflict("there is no exit north from the kitchen: look to see the exits".into());
/// assert_eq!(resmud::refusal_with_help(&no_exit, "help"), resmud::refusal(&no_exit));
/// ```
pub fn refusal_with_help(error: &Error, help: &str) -> String {
    let word = Some(help).filter(|w| {
        !w.is_empty() && is_plain(w) && !w.contains(|c: char| c.is_whitespace() || c.is_control())
    });
    phrase(error, word)
}

/// What you can do here, as the generic sentences point to it.
const HERE: &str = "what you can do here";

/// The refusal for `error`, pointing at what the player can do here by `help`, the door's
/// word for it, when there is one.
fn phrase(error: &Error, help: Option<&str>) -> String {
    let point = |see: &str| match help {
        Some(word) => format!("type {word} to see {see}"),
        None if see == HERE => format!("ask {HERE}"),
        None => format!("ask {HERE} to see {see}"),
    };
    let own = match error {
        Error::Denied(m) | Error::Conflict(m) | Error::NotFound(m) => Some(m.as_str()),
        Error::InvalidArgument { detail, .. } => Some(detail.as_str()),
        _ => None,
    };
    if let Some(message) = own.filter(|m| is_plain(m) && !m.trim().is_empty()) {
        return sentence(message);
    }
    sentence(&match error {
        Error::Denied(_) => format!("you may not do that here: {}", point("what you may")),
        Error::Conflict(_) => {
            "that cannot be done now: look to see where you are and what is here".to_string()
        }
        Error::InvalidArgument { .. } => {
            format!(
                "that is not something this realm understands: {}",
                point(HERE)
            )
        }
        Error::MissingArgument(_) => "that needs one more word: a direction to go, or the thing \
                                      to take, drop or run a verb on, such as take lamp"
            .to_string(),
        Error::NotFound(_) | Error::Unresolved(_) => {
            "there is nothing by that name in this realm".to_string()
        }
        Error::Unsupported { .. } => format!("that cannot be done to it: {}", point(HERE)),
        _ => "something went wrong in the realm: try again, and if it keeps happening, tell the \
              realm's operator"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_that_merely_contains_urn_is_plain() {
        for s in [
            "you cannot return: the door is shut",
            "a nocturn: played softly",
        ] {
            assert!(is_plain(s), "{s}");
        }
        for s in [
            "urn:x",
            "see (urn:x)",
            "the urn: urn:resmud:room:id:kitchen",
        ] {
            assert!(!is_plain(s), "{s}");
        }
    }

    #[test]
    fn a_sentence_is_capitalized_and_ends_once() {
        assert_eq!(sentence("you take nothing"), "You take nothing.");
        assert_eq!(sentence("take what?"), "Take what?");
        assert_eq!(
            sentence("\"Lamp\" is not a thing"),
            "\"Lamp\" is not a thing."
        );
        assert_eq!(sentence("done."), "Done.");
    }

    #[test]
    fn every_kind_of_refusal_reads_as_a_plain_sentence() {
        use ikigai_core::Iri;
        let iri = Iri::parse("urn:resmud:thing:id:lamp").unwrap();
        for error in [
            Error::Denied("capability does not grant `urn:cap:resmud:play`".into()),
            Error::Conflict("urn:resmud:room:id:kitchen refuses".into()),
            Error::NotFound("`urn:file:room/attic.ttl` does not exist".into()),
            Error::InvalidArgument {
                name: "content".into(),
                detail: "urn:rdf:from-sexpr: unknown graph head `select`".into(),
            },
            Error::MissingArgument("content".into()),
            Error::Unresolved(iri.clone()),
            Error::Unsupported {
                verb: None,
                detail: "<urn:resmud:thing:id:lamp> is a named request for Source".into(),
            },
            Error::Endpoint("a SELECT answered something that is not JSON".into()),
            Error::DepthExceeded {
                depth: 65,
                target: iri,
            },
            Error::Denied(String::new()),
        ] {
            for said in [refusal(&error), refusal_with_help(&error, "help")] {
                assert!(is_plain(&said), "{error:?} -> {said}");
                assert!(said.ends_with('.'), "{said}");
                assert!(said.chars().next().unwrap().is_uppercase(), "{said}");
            }
        }
    }

    #[test]
    fn a_door_word_that_is_not_one_plain_word_is_not_named() {
        let denied = Error::Denied(String::new());
        for word in ["", "type help", "<help>", "`help`", "help\u{1b}[0m"] {
            assert_eq!(
                refusal_with_help(&denied, word),
                refusal(&denied),
                "{word:?}"
            );
        }
        assert_eq!(
            refusal_with_help(&denied, "what"),
            "You may not do that here: type what to see what you may."
        );
        assert_eq!(
            refusal(&denied),
            "You may not do that here: ask what you can do here to see what you may."
        );
    }
}
