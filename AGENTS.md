# Working in this repository

ResMUD: shared, programmable worlds on ikigai. These rules are for every agent and every person
working here. `CLAUDE.md` is a link to this file.

- **Design names before code.** Read [docs/design/](docs/design/) first. A new feature starts as a
  resource map (sets, the atoms that hold state, compositions, corridors, views); code is for what a
  composition cannot do, and says why.
- **Names are singular, the type first, and partitioned by a named key**: `urn:resmud:room:id:{id}`,
  never `rooms` or `room/{id}`.
- **Read [ACCESSIBILITY.md](ACCESSIBILITY.md) before any change to output people read.** The text
  face is canonical: anything the game can say, it says in plain text.
- Security reports go through GitHub private vulnerability reporting, never a public issue.
- Every change lands through a pull request with green CI.
- Never invent testing, tool results or conformance evidence.
- Use American spelling in everything you write: code comments, docs, commit messages and pull
  requests.
