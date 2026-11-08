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
| Last reviewed | 2026-11-07 |

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
- **A move that did not go as asked**: a take or a drop writes two files, so it can lose a race
  or stop halfway, and either is said in a sentence. A player who loses a race for a thing is
  told so (`Someone else took the lamp first: look to see what is in the kitchen.`) and holds
  nothing. A move that stopped between its two writes says where the thing is now and the word
  that puts it right (`The move did not finish: the lamp is in two places now, with you and in
  the kitchen; drop it to put it right.`).
- **Refusals**: one sentence in plain words saying what is wrong and, where it can, what to do
  instead (`There is no exit north from the kitchen: look to see the exits.`). A door shows a
  player `resmud::refusal(&error)`, never the error itself (see the rules below).
- **The door** (`resmud::door`, run by `cargo run --example door`): the `ikigai` REPL opened
  for one player. It shows a prompt (`> `), reads a line, and prints the answer or the refusal
  as lines of plain text. It takes the words the affordance list and the refusals teach:
  `look`; `look` and a thing (`look lamp`, the thing's description); `help` or `what` (what you
  can do here); `go east`, `take lamp`, `drop lamp`; a verb a player wrote and a thing
  (`polish lamp`); `quit`. The door's own words are read in any case (`Look`, `LOOK`). It also takes the REPL's own
  `source`, `sink` and `exists` lines (`source urn:resmud:look`). The example's one greeting
  line says which words to start with.

The words come from the realm's own files (`realm/first/`) and from the stored queries that phrase
them (`assets/query/`), so changing what a room says is changing data, not code.

Not this repository's: the terminal, editor or page a player shows any of this in.

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
  may.`). At the door, where such a sentence points at what you can do here, it names the door's
  word for that instead (`You may not do that here: type help to see what you may.`), through
  `resmud::refusal_with_help`. Every refusal ends as a sentence: a capital letter and a period. `tests/refusal.rs` pins
  the exact sentence of every refusal it reaches, and checks each is plain;
  `src/plain.rs` checks that every kind of kernel error maps to a plain sentence.
- **A word that names no verb is told so.** The door reads any first word it does not know as a
  verb a player wrote, and the verb runner checks that the verb was published before it reads
  anything else, so a typo is refused in plain words (`There is no verb called dance.`), not with
  a sentence about the verb's author or the thing it was given (`tests/door.rs`,
  `tests/refusal.rs`, `a_verb_nobody_published_is_not_found_before_its_author_or_thing_is_read`).
- **The door prints plain text and nothing else.** A refusal at the door is
  `resmud::refusal_with_help` of the kernel's typed error, with the door's word `help`, never the engine's message (which names
  resources by IRI): the door keeps each line's first error for exactly this. A line the engine
  cannot read at all gets a plain sentence too. An answer that is not plain (a resource that
  answers Turtle, such as `source urn:resmud:manifold`) or that carries a control character (so
  no terminal escape and no color) is withheld, with a sentence that says so, rather than
  printed. Tested in `tests/door.rs`: the six steps of the play, every refusal it reaches and a
  sweep of odd lines, each checked with `resmud::is_plain` and for control characters.
- **A move never loses a thing silently.** A take or a drop writes where the thing goes before it
  takes it out of where it was, so a failure between the two leaves the thing listed twice, never
  nowhere, and the sentence says so and names the move that mends it (`tests/move.rs`,
  `a_failure_between_the_two_writes_leaves_the_thing_in_both_places_and_says_so`). Two players
  taking one thing at once end with exactly one holding it, and the other told why in plain words
  (`two_players_taking_one_thing_at_once_leave_exactly_one_holding_it`).
- **No angle brackets in what a player reads.** The affordance list says what each action takes in
  words (`go a direction`, `a verb a player wrote, then a thing`), and the plain-text test checks
  every answer of the text face, the affordance list included, with `resmud::is_plain`.

## How it is tested

- `cargo test` plays the design's six steps through as one test on the text face, and pins the
  exact text of look, the affordance list and every refusal a test reaches, as the player reads it
  (`resmud::refusal`). It plays them twice: once as requests to the kernel (`tests/play.rs`) and
  once as the words a player types at the door (`tests/door.rs`), with the same expected text.
- The door's line loop is tested with its input and output in memory: the prompt, the answers,
  and that `quit` or the end of the input ends it.
- **No automated accessibility checker, screen reader, braille display or other assistive
  technology has been used**, and no person has played it in a terminal yet.

## Known limitations

- **No person has played at the door yet.** It has been run in a terminal by a script only, so
  how it reads at a screen reader or a braille display is untested, and so is how the `> ` prompt
  is announced.
- **Only `resmud::door` is known to call `resmud::refusal`.** The error itself still carries the
  kernel's words (with IRIs) for a programmer, so a host that builds its own door on the engine
  or the kernel, instead of using this one, must call `resmud::refusal` itself, or
  `resmud::refusal_with_help` with its own word for what you can do here. Given no word,
  a refusal says "ask what you can do here", which names no word the host may lack.
- **A kernel refusal mapped at the edge loses its specifics.** `You may not do that here` does not
  say which capability is missing, and `There is nothing by that name in this realm` does not say
  which name: the kernel's message names them only by IRI. Better words need the door to know what
  the player asked for.
- **A visitor offered nothing gets an empty answer to `help`**, which the door prints as an empty
  line rather than a sentence.
- **Only the door's own words are read in any case.** A verb a player wrote, a thing and a
  direction are the realm's names, which are lowercase, so `Polish lamp`, `look Lamp` and
  `go East` are refused, each with a sentence saying how to spell it.
- **`look lamp` describes the lamp wherever it is.** It reads the thing's description by name and
  does not check that the thing is here, so a player can look at a thing in another room if they
  know its name. Whether looking should be limited to what is here is a rule of the game the door
  does not make. `look` and the name of a room or a character is not mapped (`There is nothing by
  that name in this realm.`).
- **The REPL's grammar at the door can narrow a session.** `sink urn:host:login …` (the engine's
  own login) lowers the session to what it names, and never above the player's capability; a
  player who does it is refused until `sink urn:host:logout`.
- **What a player typed is echoed in quotation marks** (`"Lamp!" is not the name of a thing`); how a
  screen reader reads the quotation marks is untested.
- **Most sentences name a thing by its key, not its label.** Only the shapes' refusal reads a
  thing's `rdfs:label`; the rest use its key (`the lamp`), which is the same word in this realm but
  need not be in another.
- **A signature's verdict is not shown to a player.** A tampered verb and one signed by someone
  else get the same sentence; which key or hash failed is in the verdict, not the refusal.
- **A room's things are listed in no promised order.** A room holding two or more things lists
  them on one line in whatever order the query answers, which can differ from one realm to
  another; the stored query does not sort them.
- **While a take or a drop is in flight, the thing is briefly listed in both places.** A look
  between the two writes sees it in the room and with the taker. The sentence a player reads after
  a lost race or a failed move is tested; what someone else sees during that moment is not.
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
| 2026-11-04 | The first door, the `ikigai` REPL (`resmud::door`): its words, `help` and `what` for what you can do here, a refusal shown as `resmud::refusal` and never the error, and only plain text printed. |
| 2026-11-07 | A word that names no verb is refused as such (`There is no verb called dance.`), before the verb's author is read (resmud#53). |
| 2026-11-07 | Refusals at the door name its word for what you can do here (`type help to see what you can do here`), through `resmud::refusal_with_help` (resmud#54). |
| 2026-11-07 | `look` and a thing describes it, and the door's own words are read in any case (resmud#55). |
| 2026-11-07 | Where things are moved into atoms of their own (resmud#52): a lost race and a move that stopped halfway each say so in a sentence, and a thing is never lost silently. |
