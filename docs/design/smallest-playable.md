# The smallest playable thing

The first build of ResMUD: **one realm, the text face, and one signed verb a player wrote.** It was
chosen by build-readiness, not by excitement: every piece already exists in a published ikigai
crate, and the build is mostly naming. It proves four things and nothing else:

1. **The selection funnel is the parser.** What you can do here is `urn:kernel:actions` under your
   capability; there is no parser.
2. **The world is live.** A room's text is derived from its contents, so taking a thing changes
   what everyone sees, with no code that updates a description.
3. **A player can program the world.** A verb a player wrote, signed by its author, runs under no
   more authority than its author holds.
4. **The text face is complete.** Everything the game can say, it says in `text/plain`.

Out of scope for this build: instances, a second realm, travel, the LLM game master, the economy,
any graphics. Each is its own design thread, for later.

This document describes the build **as built**
([PR 2](https://github.com/bsletten/resmud/pull/2)). The first version of the design was wrong in
a few places, and building it showed where; [What the build changed](#what-the-build-changed) says
what changed and why.

## Resource map

Names are singular, the type first, and every partition is by a named key. The realm's
directory is `realm/first/`.

| set | name | kind | how |
|---|---|---|---|
| realm | `urn:resmud:realm:id:{realm}` | **atom** (`realm.ttl`) | the realm's label, the IRI of its shapes, and the actions its manifold allows (`rml:allows`, each with the door pattern it matches and the line the affordance list shows) |
| realm shapes | `urn:resmud:realm:id:{realm}:shape` | **atom** (`shape.ttl`) | SHACL: what may exist here. The first rule is the accessibility floor: every item, character and room has an `rm:description` |
| room | `urn:resmud:room:id:{room}` | **atom** (`room/{room}.ttl`) | `rms:Room`, its label, `rm:description`, its exits (`rms:exit` → `rms:bearing`, `rms:leadsTo`). It does **not** say what it contains |
| room contents | `urn:resmud:room:id:{room}:content` | **atom** (`content/{room}.ttl`) | what lies in the room: one `<room> rms:contains <thing>` triple per thing. Things only: a character's place stays in its own atom |
| thing | `urn:resmud:thing:id:{thing}` | **atom** (`thing/{thing}.ttl`) | `rm:Item`, its label, `rm:description`. It says **nothing** about where it is, so a move never rewrites it |
| character | `urn:resmud:character:id:{character}` | **atom** (`character/{character}.ttl`) | `rm:Character`, its label, `rm:description`, and where it is (`<room> rms:contains <character>`). It does **not** list what it carries |
| carried things | `urn:resmud:character:id:{character}:carries` | **atom** (`carries/{character}.ttl`) | what the character carries: one `<character> rm:carries <thing>` triple per thing |
| author | `urn:resmud:author:id:{author}`, `…:key` | **atom** (`author/{author}.ttl`, `author/{author}.pub.pem`) | the author's registered key (`rml:key`) and the grant their verbs run under (`rml:grant`, game tokens such as `urn:cap:resmud:write:thing`) |
| world | `urn:resmud:world` | **atom** (`world.txt`, a list of names) | the realm's atoms, the contents and carries atoms included. The union is `urn:sparql:*`'s own: each listed graph is loaded and the default graph is their union. No question the game asks reads it any more; a verb that creates an atom adds it here |
| place | `urn:resmud:character:id:{character}:place` | composition | the stored `place` query over the character's atom alone; answers the room's key |
| carried graphs | `urn:resmud:character:id:{character}:carries:graph` | composition | the stored `carries-graph` query over the carries atom alone: that atom's name and the name of each thing it lists, one per line |
| inventory | `urn:resmud:character:id:{character}:inventory` | composition | the stored `inventory` query over the carried graphs: the carries atom and the things in it, never the world |
| contents graphs | `urn:resmud:room:id:{room}:content:graph` | composition | the stored `content-graph` query over the contents atom alone: that atom's name and the name of each thing it lists |
| room parts | `urn:resmud:room:id:{room}:description`, `…:exit`, `…:content:line` | composition | stored queries: description and exits over the room's atom alone; the contents line over the contents graphs (the contents atom and the things in it, never the world) |
| thing description | `urn:resmud:thing:id:{thing}:description` | composition | the stored `description` query over the thing's atom alone |
| room text | `urn:resmud:view:room:id:{room}` | composition | `urn:iki:fn:compose` fills the `room` template with the three room parts; `text/plain` |
| look | `urn:resmud:look` | **alias rule** in the personal corridor | → `urn:resmud:character:id:{character}:look`, which fills the `look` template with the room text and the inventory. The room is a reference to the character's place, which the kernel settles to the place's answer, so no code reads it |
| manifold | `urn:resmud:manifold` | named request | `urn:kernel:actions` as Turtle, under the caller's capability |
| affordance | `urn:resmud:affordance` | composition | the stored `affordance` query over the manifold and the realm atom: the realm's allowed actions that the caller's own manifold offers, one line each |
| go | `urn:resmud:verb:name:go` | **Sink edge** (code) | decides, writes **one** atom (the character's) with the stored `relocate` CONSTRUCT, or refuses |
| take, drop | `urn:resmud:verb:name:{take\|drop}` | **Sink edge** (code) | each decides and writes **two** atoms, where the thing goes (the stored `put` CONSTRUCT) and then where it was (`remove`), each with `if-match`; see [The two-write move](#the-two-write-move) |
| verb source | `urn:resmud:verb:id:{verb}:source` | **atom** (`verb/{verb}.sexpr`) | the verb as an s-expression; writable, which is how a player publishes it |
| verb signature | `urn:resmud:verb:id:{verb}:signature` | **atom** (`verb/{verb}.sig.ttl`) | the author's `ikigai-sign` signature graph over the source |
| verb author | `urn:resmud:verb:id:{verb}:author` | **atom** (`verb/{verb}.author.txt`) | the key of the author who claims the verb |
| verb plan | `urn:resmud:verb:id:{verb}:plan` | **transreption** | the source compiled to a graph by `urn:rdf:from-sexpr` (`ikigai-sexpr`); golden-threaded from the source |
| verb | `urn:resmud:verb:id:{verb}` | **Sink edge** (code) | verify, clamp, check sealed names, check the shapes, write |
| query adapter | `urn:resmud:query` | code | a stored query with one name bound, answered as text lines (see below) |
| stored text | `urn:resmud:query:id:{query}`, `urn:resmud:template:id:{template}` | code serving data | the stored queries (`assets/query/*.rq`) and templates (`assets/template/*.txt`), each at a name |

The local terms (`rml:` = `urn:resmud:term:name:`) are `rml:grant`, `rml:key`, `rml:shapes`,
`rml:allows` and `rml:pattern`, kept under ResMUD's own URN until a `resmud-vocab` change gives
them a published home. The plan's placeholder `urn:resmud:argument:name:target` names the thing a
verb is run on.

Of the realm's 33 names, 26 are rules over other names and 7 are code. `urn:resmud:look` is not
counted, because it lives in the personal corridor.

### Spaces

- **Personal corridor.** `resmud::corridor(character)` is a named scope whose alias maps
  `urn:resmud:look` to that character's look, so `look` takes no argument: it means *here, for
  me*. A door injects it on every request (`Kernel::issue_in`). A player's principal is their
  character's own name, so the edges read the actor from the capability.
- **Realm corridor: not built as a corridor.** The one realm is the kernel's root space, mounted at
  `urn:resmud:`. A second realm would be a second scope over the same names.
- **Sealed names.** The verb runner refuses (Denied) any plan that writes a name under
  `urn:resmud:realm:` (the realm atom and its shapes) or `urn:resmud:author:`, before anything is
  written, and no player's capability holds a file write on `realm.ttl`, `shape.ttl` or `author/`.
  Changing what may exist, or who may write, is legislation, not play.
- **The realm's internals** run in a `Level`. Each atom's public name is a Source-only view, so a
  caller cannot write an atom by its name (`sink urn:resmud:thing:id:lamp` is Unsupported), and the
  files (`urn:file:*`) are unreachable from outside the realm (Unresolved).
- **Later:** the instance corridor (an ephemeral resolution of the realm under a party) and the
  as-of corridor (the realm before a contested change).

### Views

`text/plain` is the canonical face and the accessible one: every room answers in text, and the
manifold is always a list, so nothing is ever a hidden clickable. A Turtle face (`as=text/turtle`)
for agents is not built yet.

**The first door is the `ikigai` REPL** (`resmud::door`, ledger resmud#34): `ikigai-engine`
0.3.0's `Engine` over the realm's kernel, opened for one player under their capability, sealed
there, and with every request in their personal corridor. `cargo run --example door` plays the
sample realm at a prompt. The door takes the words the affordance list and the refusals teach,
and maps each, as text, to the REPL line that names an existing resource:

| a player types | the door issues |
|---|---|
| `look` | `source urn:resmud:look` |
| `look lamp` | `source urn:resmud:thing:id:lamp:description` |
| `help` or `what` | `source urn:resmud:affordance` |
| `go east`, `take lamp`, `drop lamp` | `sink urn:resmud:verb:name:{go,take,drop} …` |
| `polish lamp` (any other first word) | `sink urn:resmud:verb:id:polish lamp` |
| `source …`, `sink …`, `exists …` | the line itself |
| `quit` | leaves |

The door's own words are read in any case (`Look`, `GO east`). A verb a player wrote, a thing and
the REPL's grammar keep their case, because the realm's names are lowercase keys: `look Lamp` is
refused as `take Lamp` is, by the door itself, so a volume that folds case cannot answer it
differently from one that does not.

It prints plain text only: a refusal is `resmud::refusal_with_help` of the kernel's typed error
with the door's word `help` (the door's resolver keeps each line's first error, because the engine
hands a frontend only its own message, which names resources by IRI), and an answer that is not
plain, such as the Turtle of `urn:resmud:manifold`, is withheld with a sentence saying so. The
refusals name `help` because the word is the door's: `resmud::refusal` alone says "ask what you can
do here", so a host with its own door is never handed a word it lacks. `tests/door.rs` replays the six steps
of [Done means](#done-means) through the door with the same expected text; `tests/play.rs` still
plays them against the kernel.

### Why each piece of code is code

- **go, take, drop** (`src/edge.rs`): each changes state and can refuse, and a refusal is part of
  its contract. A generic composer cannot say *no*. Everything else each one needs is a name: the
  actor's place, where an exit leads and whether an atom holds a thing (stored queries), and each
  rewritten atom (the stored `relocate`, `put` and `remove` CONSTRUCTs). The edge decides, orders
  the writes, and writes the answers to the files: one for `go`, two for `take` and `drop`.
- **The verb runner** (`src/runner.rs`): it checks a signature, intersects two capabilities and
  writes a patch, and every step can refuse before anything is written. Each step exists already
  (`urn:sign:verify`, the kernel's attenuation, a Sink); the runner joins them. One piece of it is
  a mapping: an author's grant is written in game tokens (`urn:cap:resmud:write:thing`), because a
  realm file cannot name host paths, so the runner maps each token to the file-system scope it
  stands for under the realm's directory, then attenuates the caller's capability to that set. The
  verb runs under no more than both hold, and the file write runs under the clamp, so `ikigai-fs`'s
  path ACL refuses a second time.
- **The query adapter** (`urn:resmud:query`, `src/query.rs`): every question the game asks (where am
  I, what is here, where the east exit goes) is a stored query with one name in it, bound at a name
  like `urn:resmud:room:id:{room}:exit`. A named request can fix an argument as a value, a
  reference, a reference template, or the whole text a variable captured. But `urn:sparql:select`
  takes a query's variables only as a JSON object (`bindings=`), and nothing in between builds that
  object. Splicing the name into the query text would be the injection `bindings=` exists to
  prevent. So the adapter takes `subject` as a name (`xsd:anyURI`), binds it as `?subject`, and
  answers the `?line` column as plain text, one value per line. It answers text itself because
  there is no `text/plain` SPARQL results format, and `ikigai-fn`'s `each` HTML-escapes every value
  it splices, which would put `&#39;` into a text face. It reads nothing itself, so it is exactly as
  cacheable as what it read. A named request that could bind a capture as a SPARQL term, plus a
  plain-text results face, would retire it.
- **The endpoints serving stored queries and templates** (`src/text.rs`): they serve `assets/` at
  names, so a composition can name a query or a template and a golden thread can hang from it. They
  are data, not logic. They are code only because a file that ships inside the crate has no other
  way to become a name. They are cacheable for good.
- **The stored-query shapes check** (`assets/query/validate.rq`, run by the runner): the realm's
  shapes are enforced by a stored SPARQL query, not a SHACL engine, because `ikigai-shacl` is not on
  ikigai-core 0.3 yet. The query implements only the SHACL Core fragment the shapes use
  (`sh:targetClass`, a predicate `sh:path`, `sh:minCount`, `sh:message`), and says so; a shape that
  uses anything else is not checked. The runner runs it over each candidate atom in a confined
  corridor that holds only the candidate and the shapes, so the check can read nothing else. When
  `ikigai-shacl` moves to core 0.3, the runner calls it instead and the query goes.

- **The door** (`src/door.rs`): a host's face, which no name can be, because nothing in the
  kernel reads a terminal. It is the engine plus two pieces. A word table maps a player's words
  to REPL lines, as text, and adds no game logic. A resolver puts every request in the player's
  corridor and keeps each line's first typed error, because the engine hands a frontend its own
  message, never the error.

Everything else is a name over a file, a query, a template or an alias.

## What the build changed

Building it showed the first design was wrong, or silent, in six places:

- **Where things are has atoms of its own** (since resmud#52; the first build put a thing's
  location in the thing's own atom). The design said both that each move writes one atom and that
  a room elsewhere stays cached when a thing moves. Both cannot hold, and the first build chose the
  one write: every room's contents then depended on every thing's atom, so taking the lamp made
  every room recompute. As built now, `<kitchen> rms:contains <lamp>` lives in the kitchen's
  contents atom (`content/kitchen.ttl`) and `<alice> rm:carries <lamp>` in alice's carries atom
  (`carries/alice.ttl`), so `take` and `drop` write those two atoms and `go` writes only the
  character's. A take recomputes only what read one of the two: the kitchen's contents line and
  room text, and alice's inventory. A room elsewhere, its text included, and the lamp's own
  description stay cached. A test pins exactly this split (`tests/play.rs`,
  `a_take_recomputes_only_what_read_the_two_atoms_it_wrote`). See
  [Decided: option 1](#decided-option-1-a-contents-atom-per-room).
- **A character's place stays in its own atom.** Characters are not listed in a room's contents
  atom, so `go` stays one write and a character's place reads one atom. The cost is that "who is
  here" is not a question over one atom; nothing asks it yet.
- **A verb can place a thing it creates.** The runner names the atom a plan triple writes by its
  subject, except a triple that places a thing: `<room> rms:contains <thing>` writes the room's
  contents atom and `<character> rm:carries <thing>` the character's carries atom. So a grant can
  cover where things are (`urn:cap:resmud:write:content`) without covering the rooms themselves,
  and the sample author's grant does. A plan only ADDS to a contents or carries atom; taking a
  thing out of a room stays the moves' job. The runner writes a created thing before the atom that
  places it, so a failure between them leaves a thing nowhere, never a room listing a thing that
  does not exist (`tests/play.rs`, `a_verb_can_create_a_thing_and_place_it_here`).
- **Two rooms, not one,** because an exit has to lead somewhere: a kitchen, and a garden east of
  it.
- **The author has an atom.** The design did not say where a verb's author is recorded. As built, a
  verb has a third atom (`…:author`) naming its author's key, and the realm registers each author
  (`urn:resmud:author:id:{author}`, with `…:key`). The runner checks the signature against the
  registered key of the author the verb names, so a false claim fails as Denied. The author's atom
  also holds the grant the author's verbs run under.
- **Files, without a watcher.** The atoms are files on `ikigai-fs`'s cacheable mount, so every
  write through the kernel cuts exactly that file's thread. But `ikigai-fs` 0.4.0 has no watcher,
  so an edit made outside the kernel (an operator changing a room's file by hand) is not seen until
  something cuts its thread. The decision below assumed a watcher.

## Economics

- **Cacheable:** the room text and its parts, the place, the inventory, the affordances, the plan,
  and the stored queries and templates. Affordances are keyed by the capability fingerprint, so one
  player's list never leaks into another's.
- **What a move recomputes.** A move cuts the threads of the atoms it wrote, and everything that
  read one of them recomputes on its next read; nothing else does:
  - after a `take` or a `drop`: the room's contents line and room text, and the mover's inventory
    (the room's contents atom and the mover's carries atom);
  - after a `go`: the mover's place, and so their look.

  Every other room, its text included, every other inventory, and the moved thing's own description
  stay cached. A verb that rewrites a thing also recomputes the contents line of the room it is in
  (or the inventory of whoever carries it), because those read the thing's atom for its label and
  description. Recomputing is lazy: a room nobody looks at costs nothing until someone does.
- **Cache entries.** More, smaller resources means more entries: a scan of every room in a
  300-room realm holds about 6,000 entries (it held about 4,500 before), above ikigai-core's
  default bound of 4,096. With the default bound such a scan evicts, and a look whose answer was
  evicted recomputes (about 17 ms, below). A host serving a realm that large sets the bound
  (`Kernel::with_cache_policy`).
- **Never joined into a cached resource:** anything uncacheable. There is no LLM in this build.

## Refusals

| case | error |
|---|---|
| `go north` where there is no north exit | **Conflict**: the state says no |
| `take lamp` when the lamp is not here, is carried already, or does not exist | **Conflict** |
| `drop lamp` when you do not carry it | **Conflict** |
| a take or a drop that loses a race for the thing (someone else moved it first), or whose atoms kept changing | **Conflict**: "Someone else took the lamp first: …" |
| a take or a drop that stopped between its two writes | **Conflict**, saying the thing is in two places and which move puts it right |
| a direction, thing or verb that is not a name | **InvalidArgument** |
| a caller who plays no character, or a principal that is not a character | **Denied** |
| a move or a verb without `urn:cap:resmud:play` or `urn:cap:resmud:verb:run` | **Denied**, by the declared capability floor |
| a verb nobody published (`dance`), checked before its author or its thing | **NotFound**: "There is no verb called dance." |
| a verb whose signature fails, or that names no author or an unknown one | **Denied** |
| a verb that writes what its author's grant and its caller's capability together do not cover | **Denied** |
| a verb that would write a sealed name | **Denied** |
| a thing created without a text description | **InvalidArgument** from the shapes (the accessibility floor) |
| a verb whose source is not a `(graph …)`, or that writes nothing | **InvalidArgument** |
| a verb that needs a thing and is given none, or a blank one | **MissingArgument** |
| `go`, `take` or `drop` given no word, or a blank one | **MissingArgument** |
| writing an atom by its public name | **Unsupported** |
| reading or writing the realm's files from outside it | **Unresolved** |

Every row is tested in `tests/refusal.rs`, `tests/play.rs` or `tests/move.rs`, including the verb runner's two
quieter paths, a verb name that is not a name and a plan that writes nothing (PR 4). A refused
request writes nothing. A blank word counts as a word left out because the `ikigai` REPL sends a
`sink` with nothing after its target as empty content, never as none.

## Done means

One test plays this through, on the text face (`tests/play.rs`,
`the_smallest_playable_thing_plays_through_on_the_text_face`), and it passes. A second plays the
same steps as the words a player types at the door, with the same expected text (`tests/door.rs`,
`the_smallest_playable_thing_plays_through_the_door`):

1. `look` lists the room's description, its exits, and the lamp.
2. `take lamp`; `look` again: the lamp is gone from the room's text, and the character carries it.
   The room text was recomputed because its thread was cut, not because anything rewrote it: the
   room's own atom is byte-for-byte unchanged.
3. A player publishes `polish`, signed. `polish lamp` changes the lamp's description.
4. A copy of `polish` with one byte changed is refused (Denied).
5. A verb that tries to write the realm's shapes is refused (Denied).
6. Creating a thing with no description is refused by the shapes.

## Built on

All published on ikigai-core 0.3: `ikigai-core` 0.3.0, `ikigai-fs` 0.4.0 (the atoms are files),
`ikigai-sexpr` 0.4.0 (the verb language), `ikigai-sign` 0.5.0 (signatures), `ikigai-fn` 0.10.0 (the
templates), `ikigai-sparql` 0.5.0 and `ikigai-rdf` 0.5.0 (the queries and the union),
`ikigai-vocab` 0.3.0, and `ikigai-engine` 0.3.0 with `ikigai-resolve` 0.3.0 (the door). The vocabulary is `resmud-vocab` (`rm:` and `rms:`).

## Decided (2026-11-03)

- **The verb language: s-expressions**, compiled to a graph patch through `ikigai-sexpr`, which is on
  core 0.3 today. Steel (`ikigai-lisp`, where the builtins are the kernel verbs) can follow once it
  moves to core 0.3; TypeScript and WebAssembly verbs after that.
- **The first door: the `ikigai` REPL.** Built (above). A line-protocol telnet door comes after
  this build.
- **Where the atoms live: files under a realm directory**, through `ikigai-fs` and golden-threaded by
  its watcher; `ikigai-store` once it is on core 0.3, if a realm outgrows files.

## The two-write move

A take or a drop writes two atoms, where the thing goes and where it was, and files have no
transaction. So:

- **The destination first.** The edge writes the atom the thing joins, then takes it out of the
  atom it leaves. A failure between the two leaves the thing listed in both places, never in
  neither. The player is told so in plain words, with the move that puts it right ("The move did
  not finish: the lamp is in two places now, with you and in the kitchen; drop it to put it
  right."), and that move does: dropping a thing the room already lists only takes it out of the
  carries atom. `tests/move.rs` simulates the failure (a capability that may write what a
  character carries but not what a room contains) and pins both files and the sentence.
- **A compare-and-swap on both writes.** `ikigai-fs` 0.4.1 honors `if-match` on a Sink: the file is
  replaced only while it is still the version the content id names, checked under a lock on its
  directory, and otherwise the Sink is refused as Conflict. The edge reads each atom's version,
  then judges and rewrites that version. A destination that changed is read again and the move
  retried (up to four times; nothing has been written yet). A source that changed is read again
  too: if the thing is still there it is taken out of the new version; if not, someone else moved
  it first, so the edge takes it back out of the destination and refuses the move ("Someone else
  took the lamp first: look to see what is in the kitchen."). `tests/move.rs` runs two players
  taking one lamp at once, on two threads, for a dozen rounds, and checks that exactly one holds it
  each time. In practice both players' first writes land and one loses the second, so the test
  goes through the lost-race path every round.
- **What is left.** While a move is in flight the thing is briefly listed in both places, so a look
  in that moment sees it twice. A version read from a cache that another process's write made
  stale (two kernels on one realm directory, which the design does not do) fails every `if-match`
  and refuses the move after four attempts, rather than writing over that process's change.
  A store with transactions (`ikigai-store`, once it is on core 0.3) would make the move one write.

## Decided: option 1, a contents atom per room

**Decided by Brian on 2026-11-07 (resmud#52), and built.** In the first build a thing's location
lived in its own atom, so any move made every room recompute its contents, and each recompute ran
the `content` query over the whole world. The three options were a contents atom per room (and a
carries atom per character), an inverted location index, or accepting the cost. Option 1 was
chosen because it is the only one that also lets a verb place what it creates, and it keeps every
dependency honest, with no thread a writer must remember to cut. Its cost is the two-write move,
settled above.

**Measured** before and after, on a generated realm of 300 rooms in a line and 300 things
(`tests/scale.rs`, ignored by default; `cargo test --release --test scale -- --ignored
--nocapture`), on an Apple M5 Max (18 cores, 128 GB), release build, medians of 9 rounds. Each
round reads every room's text, then alice's look, takes a thing, looks, reads a room far away,
drops the thing and looks again. The before numbers ran the same harness against main with the
first build's realm shape:

| read | before, default cache | after, default cache | before, unbounded | after, unbounded |
|---|---|---|---|---|
| a look, nothing moved | 26 µs | 17.5 ms | 23 µs | 8 µs |
| a look right after a take | 46.9 ms | 7.4 ms | 44.7 ms | 7.3 ms |
| a look right after a drop | 47.0 ms | 7.2 ms | 44.4 ms | 7.4 ms |
| a room elsewhere, right after a take | 23.5 ms | 18 µs | 22.4 ms | 19 µs |

- A look right after a move is about six times faster, and a room elsewhere no longer recomputes
  at all: it is a cache hit, about a thousand times faster. Before, every room recomputed after
  every move, so a scan of all 300 rooms after one move cost about 7 s; now it recomputes one.
- The "after, default cache" look with nothing moved is eviction, not invalidation: the scan of
  every room needs about 6,000 cache entries, above the default bound of 4,096 (see
  [Economics](#economics)). With a bound that evicts nothing it is 8 µs.

**For resmud#56** (should `look X` see only what is here): option 1 makes it cheap. Whether a thing
is here is now a question over two small atoms, the room's contents and the player's carries,
instead of the world, so the door's `look X` can check them before describing the thing.
