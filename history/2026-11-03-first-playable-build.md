# The first playable build (2026-11-03)

On 3 November 2026 ResMUD became something you can play, if only by its tests: one realm, two rooms,
a lamp, a character, and a verb a player wrote and signed. The design was proposed, decided and built
in one day ([PR 1](https://github.com/bsletten/resmud/pull/1), the design;
[PR 2](https://github.com/bsletten/resmud/pull/2), the build).

## What it was meant to show

ResMUD's claim is that a world programmable from inside itself, the thing a MOO was, comes out of a
resource-oriented kernel almost for free. The first build was chosen to test exactly four parts of
that claim and nothing else:

1. **The selection funnel is the parser.** There is no command parser. What you can do in a room is
   the list of actions your own capability allows there (`urn:kernel:actions`), narrowed to what the
   realm permits. Something the list does not offer is refused, never hidden.
2. **The world is live.** A room's text is a query over what is in it, filled into a template. Take
   the lamp and the room's text changes because a golden thread was cut, not because anything
   rewrote a description.
3. **A player can program the world, and the program can never outrank its author.** A verb is an
   s-expression the player wrote, signed with their key and compiled to a graph patch by a
   transreption. It runs under the caller's capability narrowed to the author's grant. A tampered
   copy is refused. A verb that tries to change the realm's rules is refused, because the realm, its
   shapes and its authors are sealed names.
4. **Text is canonical.** Every answer is plain text, and nothing can be created without a text
   description: the realm's shapes refuse it.

All four hold, each pinned by a test (`tests/play.rs`, `tests/refusal.rs`).

## Why it matters

- **The game is mostly names.** Of the realm's 29 names, 22 are rules over other names: stored
  queries, a text template, aliases, the kernel's own action list, and the verb's transreption. Only
  seven are code, and each says why it has to be: `go`, `take` and `drop` change state and can
  refuse; the verb runner checks a signature, narrows a capability and writes a patch; the rest serve
  the stored queries and templates.
- **Capabilities became gameplay.** LambdaMOO's famous trouble was authority that could be neither
  earned nor bounded, so it had to be argued over socially. Here the bound is mechanical: a verb runs
  under the intersection of what its caller and its author hold, and a sealed name cannot be written
  by any verb at all. The rule that keeps an agent safe is the rule that keeps a player's program in
  bounds.
- **It ran on the day's releases.** Every crate it uses was published on ikigai-core 0.3 that same
  day, in the first waves of the core 0.3 cascade: `ikigai-core` 0.3.0, `ikigai-fs` 0.4.0,
  `ikigai-sexpr` 0.4.0, `ikigai-sign` 0.5.0, `ikigai-fn` 0.10.0, and `ikigai-sparql` and `ikigai-rdf`
  0.5.0. No path overrides, nothing unpublished.

## What the build taught the design

Building it showed the design was wrong in five places; the corrections are recorded in PR 2 and are
the next design change:

- A thing's location has to live in the thing's own atom, so each move writes one atom. The cost is
  that taking anything makes every room recompute what it contains; descriptions and exits stay
  cached. A test pins that cost so it cannot change unnoticed.
- A character's atom does not list what it carries; each carried thing says who carries it.
- A verb cannot yet place a thing it creates, because placing it writes the room, which the author's
  grant does not cover.
- "One room" needed two, because an exit has to lead somewhere.
- Who wrote a verb needed an atom of its own, checked against the author's registered key.

## What it is not yet

There is no door. The `ikigai` REPL cannot host it until `ikigai-engine` moves to ikigai-core 0.3,
so the tests issue the same requests a REPL session would (`source urn:resmud:look`,
`sink urn:resmud:verb:name:take lamp`). The realm's rules are checked by a stored query rather than
a SHACL engine, until `ikigai-shacl` moves too. There is one realm, no travel, no game master, no
economy and no graphics: each is its own design thread, for later.
