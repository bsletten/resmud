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
- **What you can do here** (`urn:resmud:affordance`): one action per line, in plain words, each
  saying what it takes (`take a thing: pick up a thing that is here`). No angle brackets: a screen
  reader may read them aloud as "less than" and "greater than".
- **The answer to a move or a verb**: one sentence (`You take the lamp.`, `You go east, to the
  garden.`, `You polish the lamp.`).
- **Refusals**: one sentence in plain words saying what is wrong and, where it can, what to do
  instead (`There is no exit north from the kitchen: look to see the exits.`). A door shows a
  player `resmud::refusal(&error)`, never the error itself (see the rules below).

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
  `a_thing_without_a_text_description_is_refused_by_the_shapes`). The refusal names the thing by
  its label (or its key) and states the rule in words (`The realm's rules refuse the verb conjure:
  the pebble has no description, and every thing needs a text description, so give it one.`).
- **What you can do here is always an enumerable list.** The affordance list is the realm's allowed
  actions that the caller's own capability offers, one per line, and an action the list does not
  offer is refused rather than hidden (`tests/play.rs`,
  `the_affordance_is_the_realms_actions_under_the_callers_capability`).
- **A refusal is a sentence in plain words, never an IRI.** ResMUD's own refusals (the moves', the
  verb runner's, the shapes') name things by the word a player sees: a thing, a room, a verb or an
  author by its key or label (`the lamp`, `the kitchen`, `polish`, `alice`), never by IRI, and say
  what to do instead where they can. Some refusals a player can reach are the kernel's own, raised
  before any ResMUD code runs: the capability floor, a name nothing answers, a verb a resource does
  not answer, a word left out. Those name resources by IRI, so they are mapped at the edge:
  `resmud::refusal` passes a message through only when it is plain (no `urn:` starting a word, no
  `://`, no angle brackets, no backticks: `resmud::is_plain`), and otherwise gives a plain sentence
  for that kind of refusal (`You may not do that here: ask what you can do here to see what you
  may.`). Every refusal ends as a sentence: a capital letter and a period. `tests/refusal.rs` pins
  the exact sentence of every refusal it reaches, and checks each is plain;
  `src/plain.rs` checks that every kind of kernel error maps to a plain sentence.
- **No angle brackets in what a player reads.** The affordance list says what each action takes in
  words (`go a direction`, `a verb a player wrote, then a thing`), and the plain-text test checks
  every answer of the text face, the affordance list included, with `resmud::is_plain`.

## How it is tested

- `cargo test` plays the design's six steps through as one test on the text face, and pins the
  exact text of look, the affordance list and every refusal a test reaches, as the player reads it
  (`resmud::refusal`).
- **No automated accessibility checker, screen reader, braille display or other assistive
  technology has been used**, and no person has played it in a terminal yet.

## Known limitations

- **No door yet.** Nothing presents this output in a terminal, so how it reads at a screen reader
  is untested.
- **A door must call `resmud::refusal`.** The mapping lives at the edge, and there is no door yet:
  the error itself still carries the kernel's words (with IRIs) for a programmer, and a client that
  prints the error instead of `resmud::refusal` of it shows a player those words.
- **A kernel refusal mapped at the edge loses its specifics.** `You may not do that here` does not
  say which capability is missing, and `There is nothing by that name in this realm` does not say
  which name: the kernel's message names them only by IRI. Better words need the door to know what
  the player asked for.
- **"Ask what you can do here" has no word of its own yet.** It is `source urn:resmud:affordance`
  until a door gives it one.
- **What a player typed is echoed in quotation marks** (`"Lamp!" is not the name of a thing`); how a
  screen reader reads the quotation marks is untested.
- **Most sentences name a thing by its key, not its label.** Only the shapes' refusal reads a
  thing's `rdfs:label`; the rest use its key (`the lamp`), which is the same word in this realm but
  need not be in another.
- **A signature's verdict is not shown to a player.** A tampered verb and one signed by someone
  else get the same sentence; which key or hash failed is in the verdict, not the refusal.
- **The shapes cover only what a verb writes.** A realm's own files, written by its operator, are
  not checked, and the check reads only `sh:targetClass`, `sh:path` and `sh:minCount`.
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
| 2026-11-03 | Refusals in plain words, mapped at the edge by `resmud::refusal`; no angle brackets in the affordance list. |
