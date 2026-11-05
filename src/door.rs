//! The first door: the `ikigai` REPL, for one player.
//!
//! A [`Door`] is [`ikigai_engine::Engine`] (the REPL's grammar: `source`, `sink`, pipes) over
//! the realm's kernel, opened for one player: every request resolves under that player's
//! capability and in that player's personal corridor ([`crate::corridor`]), so
//! `source urn:resmud:look` means *here, for me* at the prompt exactly as it does to a host.
//! [`serve`] is the line loop around it, which `examples/door.rs` runs at a terminal.
//!
//! **The player's words.** The affordance list teaches a player to type `go east`, `take lamp`
//! and `polish lamp`, and the refusals say `look` and "ask what you can do here", so the door
//! accepts those words. Each maps, as text, to the REPL line that names an existing resource,
//! and the engine does the rest; the door adds no game logic:
//!
//! | a player types | the door issues |
//! |---|---|
//! | `look` | `source urn:resmud:look` |
//! | `help` or `what` | `source urn:resmud:affordance` (what you can do here) |
//! | `go east`, `take lamp`, `drop lamp` | `sink urn:resmud:verb:name:{go,take,drop} …` |
//! | `polish lamp` (any other first word) | `sink urn:resmud:verb:id:polish lamp`, a verb a player wrote |
//! | `source …`, `sink …`, `exists …` | the line itself, in the REPL's own grammar |
//! | `quit` | leaves |
//!
//! The engine's other commands (`cap`, `list`, `bind`, `trace`, Lisp) are not a player's:
//! their answers name resources by IRI. A first word that is not in the table is a verb a
//! player wrote, so `list` asks the realm for a verb called `list`, and the realm says it has
//! none.
//!
//! **What the door prints is plain text, always.** A refusal is [`crate::refusal`] of the
//! TYPED error the kernel raised, never the engine's message (which names resources by IRI
//! for a programmer): the door's resolver keeps the first error of each line for exactly this.
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

use crate::{corridor, is_plain, player, refusal, Realm};

/// The moves bound in code (`urn:resmud:verb:name:{move}`): the words `go`, `take` and `drop`.
pub const MOVES: [&str; 3] = ["go", "take", "drop"];

/// What the door says for an answer it will not print: one that is not plain text.
pub const WITHHELD: &str =
    "That answer is not plain text, so this door does not show it: ask what you can do here.";

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

/// What a line of the player's words is.
#[derive(Debug, PartialEq, Eq)]
enum Words {
    Nothing,
    Quit,
    /// A word that takes no more words, given some.
    Alone(String),
    /// A line in the REPL's grammar.
    Line(String),
}

/// Map a line of the player's words to the REPL line it means (see the module's table).
fn words(line: &str) -> Words {
    let line = line.trim();
    let (word, rest) = match line.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim()),
        None => (line, ""),
    };
    let alone = |line: &str| {
        if rest.is_empty() {
            Words::Line(line.to_string())
        } else {
            Words::Alone(word.to_string())
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
        "" => Words::Nothing,
        "quit" if rest.is_empty() => Words::Quit,
        "quit" => Words::Alone(word.to_string()),
        "source" | "sink" | "exists" => Words::Line(line.to_string()),
        "look" => alone("source urn:resmud:look"),
        "help" | "what" => alone("source urn:resmud:affordance"),
        _ if MOVES.contains(&word) => with(format!("urn:resmud:verb:name:{word}")),
        _ => with(format!("urn:resmud:verb:id:{word}")),
    }
}

/// The REPL line a line of the player's words issues, or `None` for one that issues nothing
/// (an empty line, `quit`, or a word such as `look` given more words).
///
/// ```
/// use resmud::door::request_line;
///
/// assert_eq!(request_line("look").as_deref(), Some("source urn:resmud:look"));
/// assert_eq!(request_line("go east").as_deref(), Some("sink urn:resmud:verb:name:go east"));
/// assert_eq!(request_line("polish lamp").as_deref(), Some("sink urn:resmud:verb:id:polish lamp"));
/// assert_eq!(request_line("help").as_deref(), Some("source urn:resmud:affordance"));
/// assert_eq!(request_line("source urn:resmud:look").as_deref(), Some("source urn:resmud:look"));
/// assert_eq!(request_line("look lamp"), None);
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
    refusal(&Error::InvalidArgument {
        name: "line".into(),
        detail: String::new(),
    })
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
                    Some(error) => Turn::refused(refusal(&error), Some(error)),
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
        assert_eq!(words("look lamp"), Words::Alone("look".into()));
        assert_eq!(words("help me"), Words::Alone("help".into()));
        assert_eq!(words("quit now"), Words::Alone("quit".into()));
    }

    #[test]
    fn printable_text_has_no_escape_and_no_iri() {
        assert!(printable("A warm kitchen.\nExits: east."));
        assert!(!printable("\u{1b}[1mA warm kitchen.\u{1b}[0m"));
        assert!(!printable("<urn:resmud:room:id:kitchen> a rms:Room ."));
        assert!(printable(WITHHELD));
        assert!(is_plain(&not_understood()));
    }
}
