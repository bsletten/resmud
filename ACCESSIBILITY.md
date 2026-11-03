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

Today, nothing a player sees: the crate defines its namespaces only (`src/lib.rs`), and the first
playable build is a design ([docs/design/smallest-playable.md](docs/design/smallest-playable.md)).
When that build lands, it will emit:

- **The text face of every room**: its description, what it contains and its exits, as plain text.
- **The list of actions available here**, as plain text.
- **Refusals**: why an action was refused (`there is no exit north`, `the lamp is not here`), as a
  sentence.

Not this repository's: the terminal, editor or page a player shows any of this in.

## The rules that output follows

These are the design's rules; no code enforces them yet:

- Text is canonical. Every other face is derived from the same graph and may add nothing a player
  needs that the text lacks.
- Nothing can exist without a text description: the realm's shapes refuse it at creation.
- What you can do here is always an enumerable list, never something hidden to find.
- A refusal is a sentence that says what is wrong and, where it can, what to do instead.
- Nothing is written in color only.

## How it is tested

- **Nothing that people read is tested yet**, because nothing is emitted yet. The doctests in
  `src/lib.rs` check the namespace constants only.
- **No automated accessibility checker, browser, terminal screen reader or other assistive
  technology has been used.**

## Known limitations

- The whole game is unbuilt; every rule above is a design intention until a test holds it.
- Live updates (a room's text changing as its contents change) will need a way to pause them
  (WCAG 2.2.2); the design names this and nothing implements it yet.

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
