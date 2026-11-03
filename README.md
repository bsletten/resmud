# ResMUD

Shared, programmable worlds on [ikigai](https://github.com/ikigai-rs). A world you can program
from inside itself: make an object, write a verb on it, and everyone else's world changes live.
Building the world is the play.

ResMUD is built on a resource-oriented kernel, and most of the game is naming:

- **An object is a resource**, named by a URN (`urn:resmud:room:id:kitchen`). The world is a
  graph you can query.
- **Permissions are capabilities**, and they only ever narrow down a chain of calls. A verb a
  player writes runs under no more authority than its author holds.
- **What you can do here is the list of actions your capability allows.** That list is the
  parser: there is no hidden thing to click and no command to guess.
- **The world is live.** A room's description is derived from what is in it, so taking a thing
  changes what everyone sees, with no code that rewrites a description.
- **Text is the canonical face.** Every room answers in plain text first; any richer view is
  derived from the same graph.

The core vocabulary (`rm:` and `rms:`) is [resmud-vocab](https://github.com/bsletten/resmud-vocab),
CC0.

## Status

The first playable build, designed in
[docs/design/smallest-playable.md](docs/design/smallest-playable.md): one realm, its text face, and
one signed verb a player wrote. It runs as a library and its tests: there is no door yet, and the
`ikigai` REPL cannot host it until `ikigai-engine` moves to ikigai-core 0.3, so the tests issue the
requests the REPL would (`source urn:resmud:look`, `sink urn:resmud:verb:name:take lamp`).

- **The realm is files.** `realm/first/` holds the sample realm: the realm atom, its shapes, two
  rooms, a lamp, a character and an author, as Turtle. The tests copy it into a scratch directory.
- **Almost everything is a name.** 22 of the realm's 29 names are rules over other names (stored
  queries, a text template, `urn:kernel:actions`, the plan's transreption). Code is `go`, `take`,
  `drop`, the verb runner, one query adapter and the two endpoints that serve the stored queries and
  templates.
- **Tested:** the design's six-step "Done means" as one test, every refusal in its table, and that a
  move recomputes the room text by cutting a golden thread. See `tests/`.
- **Not built yet:** a door (telnet or the REPL), a second realm, a Turtle face for the world as one
  graph, placing a thing a verb creates, and any way to pause the live text.
## Accessibility

How this repository approaches accessibility, what it emits that people read, and how to report a
barrier: [ACCESSIBILITY.md](ACCESSIBILITY.md).

## License

Code is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
