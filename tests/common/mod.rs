//! The test harness: a realm copied into a scratch directory, a kernel hosting it, a player,
//! and a REPL-shaped way to issue requests, so a test reads like a session at the `ikigai`
//! REPL (`source urn:resmud:look`, `sink urn:resmud:verb:name:take lamp`).
#![allow(dead_code)] // each test binary uses part of the harness

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use futures::executor::block_on;
use resmud::ikigai_core::{
    ArgRef, Capability, EndpointSpace, Error, Exact, FnEndpoint, Iri, Kernel, ReprType,
    Representation, Request, Result, Scope, Space, Verb,
};
use resmud::Realm;

/// The seed of the test author's Ed25519 key. A TEST key, generated for this suite and
/// never used anywhere else.
pub const ALICE_SEED: [u8; 32] = [
    0x52, 0x65, 0x73, 0x4d, 0x55, 0x44, 0x20, 0x74, 0x65, 0x73, 0x74, 0x20, 0x6b, 0x65, 0x79, 0x2c,
    0x20, 0x6e, 0x65, 0x76, 0x65, 0x72, 0x20, 0x72, 0x65, 0x61, 0x6c, 0x2e, 0x20, 0x30, 0x30, 0x31,
];

/// Where the test author's private key resolves: the player's own, held by the test host,
/// never by the realm.
pub const ALICE_PRIVATE_KEY: &str = "urn:example:key:alice";

/// A PEM document of `label` around `der`.
fn pem(label: &str, der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

/// The PKCS8 private key for an Ed25519 seed (RFC 8410).
pub fn private_pem(seed: &[u8; 32]) -> String {
    let mut der = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    der.extend_from_slice(seed);
    pem("PRIVATE KEY", &der)
}

/// The SPKI public key for a raw Ed25519 public key (RFC 8410).
pub fn public_pem(raw: &[u8]) -> String {
    let mut der = vec![
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    der.extend_from_slice(raw);
    pem("PUBLIC KEY", &der)
}

/// The value of `sig:signer` in a signature graph: the raw public key, base64.
fn signer(graph: &str) -> Vec<u8> {
    let at = graph
        .find("sign#signer>")
        .or_else(|| graph.find("sig:signer"))
        .unwrap_or_else(|| panic!("no sig:signer in\n{graph}"));
    let rest = &graph[at..];
    let open = rest.find('"').unwrap() + 1;
    let close = open + rest[open..].find('"').unwrap();
    base64::engine::general_purpose::STANDARD
        .decode(&rest[open..close])
        .unwrap()
}

/// Copy the directory `from` into `to`, recursively.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A second TEST key, for a signer who is not alice.
pub const MALLORY_SEED: [u8; 32] = [7; 32];

/// Where the second test key resolves.
pub const MALLORY_PRIVATE_KEY: &str = "urn:example:key:mallory";

/// The host's own names: the players' private keys, which the realm never holds.
fn keys() -> Arc<dyn Space> {
    let key = |name: &'static str, seed: &[u8; 32]| {
        let private = private_pem(seed);
        FnEndpoint::new(name, move |_| {
            Ok(Representation::new(
                ReprType::new("application/x-pem-file"),
                private.clone().into_bytes(),
            ))
        })
    };
    Arc::new(
        EndpointSpace::new()
            .bind(
                Exact::new(ALICE_PRIVATE_KEY),
                key("test-key-alice", &ALICE_SEED),
            )
            .bind(
                Exact::new(MALLORY_PRIVATE_KEY),
                key("test-key-mallory", &MALLORY_SEED),
            ),
    )
}

/// A realm in a scratch directory, the kernel hosting it, and its one player.
pub struct World {
    pub dir: tempfile::TempDir,
    pub realm: Realm,
    pub kernel: Arc<Kernel>,
}

impl World {
    /// The `first` realm, copied from the repository's `realm/first` into a fresh scratch
    /// directory, with the test author's public key registered.
    pub fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        let root: PathBuf = dir.path().join("first");
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("realm/first"),
            &root,
        );
        let realm = Realm::new("first", &root).unwrap();
        let kernel = Arc::new(resmud::kernel(&realm, vec![keys()]).unwrap());
        let world = World { dir, realm, kernel };
        // Register the author's public key, derived from the private key by signing once.
        let graph = world.text(
            world.issue(
                Request::new(Verb::Source, iri("urn:sign:sign"))
                    .with_arg("in", ArgRef::Inline(b"register".to_vec()))
                    .with_arg("key", ArgRef::Inline(ALICE_PRIVATE_KEY.as_bytes().to_vec())),
                &Capability::root(),
                Scope::empty(),
            ),
        );
        std::fs::write(
            root.join("author/alice.pub.pem"),
            public_pem(&signer(&graph)),
        )
        .unwrap();
        world
    }

    /// The realm's directory.
    pub fn root(&self) -> &Path {
        self.realm.root()
    }

    /// Alice's capability: a player playing the character `alice`.
    pub fn alice(&self) -> Capability {
        resmud::player(&self.realm, "alice")
    }

    /// Issue `request` under `cap` in `scope`.
    pub fn issue(
        &self,
        request: Request,
        cap: &Capability,
        scope: Scope,
    ) -> Result<Representation> {
        block_on(self.kernel.issue_in(request, cap, scope))
    }

    /// The answer as text, or a panic naming the error.
    pub fn text(&self, answer: Result<Representation>) -> String {
        match answer {
            Ok(rep) => String::from_utf8(rep.bytes).unwrap(),
            Err(e) => panic!("refused: {e:?}"),
        }
    }

    /// One REPL line, as alice, in her personal corridor: `source <iri> [k=v …] [value]` or
    /// `sink <iri> [k=v …] [value]`. A trailing bare value is the `content` of a sink, as
    /// the REPL pipes it.
    pub fn play(&self, line: &str) -> Result<String> {
        self.play_as(line, &self.alice(), resmud::corridor("alice"))
    }

    /// One REPL line under `cap` in `scope`.
    pub fn play_as(&self, line: &str, cap: &Capability, scope: Scope) -> Result<String> {
        let request = repl(line);
        self.issue(request, cap, scope)
            .map(|rep| String::from_utf8(rep.bytes).unwrap())
    }
}

pub fn iri(s: &str) -> Iri {
    Iri::parse(s).unwrap()
}

/// A REPL line as a request: the verb, the target, `k=v` arguments, and a bare value as the
/// `content` the REPL routes a sink's trailing value to.
pub fn repl(line: &str) -> Request {
    let mut words = line.split_whitespace();
    let verb = match words.next() {
        Some("source") => Verb::Source,
        Some("sink") => Verb::Sink,
        Some("exists") => Verb::Exists,
        other => panic!("not a REPL verb: {other:?}"),
    };
    let mut request = Request::new(verb, iri(words.next().expect("a target")));
    for word in words {
        match word.split_once('=') {
            Some((k, v)) => request = request.with_arg(k, ArgRef::Inline(v.as_bytes().to_vec())),
            None => request = request.with_arg("content", ArgRef::Inline(word.as_bytes().to_vec())),
        }
    }
    request
}

/// Whether `answer` is the error variant `want` names.
pub fn is(answer: &Result<String>, want: fn(&Error) -> bool) -> bool {
    matches!(answer, Err(e) if want(e))
}

/// The `polish` verb a player writes: set the description of the thing it is run on.
pub const POLISH: &str = r#"; polish: make a thing shine.
(graph
  (prefix (rm "https://iriref.org/resmud/core#"))
  ((iri "urn:resmud:argument:name:target") rm:description "A brass lamp, polished until it shines."))
"#;

impl World {
    /// Alice's signature graph over `source`, made by `urn:sign:sign` with her own key.
    pub fn sign(&self, source: &str) -> String {
        self.sign_with(ALICE_PRIVATE_KEY, source)
    }

    /// A signature graph over `source` made with the private key at `key`.
    pub fn sign_with(&self, key: &str, source: &str) -> String {
        self.text(
            self.issue(
                Request::new(Verb::Source, iri("urn:sign:sign"))
                    .with_arg("in", ArgRef::Inline(source.as_bytes().to_vec()))
                    .with_arg("key", ArgRef::Inline(key.as_bytes().to_vec())),
                &self.alice(),
                resmud::corridor("alice"),
            ),
        )
    }

    /// Write one part (`source`, `signature`, `author`) of verb `name`, as alice.
    pub fn publish_part(&self, name: &str, part: &str, body: &str) {
        let write = Request::new(
            Verb::Sink,
            iri(&format!("urn:resmud:verb:id:{name}:{part}")),
        )
        .with_arg("content", ArgRef::Inline(body.as_bytes().to_vec()));
        self.text(self.issue(write, &self.alice(), resmud::corridor("alice")));
    }

    /// Publish verb `name` as alice: its source, the signature graph given, and her as its
    /// author, each written through its name.
    pub fn publish_with(&self, name: &str, source: &str, signature: &str) {
        for (part, body) in [
            ("source", source),
            ("signature", signature),
            ("author", "alice\n"),
        ] {
            self.publish_part(name, part, body);
        }
    }

    /// Publish verb `name` as alice, signed by her.
    pub fn publish(&self, name: &str, source: &str) {
        let signature = self.sign(source);
        self.publish_with(name, source, &signature);
    }

    /// A file of the realm, as text.
    pub fn file(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root().join(rel)).unwrap()
    }
}

impl World {
    /// The door for alice: the `ikigai` REPL over this world's kernel, in her corridor.
    pub fn door(&self) -> resmud::door::Door {
        resmud::door::Door::new(Arc::clone(&self.kernel), &self.realm, "alice")
    }

    /// A door that resolves under `cap` in `scope`.
    pub fn door_as(&self, cap: &Capability, scope: Scope) -> resmud::door::Door {
        resmud::door::Door::with(Arc::clone(&self.kernel), cap.clone(), scope)
    }
}
