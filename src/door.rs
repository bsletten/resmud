//! The first door: the `ikigai` REPL, for one player.
//!
//! A [`Door`] is [`ikigai_engine::Engine`] (the REPL's grammar: `source`, `sink`, pipes) over
//! the realm's kernel, opened for one player: every request resolves under that player's
//! capability and in that player's personal corridor ([`crate::corridor`]), so
//! `source urn:resmud:look` means *here, for me* at the prompt exactly as it does to a host.
//! [`serve`] is the line loop around it, which `examples/door.rs` runs at a terminal.
//!
//! **The player's words.** The affordance list teaches a player to type `go east`, `take lamp`
//! and `polish lamp`, and the refusals say `look` and `help` ([`HELP`]), so the door accepts
//! those words. Each maps, as text, to the REPL line that names an existing resource,
//! and the engine does the rest; the door adds no game logic:
//!
//! | a player types | the door issues |
//! |---|---|
//! | `look` | `source urn:resmud:look` |
//! | `look lamp` | `source urn:resmud:look:name:lamp` (described only if the player sees it) |
//! | `help` or `what` | `source urn:resmud:affordance` (what you can do here) |
//! | `go east`, `take lamp`, `drop lamp` | `sink urn:resmud:verb:name:{go,take,drop} …` |
//! | `polish lamp` (any other first word) | `sink urn:resmud:verb:id:polish lamp`, a verb a player wrote |
//! | `source …`, `sink …`, `exists …` | the line itself, in the REPL's own grammar |
//! | `quit` | leaves |
//!
//! The door's own words are read in any case (`Look`, `GO east`); a verb a player wrote, a thing
//! and the REPL's grammar keep theirs, because the realm's names are lowercase (`Polish lamp` is
//! refused, as `take Lamp` is).
//!
//! The engine's other commands (`cap`, `list`, `bind`, `trace`, Lisp) are not a player's:
//! their answers name resources by IRI. A first word that is not in the table is a verb a
//! player wrote, so `list` asks the realm for a verb called `list`, and the realm says it has
//! none.
//!
//! **What the door prints is plain text, always.** A refusal is [`crate::refusal_with_help`] of
//! the TYPED error the kernel raised, never the engine's message (which names resources by IRI
//! for a programmer): the door's resolver keeps the first error of each line for exactly this.
//! Where a refusal points at what the player can do here, it names this door's word for that
//! ("type help to see what you can do here"); the word is the door's, so it is the door that
//! passes it, and a host with a door of its own passes its own word, or calls
//! [`crate::refusal`] and says "ask what you can do here".
//! An answer that is not plain ([`crate::is_plain`], and no control characters, so no color)
//! is withheld with a sentence saying so: `source urn:resmud:manifold` answers Turtle, which a
//! player does not read.
//!
//! **Why this is code.** A door is a host's face, and the engine is a library a host drives:
//! nothing in the kernel can read a terminal. Every answer is still a name's.

use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use futures::executor::block_on;
use ikigai_core::{
    Body, Capability, Error, Kernel, Provenance, Representation, Request, Scope, SpaceEntry, Verb,
};
use ikigai_engine::{Action, Engine};
use ikigai_resolve::{CacheStatus, Resolver};

use crate::{corridor, edge, is_key, is_plain, player, refusal_with_help, Realm};

/// The moves bound in code (`urn:resmud:verb:name:{move}`): the words `go`, `take` and `drop`.
pub const MOVES: [&str; 3] = ["go", "take", "drop"];

/// The door's word for what you can do here, which its refusals name. `what` means the same.
pub const HELP: &str = "help";

/// What the door says for an answer it will not print: one that is not plain text.
pub const WITHHELD: &str =
    "That answer is not plain text, so this door does not show it: type help to see what you can \
     do here.";

/// One turn at the door: what the player reads, if anything.
#[derive(Debug)]
pub enum Turn {
    /// The answer, in plain text.
    Said(String),
    /// A refusal: the sentence a player reads, and the typed error behind it when the kernel
    /// raised one (`None` for a line the engine could not read, or an answer withheld).
    Refused {
        /// One plain sentence.
        said: String,
        /// The kernel's error, for a host or a test; never shown to a player.
        error: Option<Error>,
    },
    /// The player left.
    Quit,
    /// An empty line: nothing to say.
    Nothing,
}

impl Turn {
    /// What the player reads: the answer or the refusal's sentence.
    pub fn text(&self) -> Option<&str> {
        match self {
            Turn::Said(text) | Turn::Refused { said: text, .. } => Some(text),
            Turn::Quit | Turn::Nothing => None,
        }
    }

    fn refused(said: impl Into<String>, error: Option<Error>) -> Turn {
        Turn::Refused {
            said: said.into(),
            error,
        }
    }
}

/// What `look` says given more than one word.
pub const LOOK_AT_ONE: &str = "Say look on its own, or look and one thing, such as look lamp.";

/// What a line of the player's words is.
#[derive(Debug, PartialEq, Eq)]
enum Words {
    Nothing,
    Quit,
    /// A word that takes no more words, given some.
    Alone(String),
    /// `look` and a word that is not the name of a thing: refused as `take` refuses it.
    NotAThing(String),
    /// A door word given words it cannot take: the sentence that says what it takes.
    Usage(&'static str),
    /// A line in the REPL's grammar.
    Line(String),
}

/// Map a line of the player's words to the REPL line it means (see the module's table).
///
/// The door's own words (`look`, `help`, `what`, `quit` and the [`MOVES`]) are read in any
/// case, so `Look` and `LOOK` are `look`. Everything else keeps its case: a verb a player wrote
/// and a thing are NAMES, which the realm spells in lowercase (`crate::is_key`), and the REPL's
/// `source`, `sink` and `exists` are its own grammar.
fn words(line: &str) -> Words {
    let line = line.trim();
    let (word, rest) = match line.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim()),
        None => (line, ""),
    };
    let said = word.to_lowercase();
    let alone = |line: &str| {
        if rest.is_empty() {
            Words::Line(line.to_string())
        } else {
            Words::Alone(said.clone())
        }
    };
    let with = |target: String| {
        if rest.is_empty() {
            Words::Line(format!("sink {target}"))
        } else {
            Words::Line(format!("sink {target} {rest}"))
        }
    };
    match word {
        "" => return Words::Nothing,
        "source" | "sink" | "exists" => return Words::Line(line.to_string()),
        _ => {}
    }
    match said.as_str() {
        "quit" if rest.is_empty() => Words::Quit,
        "quit" => Words::Alone(said),
        "look" if rest.is_empty() => Words::Line("source urn:resmud:look".into()),
        // `look lamp`: what the player sees by that name, resolved through the looker in the
        // player's corridor (`crate::sight`), never straight to the thing. Its name is checked
        // as the realm checks a thing's name, so `look Lamp` is refused as `take Lamp` is.
        "look" if rest.contains(char::is_whitespace) => Words::Usage(LOOK_AT_ONE),
        "look" if is_key(rest) => Words::Line(format!("source urn:resmud:look:name:{rest}")),
        "look" => Words::NotAThing(rest.to_string()),
        w if w == HELP || w == "what" => alone("source urn:resmud:affordance"),
        w if MOVES.contains(&w) => with(format!("urn:resmud:verb:name:{w}")),
        _ => with(format!("urn:resmud:verb:id:{word}")),
    }
}

/// The REPL line a line of the player's words issues, or `None` for one that issues nothing
/// (an empty line, `quit`, a word such as `help` given more words, or `look` given a word that
/// is not the name of a thing).
///
/// ```
/// use resmud::door::request_line;
///
/// assert_eq!(request_line("look").as_deref(), Some("source urn:resmud:look"));
/// assert_eq!(request_line("go east").as_deref(), Some("sink urn:resmud:verb:name:go east"));
/// assert_eq!(request_line("polish lamp").as_deref(), Some("sink urn:resmud:verb:id:polish lamp"));
/// assert_eq!(request_line("help").as_deref(), Some("source urn:resmud:affordance"));
/// assert_eq!(request_line("source urn:resmud:look").as_deref(), Some("source urn:resmud:look"));
/// assert_eq!(
///     request_line("look lamp").as_deref(),
///     Some("source urn:resmud:look:name:lamp")
/// );
/// assert_eq!(request_line("LOOK").as_deref(), Some("source urn:resmud:look"));
/// assert_eq!(request_line("look Lamp"), None);
/// assert_eq!(request_line("quit"), None);
/// ```
pub fn request_line(line: &str) -> Option<String> {
    match words(line) {
        Words::Line(line) => Some(line),
        _ => None,
    }
}

/// Whether the door may print `s`: plain words ([`is_plain`]) and no control character but
/// a line break or a tab, so no terminal escape and no color.
fn printable(s: &str) -> bool {
    is_plain(s) && s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t')
}

/// The refusal for a line the engine could not read at all.
fn not_understood() -> String {
    refusal_with_help(
        &Error::InvalidArgument {
            name: "line".into(),
            detail: String::new(),
        },
        HELP,
    )
}

/// The realm's kernel as the engine's resolver, for one player: every request resolves in the
/// player's corridor, and the first error of each line is kept for the door to phrase.
struct InCorridor {
    kernel: Arc<Kernel>,
    corridor: Scope,
    first_error: Arc<Mutex<Option<Error>>>,
}

impl InCorridor {
    /// The chain a request resolves in: the player's corridor. The engine asks for another
    /// chain only for `as-of=`, which this door does not offer (it gives the engine no time
    /// doors, so the engine refuses `as-of=` itself), so any other chain is refused here too.
    fn chain(&self, scope: Scope) -> Result<Scope, Error> {
        if scope.is_empty() {
            Ok(self.corridor.clone())
        } else {
            Err(Error::Unsupported {
                verb: None,
                detail: "a player's door resolves in the player's corridor only".into(),
            })
        }
    }

    /// Keep `answer`'s error if it is the line's first. A contract read (Meta) is the engine
    /// reading how to route arguments, not the player's request, so its failure is not kept.
    fn note<T>(&self, verb: Verb, answer: Result<T, Error>) -> Result<T, Error> {
        if let Err(error) = &answer {
            if verb != Verb::Meta {
                let mut first = self
                    .first_error
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if first.is_none() {
                    *first = Some(error.clone());
                }
            }
        }
        answer
    }
}

#[async_trait]
impl Resolver for InCorridor {
    /// Root's resolution: never, at a player's door.
    fn issue(&self, request: Request) -> Result<(Representation, CacheStatus), Error> {
        let verb = request.verb;
        self.note(
            verb,
            Err(Error::Denied(
                "a player's door resolves under the player's capability, never root".into(),
            )),
        )
    }

    fn issue_as(
        &self,
        request: Request,
        capability: &Capability,
    ) -> Result<(Representation, CacheStatus), Error> {
        block_on(self.issue_as_async_in(request, capability, None, Scope::empty()))
    }

    async fn issue_as_async(
        &self,
        request: Request,
        capability: &Capability,
    ) -> Result<(Representation, CacheStatus), Error> {
        self.issue_as_async_in(request, capability, None, Scope::empty())
            .await
    }

    async fn issue_as_async_with_incoming(
        &self,
        request: Request,
        capability: &Capability,
        incoming: Provenance,
    ) -> Result<(Representation, CacheStatus), Error> {
        self.issue_as_async_in(request, capability, Some(incoming), Scope::empty())
            .await
    }

    async fn issue_as_async_in(
        &self,
        request: Request,
        capability: &Capability,
        incoming: Option<Provenance>,
        scope: Scope,
    ) -> Result<(Representation, CacheStatus), Error> {
        let verb = request.verb;
        let answer = match self.chain(scope) {
            Ok(scope) => {
                <Kernel as Resolver>::issue_as_async_in(
                    &self.kernel,
                    request,
                    capability,
                    incoming,
                    scope,
                )
                .await
            }
            Err(e) => Err(e),
        };
        self.note(verb, answer)
    }

    async fn issue_attached_in(
        &self,
        request: Request,
        capability: &Capability,
        incoming: Option<Provenance>,
        scope: Scope,
        attach: Option<(String, Body)>,
    ) -> Result<(Representation, CacheStatus), Error> {
        let verb = request.verb;
        let answer = match self.chain(scope) {
            Ok(scope) => {
                <Kernel as Resolver>::issue_attached_in(
                    &self.kernel,
                    request,
                    capability,
                    incoming,
                    scope,
                    attach,
                )
                .await
            }
            Err(e) => Err(e),
        };
        self.note(verb, answer)
    }

    async fn issue_body_attached_in(
        &self,
        request: Request,
        capability: &Capability,
        incoming: Option<Provenance>,
        scope: Scope,
        attach: Option<(String, Body)>,
    ) -> Result<(Representation<Body>, CacheStatus), Error> {
        let verb = request.verb;
        let answer = match self.chain(scope) {
            Ok(scope) => {
                <Kernel as Resolver>::issue_body_attached_in(
                    &self.kernel,
                    request,
                    capability,
                    incoming,
                    scope,
                    attach,
                )
                .await
            }
            Err(e) => Err(e),
        };
        self.note(verb, answer)
    }

    fn is_cached(&self, request: &Request, capability: &Capability) -> bool {
        self.kernel
            .is_cached_in(request, capability, &self.corridor)
    }

    fn is_cached_in(&self, request: &Request, capability: &Capability, scope: &Scope) -> bool {
        scope.is_empty() && self.is_cached(request, capability)
    }

    /// The door does not list the kernel's bindings: a player's list is the affordance list.
    fn entries(&self) -> Option<Vec<SpaceEntry>> {
        None
    }

    fn transport(&self) -> String {
        "a player's door · in-process".to_string()
    }
}

/// The REPL, opened for one player of a realm.
pub struct Door {
    engine: Engine,
    first_error: Arc<Mutex<Option<Error>>>,
}

impl Door {
    /// The door for the player playing `character` in `realm`, over `kernel` (built by
    /// [`crate::kernel`] for the same realm): [`crate::player`]'s capability, in the
    /// character's corridor.
    pub fn new(kernel: Arc<Kernel>, realm: &Realm, character: &str) -> Door {
        Door::with(kernel, player(realm, character), corridor(character))
    }

    /// A door that resolves under `capability` in `corridor`. The session is SEALED at that
    /// capability: no REPL command at the door can widen it.
    pub fn with(kernel: Arc<Kernel>, capability: Capability, corridor: Scope) -> Door {
        let first_error = Arc::new(Mutex::new(None));
        let engine = Engine::with_identity(
            InCorridor {
                kernel,
                corridor,
                first_error: Arc::clone(&first_error),
            },
            capability,
        );
        engine.seal();
        Door {
            engine,
            first_error,
        }
    }

    /// One line a player typed, and what the door says back.
    pub fn eval(&self, line: &str) -> Turn {
        let line = match words(line) {
            Words::Nothing => return Turn::Nothing,
            Words::Quit => return Turn::Quit,
            Words::Alone(word) => return Turn::refused(format!("Say {word} on its own."), None),
            Words::Usage(said) => return Turn::refused(said, None),
            Words::NotAThing(raw) => {
                let error = edge::not_a_thing(&raw);
                return Turn::refused(refusal_with_help(&error, HELP), Some(error));
            }
            Words::Line(line) => line,
        };
        *self
            .first_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        let action = self.engine.eval(&line);
        let first_error = self
            .first_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        match action {
            Action::Output(entry) => match entry.result {
                Ok(text) if printable(&text) => Turn::Said(text),
                Ok(_) => Turn::refused(WITHHELD, None),
                Err(_) => match first_error {
                    Some(error) => Turn::refused(refusal_with_help(&error, HELP), Some(error)),
                    None => Turn::refused(not_understood(), None),
                },
            },
            Action::Quit => Turn::Quit,
            _ => Turn::Nothing,
        }
    }
}

/// The prompt [`serve`] shows before each line.
pub const PROMPT: &str = "> ";

/// Serve `door` as a line loop: show [`PROMPT`], read a line from `input`, write what the door
/// says (one or more lines of plain text) to `output`, until `quit` or the end of the input.
pub fn serve(door: &Door, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut lines = input.lines();
    loop {
        output.write_all(PROMPT.as_bytes())?;
        output.flush()?;
        let Some(line) = lines.next().transpose()? else {
            output.write_all(b"\n")?;
            return output.flush();
        };
        let turn = door.eval(&line);
        if let Some(text) = turn.text() {
            output.write_all(text.as_bytes())?;
            output.write_all(b"\n")?;
        }
        if matches!(turn, Turn::Quit) {
            return output.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_players_words_map_to_repl_lines() {
        for (said, line) in [
            ("look", "source urn:resmud:look"),
            ("  look  ", "source urn:resmud:look"),
            ("help", "source urn:resmud:affordance"),
            ("what", "source urn:resmud:affordance"),
            ("go east", "sink urn:resmud:verb:name:go east"),
            ("go", "sink urn:resmud:verb:name:go"),
            ("take lamp", "sink urn:resmud:verb:name:take lamp"),
            ("drop  lamp", "sink urn:resmud:verb:name:drop lamp"),
            ("polish lamp", "sink urn:resmud:verb:id:polish lamp"),
            ("conjure", "sink urn:resmud:verb:id:conjure"),
            ("list", "sink urn:resmud:verb:id:list"),
            (
                "sink urn:resmud:verb:name:take lamp",
                "sink urn:resmud:verb:name:take lamp",
            ),
            ("exists urn:resmud:look", "exists urn:resmud:look"),
        ] {
            assert_eq!(words(said), Words::Line(line.into()), "{said}");
        }
        assert_eq!(words(""), Words::Nothing);
        assert_eq!(words("   "), Words::Nothing);
        assert_eq!(words("quit"), Words::Quit);
        assert_eq!(words("help me"), Words::Alone("help".into()));
        assert_eq!(words("quit now"), Words::Alone("quit".into()));
    }

    #[test]
    fn look_and_a_name_resolves_through_the_looker() {
        assert_eq!(
            words("look lamp"),
            Words::Line("source urn:resmud:look:name:lamp".into())
        );
        assert_eq!(
            words("Look  brass-lamp-2"),
            Words::Line("source urn:resmud:look:name:brass-lamp-2".into())
        );
        assert_eq!(words("look Lamp"), Words::NotAThing("Lamp".into()));
        assert_eq!(words("look lamp!"), Words::NotAThing("lamp!".into()));
        assert_eq!(words("look at lamp"), Words::Usage(LOOK_AT_ONE));
    }

    #[test]
    fn the_doors_own_words_are_read_in_any_case() {
        for (said, line) in [
            ("Look", "source urn:resmud:look"),
            ("LOOK", "source urn:resmud:look"),
            ("lOoK", "source urn:resmud:look"),
            ("Help", "source urn:resmud:affordance"),
            ("WHAT", "source urn:resmud:affordance"),
            ("Go east", "sink urn:resmud:verb:name:go east"),
            ("TAKE lamp", "sink urn:resmud:verb:name:take lamp"),
            ("Drop lamp", "sink urn:resmud:verb:name:drop lamp"),
            // A NAME keeps its case: the realm decides what it means.
            ("go East", "sink urn:resmud:verb:name:go East"),
            ("Polish lamp", "sink urn:resmud:verb:id:Polish lamp"),
            // The REPL's grammar is its own, and exact.
            (
                "Source urn:resmud:look",
                "sink urn:resmud:verb:id:Source urn:resmud:look",
            ),
        ] {
            assert_eq!(words(said), Words::Line(line.into()), "{said}");
        }
        assert_eq!(words("QUIT"), Words::Quit);
        assert_eq!(words("Quit now"), Words::Alone("quit".into()));
        assert_eq!(words("HELP me"), Words::Alone("help".into()));
    }

    #[test]
    fn printable_text_has_no_escape_and_no_iri() {
        assert!(printable("A warm kitchen.\nExits: east."));
        assert!(!printable("\u{1b}[1mA warm kitchen.\u{1b}[0m"));
        assert!(!printable("<urn:resmud:room:id:kitchen> a rms:Room ."));
        assert!(printable(WITHHELD));
        assert!(is_plain(&not_understood()));
    }

    #[test]
    fn the_word_the_refusals_name_is_a_word_the_door_takes() {
        assert_eq!(
            words(HELP),
            Words::Line("source urn:resmud:affordance".into())
        );
        for said in [WITHHELD.to_string(), not_understood()] {
            assert!(said.contains(&format!("type {HELP} to see")), "{said}");
        }
    }
}
