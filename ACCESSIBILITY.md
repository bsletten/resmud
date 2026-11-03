# Accessibility

What ResMUD emits that people read, the rules that output follows, how it is tested, and how to
report a barrier. ResMUD is a text-first game: every room, thing and action is meant to answer in
plain text before any other face, and the list of what you can do is always a list. It is not a
conformance claim. Adapted from the [ACCESSIBILITY.md template](https://github.com/mgifford/ACCESSIBILITY.md) (MIT).

| Field | Value |
|---|---|
| Target | WCAG 2.2 Level AA, for any page or terminal that presents this game |
| Status | Evaluation in progress. No conformance claim. |
| Owner | the maintainers of this repository |
| Report a barrier | [a GitHub issue](https://github.com/bsletten/resmud/issues/new?labels=accessibility) with the `accessibility` label; anything security-sensitive through [private vulnerability reporting](https://github.com/bsletten/resmud/security/advisories/new) |
| Last reviewed | 2026-11-03 |

## What it emits that people read

The first playable build ([docs/design/smallest-playable.md](docs/design/smallest-playable.md))
emits, as `text/plain; charset=utf-8`:

- **The text face of a room** (`urn:resmud:view:room:id:{room}`): three lines, its description,
  what can be taken there (`You see the lamp. A brass lamp, dull with age.`), and its exits
  (`Exits: east.`).
- **Look** (`urn:resmud:look`): the room text of where the player's character is, then a line for
  what it carries (`You carry nothing.`).
- **What you can do here** (`urn:resmud:affordance`): one action per line, each with what it takes
  in angle brackets (`take <thing>: pick up a thing that is here`).
- **The answer to a move or a verb**: one sentence (`You take the lamp.`, `You go east, to the
  garden.`, `You polish the lamp.`).
- **Refusals**: a sentence saying what is wrong and, where it can, what to do instead (`there is
  no exit north from the kitchen: look to see the exits`).

The words come from the realm's own files (`realm/first/`) and from the stored queries that phrase
them (`assets/query/`), so changing what a room says is changing data, not code.

Not this repository's: the terminal, editor or page a player shows any of this in. There is no
door yet; the tests issue the requests a REPL would.

## The rules that output follows

What the build now holds, and how:

- **Text is canonical.** Every answer above is `text/plain`, with no terminal escape codes (so no
  color) and no markup. Tested for look, the room text, the affordance list, every move and a
  verb (`tests/play.rs`, `every_answer_of_the_text_face_is_plain_text`).
- **Nothing a verb writes can exist without a text description.** The realm's shapes require
  `rm:description` on every item, character and room, and the verb runner checks every atom a verb
  would write against them before writing anything (`tests/refusal.rs`,
  `a_thing_without_a_text_description_is_refused_by_the_shapes`). A refusal names the thing and
  the rule.
- **What you can do here is always an enumerable list.** The affordance list is the realm's allowed
  actions that the caller's own capability offers, one per line, and an action the list does not
  offer is refused rather than hidden (`tests/play.rs`,
  `the_affordance_is_the_realms_actions_under_the_callers_capability`).
- **A refusal is a sentence that says what is wrong.** The moves' refusals are tested by their
  words in `tests/refusal.rs`: the exact sentence for most, a key phrase for the rest.

## How it is tested

- `cargo test` plays the design's six steps through as one test on the text face, and pins the
  exact text of look, the affordance list and most of the moves' refusals.
- **No automated accessibility checker, screen reader, braille display or other assistive
  technology has been used**, and no person has played it in a terminal yet.

## Known limitations

- **No door yet.** Nothing presents this output in a terminal, so how it reads at a screen reader
  is untested.
- **Some refusals name resources by IRI**, not in plain words: the verb runner's
  (`urn:resmud:thing:id:pebble has no rm:description, …`) and the kernel's own (a capability
  denial, a name a caller may not write). A player should not have to read an IRI to understand
  why.
- **The shapes cover only what a verb writes.** A realm's own files, written by its operator, are
  not checked, and the check reads only `sh:targetClass`, `sh:path` and `sh:minCount`.
- **The affordance list uses angle brackets** for what an action takes (`<thing>`); a screen reader
  may read them aloud as "less than" and "greater than".
- Live updates (a room's text changing as its contents change) will need a way to pause them (WCAG
  2.2.2). This build pushes nothing: text changes only when the player asks again.

## Contributing

Read this file before changing any output listed above. Keep a refusal a sentence that says what is
wrong, where, and what to do. Say in the pull request what was tested, with what, and what was not.

## Reporting a barrier

[Open an issue](https://github.com/bsletten/resmud/issues/new?labels=accessibility) with the
`accessibility` label (the link sets it when GitHub lets you add labels; otherwise a maintainer adds
it): what you were doing, what happened, and your terminal, editor or browser and any assistive
technology, if you are comfortable sharing them. Anything security-sensitive goes through
[private vulnerability reporting](https://github.com/bsletten/resmud/security/advisories/new).
**We acknowledge a report within 7 days**, and promise nothing further.

## Change history

| Date | Change |
|---|---|
| 2026-11-03 | First version, with the repository. |
| 2026-11-03 | The first playable build: what its text face emits, the rules it now holds and how they are tested. |
