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
| thing | `urn:resmud:thing:id:{thing}` | **atom** (`thing/{thing}.ttl`) | `rm:Item`, its label, `rm:description`, and **where it is**: `<room> rms:contains <thing>` or `<character> rm:carries <thing>`, in the thing's own atom |
| character | `urn:resmud:character:id:{character}` | **atom** (`character/{character}.ttl`) | `rm:Character`, its label, `rm:description`, and where it is (`<room> rms:contains <character>`). It does **not** list what it carries |
| author | `urn:resmud:author:id:{author}`, `…:key` | **atom** (`author/{author}.ttl`, `author/{author}.pub.pem`) | the author's registered key (`rml:key`) and the grant their verbs run under (`rml:grant`, game tokens such as `urn:cap:resmud:write:thing`) |
| world | `urn:resmud:world` | **atom** (`world.txt`, a list of names) | the realm's atoms. The union is `urn:sparql:*`'s own: each listed graph is loaded and the default graph is their union |
| place | `urn:resmud:character:id:{character}:place` | composition | the stored `place` query over the character's atom alone; answers the room's key |
| inventory | `urn:resmud:character:id:{character}:inventory` | composition | the stored `inventory` query over the world |
| room parts | `urn:resmud:room:id:{room}:description`, `…:exit`, `…:content` | composition | stored queries: description and exits over the room's atom alone; contents over the world |
| thing description | `urn:resmud:thing:id:{thing}:description` | composition | the stored `description` query over the thing's atom alone |
| room text | `urn:resmud:view:room:id:{room}` | composition | `urn:iki:fn:compose` fills the `room` template with the three room parts; `text/plain` |
| look | `urn:resmud:look` | **alias rule** in the personal corridor | → `urn:resmud:character:id:{character}:look`, which fills the `look` template with the room text and the inventory. The room is a reference to the character's place, which the kernel settles to the place's answer, so no code reads it |
| manifold | `urn:resmud:manifold` | named request | `urn:kernel:actions` as Turtle, under the caller's capability |
| affordance | `urn:resmud:affordance` | composition | the stored `affordance` query over the manifold and the realm atom: the realm's allowed actions that the caller's own manifold offers, one line each |
| go, take, drop | `urn:resmud:verb:name:{go\|take\|drop}` | **Sink edge** (code) | each decides, writes **one** atom with the stored `relocate` CONSTRUCT, or refuses |
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

Of the realm's 29 names, 22 are rules over other names and 7 are code. `urn:resmud:look` is not
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
| `help` or `what` | `source urn:resmud:affordance` |
| `go east`, `take lamp`, `drop lamp` | `sink urn:resmud:verb:name:{go,take,drop} …` |
| `polish lamp` (any other first word) | `sink urn:resmud:verb:id:polish lamp` |
| `source …`, `sink …`, `exists …` | the line itself |
| `quit` | leaves |

It prints plain text only: a refusal is `resmud::refusal` of the kernel's typed error (the door's
resolver keeps each line's first error, because the engine hands a frontend only its own message,
which names resources by IRI), and an answer that is not plain, such as the Turtle of
`urn:resmud:manifold`, is withheld with a sentence saying so. `tests/door.rs` replays the six steps
of [Done means](#done-means) through the door with the same expected text; `tests/play.rs` still
plays them against the kernel.

### Why each piece of code is code

- **go, take, drop** (`src/edge.rs`): each changes state and can refuse, and a refusal is part of
  its contract. A generic composer cannot say *no*. Everything else each one needs is a name: the
  actor's place, where an exit leads and who holds a thing (stored queries), and the rewritten atom
  (the stored `relocate` CONSTRUCT). The edge only decides, and writes the answer to one file.
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

- **A thing's location is in its own atom.** The design said both that each move writes one atom
  and that a room elsewhere stays cached when a thing moves. Both cannot hold. As built,
  `<kitchen> rms:contains <lamp>` lives in `thing/lamp.ttl`, so `take` writes only the lamp's atom
  and `go` writes only the character's. **The cost** is that every room's contents now depend on
  every thing's atom: taking the lamp makes every room recompute its contents line, and so its room
  text, not only the kitchen's. A room's description and exits read only the room's own atom, and
  stay cached. A test pins exactly this split, so it cannot change unnoticed
  (`tests/play.rs`, `a_take_recomputes_what_read_the_taken_thing_and_keeps_the_rest`). See
  [Open: the caching cost](#open-the-caching-cost).
- **A character's atom does not list what it carries.** Each carried thing's atom says who carries
  it (`rm:carries`), for the same one-write reason.
- **A verb cannot yet place a thing it creates.** A verb's plan rewrites atoms by subject, and the
  triple that places a thing has the room as its subject, so the runner reads it as a write to the
  room's atom. The author's grant covers things, not rooms, so the clamp refuses it. A verb can
  create a pebble that has a description; the pebble joins the world's list, but it is nowhere.
  A test pins the refusal (`tests/play.rs`, `a_verb_can_create_a_thing_that_has_a_description`).
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
- **What a move recomputes.** A move writes one atom and cuts its thread. Everything that read that
  atom recomputes on its next read, and nothing else does:
  - every room's contents line, and so its room text, because contents are a question over the
    world;
  - every character's inventory, for the same reason;
  - the moved thing's own description;
  - after a `go`, the mover's place.

  Each room's description and exits, and every other character's place, stay cached. Recomputing is
  lazy: a room nobody looks at costs nothing until someone does.
- **Never joined into a cached resource:** anything uncacheable. There is no LLM in this build.

## Refusals

| case | error |
|---|---|
| `go north` where there is no north exit | **Conflict**: the state says no |
| `take lamp` when the lamp is not here, is carried already, or does not exist | **Conflict** |
| `drop lamp` when you do not carry it | **Conflict** |
| a direction, thing or verb that is not a name | **InvalidArgument** |
| a caller who plays no character, or a principal that is not a character | **Denied** |
| a move or a verb without `urn:cap:resmud:play` or `urn:cap:resmud:verb:run` | **Denied**, by the declared capability floor |
| a verb whose signature fails, or that names no author or an unknown one | **Denied** |
| a verb that writes what its author's grant and its caller's capability together do not cover | **Denied** |
| a verb that would write a sealed name | **Denied** |
| a thing created without a text description | **InvalidArgument** from the shapes (the accessibility floor) |
| a verb whose source is not a `(graph …)`, or that writes nothing | **InvalidArgument** |
| a verb that needs a thing and is given none, or a blank one | **MissingArgument** |
| `go`, `take` or `drop` given no word, or a blank one | **MissingArgument** |
| writing an atom by its public name | **Unsupported** |
| reading or writing the realm's files from outside it | **Unresolved** |

Every row is tested in `tests/refusal.rs` or `tests/play.rs`, including the verb runner's two
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

## Open: the caching cost

**A decision for Brian; not taken.** A thing's location lives in its own atom, so any move makes
every room recompute its contents (above). At two rooms and one lamp that costs nothing. In a realm
of hundreds of rooms, where players move every few seconds, nearly every `look` would recompute its
room's contents, and each recompute runs the `content` query over the whole world, every atom
loaded. Nothing has been measured. Three ways to go:

1. **A contents atom per room.** A room's contents move out of the things' atoms into an atom of
   their own (`urn:resmud:room:id:{room}:content`, a file beside the room's), and a character's
   carried things into one per character. Its description and exits stay where they are.
   - Gains:
     - A move recomputes only the rooms it touches, and a contents query reads one small atom
       instead of the world.
     - A thing's own description stops recomputing when it moves.
     - It gives a verb somewhere to put what it creates: a grant could cover a room's contents
       without covering its description, which fixes "a verb cannot place a thing it creates".
   - Costs:
     - A move writes two atoms (where the thing was, and where it is now), and files have no
       transaction. A failure between the two writes leaves a thing in two places or in none.
     - Two players taking from one room write the same atom, and the last write wins. The edges
       would need to order the writes so that a failure is visible (write the destination first,
       so the worst case is a thing listed twice) or a compare-and-swap.
     - The realm's files and the stored queries change shape.
2. **An inverted location index.** Location stays in the thing's atom as the one truth, and a new
   per-room index answers what a room contains. The index hangs on one thread per room, which the
   moves cut for the two rooms involved, instead of on every thing's atom.
   - Gains:
     - Still one write per move, and rooms elsewhere stay cached.
   - Costs:
     - The index is code. It has to answer with its own per-room threads instead of the threads of
       the atoms it read. Whether ikigai-core lets an endpoint do that is not checked.
     - Correctness then rests on every writer cutting the right two threads. A verb, a future
       watcher or an out-of-band edit that moves a thing without going through the edges leaves
       the index stale, silently. That trades a cost you can see for a bug you cannot, which is
       the trade golden threads exist to avoid.
3. **Accept the cost.** Keep the build as it is: a move recomputes every room's contents, and
   descriptions and exits stay cached.
   - Gains:
     - No new code, no new atoms, and the one-write move stays.
     - Recomputing is lazy, so only rooms someone looks at pay.
     - The test that pins the split would catch any change.
   - Costs:
     - The cost grows with rooms times moves, and each recompute reads the whole world. "A room
       elsewhere stays cached", the design's first hope, stays false.
     - A verb still cannot place what it creates.

**Recommendation: option 1, a contents atom per room, after measuring.**
- It is the only option that also fixes a second open problem, placing what a verb creates.
- It keeps every dependency honest, with no thread a writer must remember to cut.
- It makes a contents query read one atom instead of the world.

Its real cost is the two-write move, and that should be settled first (the write order above, or a
store with transactions once `ikigai-store` is on core 0.3).

Before changing anything, measure a generated realm of a few hundred rooms and things: a `look`
after a move, before and after. Until a realm is that large, option 3 is the honest default.
