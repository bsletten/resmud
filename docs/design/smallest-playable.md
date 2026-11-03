# The smallest playable thing

The first build of ResMUD: **one realm, one room, the text face, and one signed verb a player
wrote.** It is chosen by build-readiness, not by excitement: every piece below already exists in a
published ikigai crate, and the build is mostly naming. It should prove four things and nothing
else:

1. **The selection funnel is the parser.** What you can do here is `urn:kernel:actions` under your
   capability; there is no parser.
2. **The world is live.** A room's text is derived from its contents, so taking a thing changes
   what everyone sees, with no code that updates a description.
3. **A player can program the world.** A verb a player wrote, signed by its author, runs under no
   more authority than its author holds.
4. **The text face is complete.** Everything the game can say, it says in `text/plain`.

Out of scope for this build: instances, a second realm, travel, the LLM game master, the economy,
any graphics. Each is its own design thread, for later.

## Resource map

Names are singular, the type first, and every partition is by a named key.

| set | name | kind | how |
|---|---|---|---|
| realm | `urn:resmud:realm:id:{realm}` | **atom** (a Turtle file) | the realm's label, the IRI of its shapes and the verbs its manifold allows |
| realm shapes | `urn:resmud:realm:id:{realm}:shape` | **atom** (a Turtle file) | SHACL: what may exist here; the first rule is the accessibility floor, *every thing has a text description* |
| room | `urn:resmud:room:id:{room}` | **atom** | `rms:Room`, `rm:description`, its exits (`rms:exit` → `rms:leadsTo`) |
| thing | `urn:resmud:thing:id:{thing}` | **atom** | `rm:Item` with `rm:description`; where it is (`rms:contains` from its room, or `rm:carries` from a character) |
| character | `urn:resmud:character:id:{character}` | **atom** | `rm:Character`, its place, what it carries |
| world | `urn:resmud:world` | list of names | the realm's atoms, unioned (`urn:rdf:union`); no code |
| place | `urn:resmud:character:id:{character}:place` | composition | a SPARQL `SELECT` over the world |
| room text | `urn:resmud:view:room:id:{room}` | composition | a SPARQL `SELECT` (description, contents, exits) filled into a template with `ikigai-fn`; `text/plain` |
| look | `urn:resmud:look` | **alias rule** | the room text of the caller's place, through the personal corridor below |
| affordance | `urn:resmud:affordance` | composition | `urn:kernel:actions` under the caller's capability, narrowed to the realm's manifold: *what can I do here* |
| go, take, drop | `urn:resmud:verb:name:{go\|take\|drop}` | **Sink edge** (code) | each writes one atom; refusals below |
| verb source | `urn:resmud:verb:id:{verb}:source` | **atom** | the verb as an s-expression, with its author's `ikigai-sign` signature graph |
| verb plan | `urn:resmud:verb:id:{verb}:plan` | **transreption** | the source compiled to a graph patch by `ikigai-sexpr`; golden-threaded from the source |
| verb | `urn:resmud:verb:id:{verb}` | **Sink edge** (code) | verify the signature, clamp to the author's grant, apply the plan |

### Spaces

- **Realm corridor.** A scope per realm binds its atoms and its shapes. Two realms are two
  corridors over the same names, not two programs.
- **Personal corridor.** The caller's principal resolves to a character, so `look` and
  `affordance` take no argument: they mean *here, for me*.
- **Sealed names.** No verb may write `…:shape` or the realm's manifold: changing what may exist
  is legislation, not play.
- **Later:** the instance corridor (an ephemeral resolution of the realm under a party) and the
  as-of corridor (the realm before a contested change).

### Views

`text/plain` is the canonical face and the accessible one: every room answers in text, and the
manifold is always a list, so nothing is ever a hidden clickable. A Turtle face (`as=text/turtle`)
serves agents. The first client is the `ikigai` REPL (`source urn:resmud:look`); a line-protocol
door for a literal telnet session comes after.

### Why each piece of code is code

- **go, take, drop**: each changes state and can refuse, and a refusal is part of its contract. A
  generic composer cannot say *no*.
- **The verb runner**: it checks a signature, intersects two capabilities and writes a patch. Each
  step exists already (`urn:sign:verify`, the kernel's attenuation, a Sink); the runner only joins
  them. It should stay short enough to read in one sitting.

Everything else is a name over a file, a query, a template or an alias.

## Economics

- **Cacheable:** the room text, the place and the affordances. The room text hangs on the room's
  atom and the atoms of what it contains, so taking the lamp cuts exactly that room's thread; a
  room elsewhere stays cached. Affordances are keyed by the capability fingerprint, so one
  player's list never leaks into another's.
- **Never joined into a cached resource:** anything uncacheable. There is no LLM in this build.

## Refusals

| case | error |
|---|---|
| `go north` where there is no north exit | **Conflict**: the state says no |
| `take lamp` when the lamp is not here | **Conflict** |
| a direction or thing that is not a name | **InvalidArgument** |
| a verb whose signature fails, or whose author does not hold what it needs | **Denied** |
| a verb that would write a sealed name | **Denied** |
| a thing created without a text description | **InvalidArgument** from the shapes (the accessibility floor) |

## Done means

One test plays this through, on the text face:

1. `look` lists the room's description, its exits, and the lamp.
2. `take lamp`; `look` again: the lamp is gone from the room's text, and the character carries it.
   The room text was recomputed because its thread was cut, not because anything rewrote it.
3. A player publishes `polish`, signed. `polish lamp` changes the lamp's description.
4. A copy of `polish` with one byte changed is refused (Denied).
5. A verb that tries to write the realm's shapes is refused (Denied).
6. Creating a thing with no description is refused by the shapes.

## Built on

All published on ikigai-core 0.3: `ikigai-core`, `ikigai-fs` (the atoms are files),
`ikigai-sexpr` (the verb language), `ikigai-sign` (signatures), `ikigai-fn` (the template), and
`ikigai-sparql` with `ikigai-rdf` (queries and union), whose core 0.3 releases are in the current
publish batch. The vocabulary is `resmud-vocab` (`rm:` and `rms:`).

## Decided (2026-11-03)

- **The verb language: s-expressions**, compiled to a graph patch through `ikigai-sexpr`, which is on
  core 0.3 today. Steel (`ikigai-lisp`, where the builtins are the kernel verbs) can follow once it
  moves to core 0.3; TypeScript and WebAssembly verbs after that.
- **The first door: the `ikigai` REPL.** A line-protocol telnet door comes after this build.
- **Where the atoms live: files under a realm directory**, through `ikigai-fs` and golden-threaded by
  its watcher; `ikigai-store` once it is on core 0.3, if a realm outgrows files.
