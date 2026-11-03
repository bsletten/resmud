//! ResMUD: shared, programmable worlds on ikigai.
//!
//! The first playable build, as `docs/design/smallest-playable.md` designs it: one realm, the
//! text face, and one signed verb a player wrote. Almost all of it is NAMES: the atoms are Turtle
//! files under a realm directory, and every other resource is composed from them by a query, a
//! template, an alias or `urn:kernel:actions`. Code exists only where a composition cannot say
//! *no*:
//!
//! - [`edge`]: `go`, `take` and `drop`, each of which rewrites one atom or refuses;
//! - [`runner`]: the verb runner, which checks a verb's signature, clamps it to its author's
//!   grant, validates the patch against the realm's shapes and writes it;
//! - [`query`]: one adapter, `urn:resmud:query`, that binds a NAME into a stored SPARQL query
//!   and answers its `?line` column as plain text. It is code because a named request cannot
//!   carry a SPARQL binding (see its docs); the map as built says so;
//! - [`refusal`]: what a player reads when a request is refused, one plain sentence, mapped
//!   at the edge because some refusals are the kernel's own and name resources by IRI.
//!
//! Every ResMUD name lives under [`NAMESPACE`], and resolves through an [`ikigai_core`]
//! kernel built by [`kernel`].
#![forbid(unsafe_code)]

pub mod edge;
mod plain;
pub mod query;
pub mod runner;
mod text;

pub use plain::{is_plain, refusal};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ikigai_core::{
    Alias, AliasTable, ArgSpec, Capability, DeclarationError, EndpointSpace, Exact, Fallback,
    FixedArgument, Iri, Kernel, Level, Mount, Scope, Space, TopologyRule, UriTemplate, Verb,
};

pub use ikigai_core;

/// The URN prefix every ResMUD resource is named under, the type first: `urn:resmud:room:id:{room}`.
///
/// ```
/// assert!(resmud::NAMESPACE.ends_with(':'));
/// assert_eq!(format!("{}room:id:kitchen", resmud::NAMESPACE), "urn:resmud:room:id:kitchen");
/// ```
pub const NAMESPACE: &str = "urn:resmud:";

/// The core vocabulary's namespace (`rm:`), published by the `resmud-vocab` repository and
/// served at this IRI.
///
/// ```
/// assert_eq!(resmud::VOCABULARY, "https://iriref.org/resmud/core#");
/// ```
pub const VOCABULARY: &str = "https://iriref.org/resmud/core#";

/// The spatial vocabulary's namespace (`rms:`), from the same repository.
pub const SPACE_VOCABULARY: &str = "https://iriref.org/resmud/space#";

/// The namespace of the terms this build needs and `resmud-vocab` does not define yet
/// (`rml:`): `rml:grant`, `rml:key`, `rml:shapes`, `rml:allows` and `rml:pattern`. Clearly
/// local, under ResMUD's own URN, until a vocabulary change gives them a published home.
///
/// ```
/// assert_eq!(format!("{}grant", resmud::LOCAL_VOCABULARY), "urn:resmud:term:name:grant");
/// ```
pub const LOCAL_VOCABULARY: &str = "urn:resmud:term:name:";

/// The capability every player action (`go`, `take`, `drop`) requires.
pub const CAP_PLAY: &str = "urn:cap:resmud:play";

/// The capability running a verb a player wrote requires.
pub const CAP_VERB_RUN: &str = "urn:cap:resmud:verb:run";

/// The prefix of the grant tokens an author's verbs run under: `urn:cap:resmud:write:thing`
/// lets a verb rewrite things, and so on for `character` and `room`.
pub const CAP_WRITE_PREFIX: &str = "urn:cap:resmud:write:";

/// The placeholder a verb's plan names the thing it is run on by: the runner fills it with
/// `urn:resmud:thing:id:{content}`.
pub const TARGET: &str = "urn:resmud:argument:name:target";

/// The kinds of atom a move or a verb may rewrite. Everything under `urn:resmud:realm:` and
/// `urn:resmud:author:` is sealed: changing what may exist, or who may write, is legislation,
/// not play.
pub const WRITABLE_KINDS: [&str; 3] = ["thing", "character", "room"];

/// One hosted realm: its key (`first`), and the directory its atoms live in.
#[derive(Clone, Debug)]
pub struct Realm {
    id: String,
    root: PathBuf,
}

impl Realm {
    /// The realm `id`, whose atoms are the files under `root`.
    ///
    /// ```
    /// let realm = resmud::Realm::new("first", "/tmp/realm").unwrap();
    /// assert_eq!(realm.iri(), "urn:resmud:realm:id:first");
    /// assert!(resmud::Realm::new("First Realm", "/tmp/realm").is_err());
    /// ```
    pub fn new(id: &str, root: impl Into<PathBuf>) -> ikigai_core::Result<Realm> {
        if !is_key(id) {
            return Err(ikigai_core::Error::InvalidArgument {
                name: "realm".into(),
                detail: format!("`{id}` is not a realm key: use lowercase letters, digits and `-`"),
            });
        }
        Ok(Realm {
            id: id.to_string(),
            root: root.into(),
        })
    }

    /// The realm's key.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The directory holding the realm's atoms.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The realm atom's name, `urn:resmud:realm:id:{realm}`.
    pub fn iri(&self) -> String {
        format!("urn:resmud:realm:id:{}", self.id)
    }

    /// The realm's shapes, `urn:resmud:realm:id:{realm}:shape`.
    pub fn shape_iri(&self) -> String {
        format!("urn:resmud:realm:id:{}:shape", self.id)
    }

    /// A file-system capability scope for `action` (`read`, `write`) on `rel` under the
    /// realm's directory (the whole directory when `rel` is empty).
    pub fn fs_scope(&self, action: &str, rel: &str) -> String {
        let root = self.root.to_string_lossy();
        let root = root.trim_end_matches('/');
        if rel.is_empty() {
            format!("urn:cap:fs:{action}:{root}")
        } else {
            format!("urn:cap:fs:{action}:{root}/{rel}")
        }
    }
}

/// Whether `s` is a key: the last, named segment of a ResMUD name (`kitchen`, `lamp`,
/// `polish`). Lowercase ASCII letters, digits and `-`, starting with a letter or digit, at
/// most 64 characters.
///
/// ```
/// assert!(resmud::is_key("lamp"));
/// assert!(resmud::is_key("brass-lamp-2"));
/// assert!(!resmud::is_key("Lamp"));
/// assert!(!resmud::is_key("lamp!"));
/// assert!(!resmud::is_key(""));
/// assert!(!resmud::is_key("-lamp"));
/// ```
pub fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
}

/// The six bearings of `rms:Direction`, in the order the vocabulary lists them.
pub const DIRECTIONS: [&str; 6] = ["north", "south", "east", "west", "up", "down"];

/// The name of the level the realm's internals run in.
fn level_iri(realm: &Realm) -> Iri {
    iri(&format!("urn:resmud:level:realm:id:{}", realm.id))
}

pub(crate) fn iri(s: &str) -> Iri {
    Iri::parse(s).unwrap_or_else(|e| panic!("`{s}` is a valid IRI by construction: {e}"))
}

fn template(s: &str) -> UriTemplate {
    UriTemplate::parse(s).unwrap_or_else(|e| panic!("`{s}` is a valid template: {e:?}"))
}

/// The realm's resource map: every name the design lists, as an alias table.
///
/// - **Atoms** are files, each reached by a Source-only view, so no caller can write an atom
///   by its public name: `urn:resmud:room:id:{room}` reads `room/{room}.ttl`.
/// - **Verb sources** (`:source`, `:signature`, `:author`) forward every verb, so a player
///   publishes a verb by writing them; nothing else about a verb is writable.
/// - **Compositions** are named requests onto `urn:resmud:query` (a stored query with one name
///   bound), `urn:iki:fn:compose` (a template), `urn:rdf:from-sexpr` (the plan) and
///   `urn:kernel:actions` (the manifold).
pub fn resource_map(realm: &Realm) -> Result<AliasTable, DeclarationError> {
    let view = |from: &str, to: &str| TopologyRule::named_template(from, to).verb(Verb::Source);
    let file = |from: &str, to: &str| TopologyRule::named_request(from, to).verb(Verb::Source);
    // A question about the whole world reads every atom the world lists; a question about
    // one atom reads only that atom, so it hangs from that atom's thread alone.
    let ask = |from: &str, name: &str, subject: &str| {
        TopologyRule::named_template(from, "urn:resmud:query")
            .verb(Verb::Source)
            .fixing(FixedArgument::reference(
                "query",
                iri(&format!("urn:resmud:query:id:{name}")),
            ))
            .fixing(FixedArgument::reference_template(
                "subject",
                template(subject),
            ))
    };
    let query = |from: &str, name: &str, subject: &str| {
        ask(from, name, subject).fixing(FixedArgument::reference("graph", iri("urn:resmud:world")))
    };
    let query_atom = |from: &str, name: &str, subject: &str| {
        ask(from, name, subject)
            .fixing(FixedArgument::reference_template("atom", template(subject)))
    };
    let rules = [
        // The atoms.
        file(&realm.iri(), "urn:file:realm.ttl"),
        file(&realm.shape_iri(), "urn:file:shape.ttl"),
        file("urn:resmud:world", "urn:file:world.txt"),
        view("urn:resmud:room:id:{room}", "urn:file:room/{room}.ttl"),
        view("urn:resmud:thing:id:{thing}", "urn:file:thing/{thing}.ttl"),
        view(
            "urn:resmud:character:id:{character}",
            "urn:file:character/{character}.ttl",
        ),
        view(
            "urn:resmud:author:id:{author}",
            "urn:file:author/{author}.ttl",
        ),
        view(
            "urn:resmud:author:id:{author}:key",
            "urn:file:author/{author}.pub.pem",
        ),
        // A verb as published: its source, its signature graph and its author's key.
        TopologyRule::named_template(
            "urn:resmud:verb:id:{verb}:source",
            "urn:file:verb/{verb}.sexpr",
        ),
        TopologyRule::named_template(
            "urn:resmud:verb:id:{verb}:signature",
            "urn:file:verb/{verb}.sig.ttl",
        ),
        TopologyRule::named_template(
            "urn:resmud:verb:id:{verb}:author",
            "urn:file:verb/{verb}.author.txt",
        ),
        // The plan: the source, compiled to a graph by ikigai-sexpr's transreptor.
        TopologyRule::named_template("urn:resmud:verb:id:{verb}:plan", "urn:rdf:from-sexpr")
            .verb(Verb::Source)
            .fixing(FixedArgument::reference_template(
                "content",
                template("urn:resmud:verb:id:{verb}:source"),
            )),
        // Questions about the world, each a stored query with one name bound.
        query_atom(
            "urn:resmud:character:id:{character}:place",
            "place",
            "urn:resmud:character:id:{character}",
        ),
        query(
            "urn:resmud:character:id:{character}:inventory",
            "inventory",
            "urn:resmud:character:id:{character}",
        ),
        query_atom(
            "urn:resmud:room:id:{room}:description",
            "description",
            "urn:resmud:room:id:{room}",
        ),
        query(
            "urn:resmud:room:id:{room}:content",
            "content",
            "urn:resmud:room:id:{room}",
        ),
        query_atom(
            "urn:resmud:room:id:{room}:exit",
            "exit",
            "urn:resmud:room:id:{room}",
        ),
        query_atom(
            "urn:resmud:thing:id:{thing}:description",
            "description",
            "urn:resmud:thing:id:{thing}",
        ),
        // The room text: a template filled with the room's three sentences.
        TopologyRule::named_template("urn:resmud:view:room:id:{room}", "urn:iki:fn:compose")
            .verb(Verb::Source)
            .fixing(FixedArgument::value("src", "urn:resmud:template:id:room"))
            .fixing(FixedArgument::capture("room", "room")),
        // Look, for one character: the room text of its place, then what it carries. The
        // place is a REFERENCE, which the kernel settles to the place's answer (the room's
        // key), so the template names the room it is in without any code reading it.
        TopologyRule::named_template(
            "urn:resmud:character:id:{character}:look",
            "urn:iki:fn:compose",
        )
        .verb(Verb::Source)
        .fixing(FixedArgument::value("src", "urn:resmud:template:id:look"))
        .fixing(FixedArgument::capture("character", "character"))
        .fixing(FixedArgument::reference_template(
            "room",
            template("urn:resmud:character:id:{character}:place"),
        )),
        // The caller's manifold, as a graph: urn:kernel:actions under the caller's capability.
        TopologyRule::named_request("urn:resmud:manifold", "urn:kernel:actions")
            .verb(Verb::Source)
            .input(
                ArgSpec::new("as")
                    .summary("the manifold's face; Turtle, so a query can read it")
                    .one_of(["text/turtle", "text/plain"])
                    .default_value("text/turtle"),
            ),
        // What can I do here: the realm's allowed actions that the caller's manifold offers.
        TopologyRule::named_request("urn:resmud:affordance", "urn:resmud:query")
            .verb(Verb::Source)
            .fixing(FixedArgument::reference(
                "query",
                iri("urn:resmud:query:id:affordance"),
            ))
            .fixing(FixedArgument::value(
                "graph",
                format!("urn:resmud:manifold {}", realm.iri()),
            ))
            .fixing(FixedArgument::value("subject", realm.iri())),
    ];
    rules
        .into_iter()
        .try_fold(AliasTable::new(), |table, rule| table.with_rule(rule))
}

/// The realm as a space: the resource map over the realm's code and its atoms, inside a
/// [`Level`] under `urn:resmud:`, so the files (`urn:file:*`) are reachable only from the
/// realm's own endpoints and views, never by a caller.
pub fn realm_space(realm: &Realm) -> Result<Arc<dyn Space>, DeclarationError> {
    let code = EndpointSpace::new()
        .bind(template("urn:resmud:query:id:{query}"), text::queries())
        .bind(
            template("urn:resmud:template:id:{template}"),
            text::templates(),
        )
        .bind(Exact::new("urn:resmud:query"), query::binder())
        .bind(Exact::new("urn:resmud:verb:name:go"), edge::go())
        .bind(Exact::new("urn:resmud:verb:name:take"), edge::take())
        .bind(Exact::new("urn:resmud:verb:name:drop"), edge::drop())
        .bind(
            template("urn:resmud:verb:id:{verb}"),
            runner::runner(realm.clone()),
        );
    let atoms: Arc<dyn Space> = Arc::new(ikigai_fs::cacheable_space(realm.root()));
    let inner = Fallback::new(vec![
        Arc::new(Alias::new(Arc::new(resource_map(realm)?), Arc::new(code))),
        atoms,
    ]);
    Ok(Arc::new(Mount::new(
        NAMESPACE,
        Arc::new(Level::new(level_iri(realm), Arc::new(inner))),
    )))
}

/// The published modules the realm composes with, at their conventional names:
/// `urn:sparql:*`, `urn:rdf:*` (and `urn:rdf:from-sexpr`), `urn:sign:*` and `urn:iki:fn:*`.
pub fn modules() -> Vec<Arc<dyn Space>> {
    vec![
        Arc::new(ikigai_sparql::space()),
        Arc::new(ikigai_sexpr::space()),
        Arc::new(ikigai_rdf::space()),
        Arc::new(ikigai_sign::space()),
        Arc::new(ikigai_fn::space()),
    ]
}

/// A kernel hosting `realm`, with `extra` spaces after it (a host's own names, such as a
/// player's key), and the Meta renderer every ikigai host uses.
pub fn kernel(realm: &Realm, extra: Vec<Arc<dyn Space>>) -> Result<Kernel, DeclarationError> {
    let mut spaces = vec![realm_space(realm)?];
    spaces.extend(modules());
    spaces.extend(extra);
    Ok(Kernel::with_meta_renderer(
        Arc::new(Fallback::new(spaces)),
        Arc::new(ikigai_vocab::TurtleRenderer),
    ))
}

/// The name of character `key`, `urn:resmud:character:id:{key}`, which is also the
/// principal a player playing it acts as.
pub fn character_iri(key: &str) -> String {
    format!("urn:resmud:character:id:{key}")
}

/// The personal corridor of the player playing `character`: `urn:resmud:look` there means
/// *here, for me*, the look of that character. A door that authenticates a player injects
/// it on every request (`Kernel::issue_in`).
pub fn corridor(character: &str) -> Scope {
    let table = AliasTable::new().exact_at_root(
        "urn:resmud:look",
        format!("{}:look", character_iri(character)),
    );
    Scope::empty().with_named(
        iri(&format!("urn:resmud:corridor:character:id:{character}")),
        Arc::new(Alias::new(Arc::new(table), Arc::new(EndpointSpace::new()))),
    )
}

/// The capability a player playing `character` in `realm` holds: the principal (the
/// character's own name), play and verb running, signing (to publish a verb), reading the
/// realm, and writing the atoms a move or a verb may rewrite. Writing the realm's shapes, its
/// realm atom and its authors is not in it: no player holds that.
pub fn player(realm: &Realm, character: &str) -> Capability {
    let mut scopes = vec![
        CAP_PLAY.to_string(),
        CAP_VERB_RUN.to_string(),
        ikigai_sign::CAP_SIGN.to_string(),
        realm.fs_scope("read", ""),
        realm.fs_scope("write", "verb"),
        realm.fs_scope("write", "world.txt"),
    ];
    for kind in WRITABLE_KINDS {
        scopes.push(format!("{CAP_WRITE_PREFIX}{kind}"));
        scopes.push(realm.fs_scope("write", kind));
    }
    Capability::scoped(scopes)
        .with_principal(&character_iri(character))
        .expect("a character's name is a well-formed principal")
}
