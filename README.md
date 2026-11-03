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

Nothing playable yet. The first build is designed in
[docs/design/smallest-playable.md](docs/design/smallest-playable.md): one realm, one room, the text
face, and one signed verb a player wrote.

## Accessibility

How this repository approaches accessibility, what it emits that people read, and how to report a
barrier: [ACCESSIBILITY.md](ACCESSIBILITY.md).

## License

Code is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
