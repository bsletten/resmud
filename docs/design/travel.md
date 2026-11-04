# Travel: portals between realms

**A proposal, not decided.** Proposed by Matt Stine, on Brian's travel notes as hub summarized
them in `talk-matt` (2026-11-04). Everything here is open to being wrong.

The second build of ResMUD: **three realms, two portals, and one traveler who crosses with what
she carries.** It proves five things and nothing else:

1. **Mechanics are the hub, theme is a face.** A thing that crosses keeps its mechanics
   byte-for-byte; only how it is presented changes.
2. **A realm's shapes are its customs.** The border is a reasoning step, a shapes check and a face,
   with four outcomes: admitted as is, admitted with a new face, left behind, or traveler refused.
3. **Inventory is a resolution, not a container.** Nothing moves when you cross. What you carry is
   asked again in the realm you are in, and a thing that does not resolve there is simply not
   there. Walk back and it is.
4. **A portal is a named resource with an author.** It names the realms it joins and can carry
   its own correspondence: how a thing from one side should look on the other. It can rename;
   it can never reclassify.
5. **Translation never stacks.** A face is always computed from the hub. A thing that crosses
   two borders is not a translation of a translation.

Out of scope for this build: instances, as-of resolution, promotion, verbs that cross (a signed
verb from one realm run in another), attestations and reputation, the OWL slice beyond what is
named below, and any way to legislate (open a border, change a realm's shapes) from inside the
game. Each is its own thread.

## The play, in one walk

Three realms, each a directory under `realm/`:

| realm | what it is | its customs |
|---|---|---|
| `forge` | a machine shop on a station | admits anything with a description |
| `grove` | an old wood where light is a craft | admits weapons, presents its own face for them |
| `first` | the existing kitchen and garden, made violence-free | refuses `rm:Weapon`; its action list has no attack |

Alice starts in the forge, carrying the lamp and a `forge:EnergyLance` (a laser blaster:
`rm:damage 4`, `rm:range 30`). The forge has a portal to the grove, and the grove has a portal
to the garden.

- **forge → grove.** The blaster crosses **with a new face**: in the grove it is a wand of
  searing light, `rm:damage 4`, `rm:range 30`. The lamp crosses **as is**.
- **grove → garden.** The garden refuses `rm:Weapon`. The blaster is **left behind**: `go` says
  so, in plain words, and succeeds. Alice carries the lamp; `look` and her inventory say
  nothing of a blaster or a wand.
- **garden → grove.** The blaster resolves again, as the wand. No escrow, no write.
- **A war golem tries the garden.** Its own atom says `rm:damage 6`. `rm:damage` has domain
  `rm:Weapon`, so the golem *is* a weapon by entailment, whether its author said so or not.
  **Traveler refused.**

## Resource map

Names follow `smallest-playable.md`: singular, the type first, partitioned by a named key.

| set | name | kind | how |
|---|---|---|---|
| realm | `urn:resmud:realm:id:{realm}` | **atom** (`realm/{realm}/realm.ttl`), sealed | as today, plus `rml:accepts` naming each portal the realm accepts. Opening a border is legislation, on both sides |
| realm shapes | `urn:resmud:realm:id:{realm}:shape` | **atom**, sealed | as today, plus the realm's customs: shapes over **core** classes only (`sh:targetClass rm:Weapon`, `sh:maxCount 0` or a `sh:not`), so a border never needs a realm's dialect |
| realm face | `urn:resmud:realm:id:{realm}:face` | **atom** (`realm/{realm}/face.ttl`), sealed | the realm's default presentation, per core class: `rm:RangedWeapon` shows here as `grove:Wand`, with a label and a description template. Brian's "the destination presents your things in its own terms" |
| portal | `urn:resmud:portal:id:{portal}` | **atom** (`portal/{portal}.ttl`) | `rml:joins` two exits, one in each realm (each an ordinary `rms:Exit` whose `rms:leadsTo` names a room in the other realm), `rml:author`, and its correspondence (below) |
| correspondence | inside the portal's atom | data | entries `source class → destination class, label, description`, e.g. `forge:EnergyLance → grove:WandOfSearingLight`. Each destination class must already exist in the destination's vocabulary: a portal names the destination's canon, it never adds to it |
| home realm | `urn:resmud:thing:id:{thing}:realm` | composition | the realm whose directory holds the thing's atom. Answered from `world.txt`, which becomes one list per realm |
| closure | `urn:resmud:thing:id:{thing}:core` | composition | the stored `core` CONSTRUCT: the thing's atom plus its RDFS closure (rdfs2 domain, rdfs3 range, rdfs9 subclass, rdfs7 subproperty) over core and the thing's home vocabulary, **projected to core terms**. This is the hub |
| customs | `urn:resmud:realm:id:{realm}:customs:thing:id:{thing}` | composition | the realm's shapes run over the thing's closure alone (the confined corridor the verb runner already uses). Answers admitted or refused, with the shape's message |
| face | `urn:resmud:realm:id:{realm}:face:thing:id:{thing}` | **transreption** | the thing's closure presented in the realm: the joining portal's correspondence if it names the thing's class, else the realm's default face for its core class, else the thing as it is. Golden-threaded from the thing's atom, the portal and the realm face |
| inventory | `urn:resmud:character:id:{character}:inventory` | composition | changes: what the character carries, each through **its own realm's** customs and then its face. A refused thing does not appear. Nothing is written |
| room text | `urn:resmud:view:room:id:{room}` | composition | changes the same way: every thing in the room is shown by its face in the room's realm |
| go | `urn:resmud:verb:name:go` | **Sink edge** (code) | gains the border: when an exit's `rms:leadsTo` is a room in another realm, it checks the joining portal is accepted by both realms, runs the traveler's own customs, and then writes **one** atom, the character's, as today. Its answer names what stayed behind |

Seven names change or are new rules over names (closure, customs, face, home realm, inventory,
room text, and the per-realm world list). Two are new data (realm face, portal). One edge changes
(`go`). No new edge.

### Rename, never reclassify

The one rule that keeps a portal honest, checked when its atom is written (a stored ASK, the way
`validate.rq` checks the shapes today):

> A correspondence entry is refused unless the destination class and the source class have **the
> same closure in core terms**.

So a portal may present a `forge:EnergyLance` (closure: `rm:RangedWeapon`, `rm:Weapon`,
`rm:Item`, `rm:Thing`) as `grove:WandOfSearingLight` with the same closure, and may not present
it as `grove:Toy` (`rm:Item`, `rm:Thing`). A portal's author can make a thing beautiful on the
other side; they cannot make it harmless, and they cannot make it dangerous.

Customs does not depend on this rule to be safe: it always reads the hub (the thing's own closure),
never the face. The rule exists so a portal cannot *lie*: no player ever reads a face that
misstates what a thing does.

### Why faces never stack

A face is a function of `(thing's closure, realm, joining portal)`. It never takes another face as
input. Alice's wand in the grove is the forge's blaster presented in the
grove; if she walked on into a fourth realm it would be the forge's blaster presented there, not
the grove's wand presented there. Every border reasons to core and only to core, which is what
makes core the interlingua. The telephone game is structurally impossible.

When no portal joins a thing's home realm to the realm it is being shown in (it crossed two
borders), the realm's default face applies. A portal's correspondence only speaks for the pair
of realms it joins.

### Who writes what

| act | who | why |
|---|---|---|
| a realm accepting a portal (`rml:accepts`) | the realm's legislators (today, the operator) | opening a border changes what may exist in a realm |
| a realm's customs and default face | the realm's legislators | canon |
| a portal's atom, including its correspondence | its author, holding `urn:cap:resmud:write:portal` | play: an author may write a portal nobody accepts yet, and it does nothing until both realms accept it |
| a thing's atom | as today | crossing never writes it |

A portal is therefore an offer: anyone with the grant can propose a passage and how things
should look through it, and the realms on each side decide whether to accept it. This answers
hub's open question ("a portal as a named resource with its own author is not in the notes")
without taking presentation away from the destination: the destination's default face is
always there, and a portal can only be more specific within the destination's own vocabulary.

### Code

- **`go`** gains the border, and the border is the only new code. It decides, and can refuse, so
  it is an edge, as today. Everything it reads is a name: the portal, both realms' acceptance,
  the traveler's customs. It still writes one atom.
- **Nothing else is code.** Closure is a stored CONSTRUCT (RDFS's four rules are one property path
  each), customs is the confined shapes run the verb runner already does, and the face is a
  transreption over stored data.

## Economics

- **Cacheable:** closure, customs, and face, each keyed by the atoms it read. A thing's closure
  recomputes only when its own atom or its vocabulary changes. Its face in a realm recomputes when
  the thing, the portal or the realm face changes.
- **What a crossing recomputes:** the traveler's place, and so their look and inventory, which now
  resolve in another realm. **Nothing about the things they carry recomputes**: their closures,
  customs answers and faces in every realm were already cached or are computed lazily on first
  look. A crossing is as cheap as any `go`.
- **What a portal edit recomputes:** the faces of things from its source realm, in its destination
  realm, and nothing else.
- The open caching cost in `smallest-playable.md` (contents over the whole world) is unchanged by
  this build and not addressed here.

## Refusals

| case | error |
|---|---|
| `go` through an exit whose portal either realm does not accept | **Conflict**: the state says no |
| a traveler whose own closure the destination's customs refuse | **Conflict**, with the shape's message: "you cannot go west: the garden admits no weapons, and you are one" |
| a carried thing the destination refuses | **not a refusal.** `go` succeeds, and says "the blaster stays behind in the grove" |
| a portal written with a correspondence that reclassifies | **InvalidArgument**, naming the entry |
| a correspondence naming a class the destination does not define | **InvalidArgument** |
| a portal written without `urn:cap:resmud:write:portal` | **Denied** |
| writing `rml:accepts`, a realm's customs or its default face from play | **Denied**: sealed names, as today |
| an attack-class action in a realm whose action list omits it | not a refusal: there is no such action in the affordance list, so there is nothing to refuse. The selection funnel already does this |

## Done means

One test walks it through on the text face:

1. In the forge, Alice's inventory lists the lamp and the laser blaster.
2. `go` into the grove. Her inventory lists the lamp and the wand of searing light. The blaster's
   atom is byte-for-byte unchanged, and the wand's closure in core terms equals the blaster's.
3. `go` into the garden. The answer says the blaster stays behind. Her inventory lists the lamp
   only. The affordance list has no attack.
4. `go` back into the grove. The wand is in her inventory again. Nothing was written but Alice's
   atom, three times.
5. A war golem with `rm:damage 6` and no declared class is refused at the garden, by entailment.
6. A portal whose correspondence presents the blaster as a toy is refused when it is written.
7. A thing with no portal between its home and the realm it is shown in gets that realm's default
   face, computed from the hub, never from another face.

## Open: decisions for Brian

1. **Where a traveler's atom lives.** Today Alice's atom is in `realm/first/character/`. A
   traveler belongs to the realm they are in, the realm they came from, or none. My lean: a
   character's atom lives outside every realm (`character/` beside `realm/`), because a traveler is
   the one thing whose home is the crossing. Its things keep their home realm. Your notes point
   the same way: exit ("fork the realm and leave with your layer and characters") is always
   available. If a character belonged to a realm, leaving would need the realm to let it go; if it
   belongs to none, exit is only a change of place, and no realm can hold a character hostage.
2. **Traveler refused: Conflict or Denied.** Proposed Conflict, since the state (shapes) says no,
   not the caller's capability. Denied would read as "you lack a grant", which is wrong for a golem
   that simply is a weapon.
3. **The reasoner.** Proposed: a stored CONSTRUCT with RDFS's four rules, run in the confined
   corridor. The thin OWL slice from your notes is out of scope here; which OWL constructs, and
   whether that becomes a crate, is its own thread.
4. **More than one portal between two realms.** Two portals joining the same pair, with different
   correspondences, would give a thing two faces. Proposed for this build: a realm accepts at most
   one portal per neighbor. Later, the face could follow the portal you came through, which makes
   the face depend on the traveler's history and is worth its own design.
5. **Portals as play.** Proposed: writing a portal is play (a grant), accepting it is legislation.
   If you would rather portals be legislation from the start, the correspondence moves into the
   realm face and the portal becomes only an accepted pair of exits. With your economics notes,
   the split maps onto the two tracks: writing a portal is building, so it costs spendable IP;
   accepting one changes what may exist in a realm, so it takes promotion authority, which only
   standing mints. A realm's politics then decides who accepts: one issuer in a monarchy, a vote
   in a democracy.
6. **The realm that can never open a border.** An anarchist collective has no promotion capability
   at all, so under (5) nobody there can accept a portal, and no border into it ever opens unless
   the realm was founded with one. Is that a feature (a sealed commons, chosen at founding), or does
   founding need a way to name the realm's first portals?
7. **Who accepts a realm's first portal.** Before anyone has standing in a new realm, someone has
   to accept its first border. That is one concrete instance of genesis, your open problem of who
   mints the first grant without a root account.
