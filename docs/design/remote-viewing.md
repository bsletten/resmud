# Remote viewing: a space over what a character sees

**A proposal, not decided.** Written for Brian's review on resmud#59 (2026-11-09), from Brian's note
on resmud#56: remote viewing through magic eyes, cameras and binoculars "may be a separate space to
resolve limitations of the default realm". No code is proposed here until it is reviewed.

resmud#56 made what a character can see one resource, `urn:resmud:character:id:{c}:sees`, and said
that is the seam. This note designs what goes through the seam. It proves four things and nothing
else:

1. **Sight is a space, not a change to the rule.** A device switches on a corridor for one
   character, and the corridor rebinds one name. `sees` itself, and everything #56 pinned, stays
   as it is.
2. **Holding the device is the authority.** What you can see through it is asked again on every
   read, so a device that leaves your hands stops showing you anything, with nothing to revoke.
3. **A device reads one room and writes nothing.** `take` and `drop` never read `sees`, so seeing
   a thing through a camera gives no reach to it.
4. **A realm without devices is the realm of #56, byte for byte.**

Out of scope: hearing (nothing in the game speaks yet), spells a player publishes, and any pushed
or live feed. Each is named below where it would attach.

## What the default realm cannot do

Four limits of what is built set the shape of this design. Each was run against the kernel or read
in its source; see [What was tested](#what-was-tested-for-this-note).

- **`sees` is anchored on the looker's place.** The stored `sees` query starts from
  `?room rms:contains ?subject`, so adding another room's contents atom to `…:sees:graph` changes
  nothing. A remote room has to be asked about by its own query, and joined on.
- **A rebinding cannot read its own name.** A corridor that maps `…:sees` to something that reads
  `…:sees` loops until the kernel stops it (`DepthExceeded`). So the device layer re-asks the same
  stored query over the same `…:sees:graph` under a second name, `…:sees:here`.
- **A kernel capability cannot be narrowed to one room.** `Capability::attenuate` keeps the
  intersection of exact tokens, and a player holds one read token for the realm's whole directory
  (`Realm::fs_scope("read", "")`). Asking for one file of it keeps nothing. Nor can a capability
  be widened, and a door seals its session when it opens, so a device picked up mid-session cannot
  add a grant to that session. "The device as a capability, attenuated to one room" therefore
  cannot mean a kernel `Capability` on ikigai-core 0.3; see
  [How it is authorized](#how-it-is-authorized).
- **Sight is a rule of the game, not a boundary of authority.** A player's door still answers
  `source urn:resmud:view:room:id:garden` from the kitchen, and a verb a player wrote can rewrite a
  thing in another room without seeing it. ACCESSIBILITY.md lists the first as a known limitation
  of the REPL's grammar at the door. Neither is this design's to fix, and both bound what it can
  promise (decision 1).

## The play

The sample realm gains a cloudy crystal, the receiver of a camera fixed on the garden. Alice is in
the kitchen, and the crystal is in her hands.

- `look rock` says `You see no rock here.`, as it always did.
- `watch crystal` says `Through the crystal you see the garden.`, then the garden's room text.
- `look rock` now describes the rock. `look lamp` still describes the lamp: a device adds to what
  you see, it does not take you away.
- `look` alone still says the kitchen and what she carries. A device does not move her.
- `take rock` is refused: `The rock is not here: look to see what is in the kitchen.`
- She drops the crystal. `look rock` is `You see no rock here.` again, and `watch` says the crystal
  shows nothing now. Whoever picks it up holds that view, and no other.
- `unwatch` ends it. Her corridor is back to the personal one.

## Resource map

Names follow `smallest-playable.md`. The terms stay under `rml:` (`urn:resmud:term:name:`) until a
`resmud-vocab` change gives them a home.

| set | name | kind | how |
|---|---|---|---|
| device | `urn:resmud:device:id:{device}` | **atom** (`device/{device}.ttl`), sealed | `rml:Device`, `rml:held` (the thing that must be carried), `rml:reach` (`rml:room`, `rml:adjacent` or `rml:placed`), `rml:watches` a room when its reach is `rml:room`, and `rml:conveys rml:sight`. A device's key is the key of the thing it is held by |
| aim | `urn:resmud:device:id:{device}:aim` | **atom** (`aim/{device}.ttl`), read by code | where a placed device was put: `<device> rml:watches <room>`, written only by `place`. A file that is not there reads as empty, as `…:carries` does |
| gaze | `urn:resmud:character:id:{c}:gaze` | **atom** (`gaze/{c}.ttl`), read by code | what this character has switched on: `rml:through <device>`, `rml:watches <room>`, and `rml:from <room>` for an adjacent device. Written only by `watch` and `unwatch`. Missing reads as empty: not watching |
| gaze parts | `…:gaze:device`, `…:gaze:target` | composition | stored queries over the gaze atom alone, each one key or nothing, as `…:place` is |
| sees, here | `urn:resmud:character:id:{c}:sees:here` | composition | the stored `sees` query over `…:sees:graph`, unchanged, at a second name (see above) |
| sight graphs | `urn:resmud:device:id:{d}:sight:character:id:{c}:graph` | composition | a template: the gaze, carries and own atoms of the character, the device and aim atoms, the target's contents atom, and every character the world lists |
| device sight | `urn:resmud:device:id:{d}:sight:character:id:{c}` | composition | the stored `sight` query over those graphs: the target room, what it contains and who is there, one name per line, **only if** the join below holds. Otherwise nothing |
| device sees | `urn:resmud:device:id:{d}:sees:character:id:{c}` | composition | a template: `…:sees:here`, then the device sight |
| what you are watching | `urn:resmud:character:id:{c}:watch` | **refusing view** (code) | `Through the crystal you see the garden.` and the room text, or a refusal; see [Views](#views) |
| watch | `urn:resmud:watch` | **alias rule** in the personal corridor | → `urn:resmud:character:id:{c}:watch`, as `urn:resmud:look` does |
| watch, unwatch | `urn:resmud:verb:name:{watch\|unwatch}` | **Sink edge** (code) | each decides and writes **one** atom, the gaze |
| place | `urn:resmud:verb:name:place` | **Sink edge** (code) | writes **one** atom, the aim, for a device whose reach is `rml:placed` |

`device`, `aim` and `gaze` are not in `WRITABLE_KINDS`, so the runner already refuses a plan that
writes one. `aim` and `gaze` are bound the way the atoms of where things are are bound, before the
resource map. The compositions join the resource map only for a realm with a `device/` directory.

### The three kinds of device

| | what you hold | `rml:reach` | what it shows |
|---|---|---|---|
| a camera on a room | its receiver, a thing | `rml:room`, with `rml:watches` in the sealed atom | that room, wherever you are |
| binoculars | the binoculars | `rml:adjacent` | the room an exit leads to, from the room you stood in when you raised them. Walk away and they show nothing (`rml:from` no longer matches your place) |
| a magic eye | a stone paired with it | `rml:placed`, with `rml:watches` in the aim atom | the room the eye was placed in |

The thing you hold is an ordinary thing, with a description, in a contents or carries atom. The eye
and the camera themselves are not things. They have no body in the contents atom, so nobody can
take them (decision 3). What can be stolen is what you hold, which is what the brief asks a theft to
be.

### Spaces

- **The device corridor** is `urn:resmud:corridor:character:id:{c}:device:id:{d}`, one rule:
  `urn:resmud:character:id:{c}:sees` → `urn:resmud:device:id:{d}:sees:character:id:{c}`. The name
  is the corridor's cache identity, and the kernel asks that two corridors named alike bind the same
  doors, so it is partitioned by both keys and holds nothing else. It rebinds no other name:
  not `…:place`, not `…:carries`.
- **Switching it on.** The corridor is injected only while the gaze names a device. A door reads
  `…:gaze:device` (one cached read) before each turn and passes the personal corridor, with or
  without the device corridor added. `Door::with` today holds one `Scope` for the session; it would
  hold a way to get the turn's. A host that never asks has the realm of #56.
- **Why not rebind for good.** A composition in the realm could union a gaze into `sees`, and
  needs no door change. But that changes the rule #56 pinned, for every realm, which the brief rules
  out.

### How it is authorized

The brief's sentence is "the device as a capability the character holds, attenuated to one target
room". On ikigai-core 0.3 (above) it comes apart into three things, each enforced where it can be:

- **The capability is the thing in your hands.** Device sight joins, on every read: the gaze names
  this device and this room; the device says it is held by this thing; the carries atom says this
  character holds that thing; and the device's own `rml:watches`, or its aim, grants this room. If
  any half fails the sight is empty, and the kernel's cut of the carries atom makes the next read
  recompute. There is no revocation to write, and a stale gaze sees nothing.
- **One room, by naming.** Device sight is run over a list of graphs that names one room's contents
  atom, the fence `…:sees:graph` already is. The device's atoms are sealed, so no verb can move the
  fence.
- **Nothing written.** The corridor rebinds a name only `look` reads. `go`, `take`, `drop` and the
  verb runner do not read `…:sees`, so what a device shows gives them nothing to act on.

**A verb must not be able to forge the holding.** Today a plan can place a thing it did not create:
with the sample author's grant, a verb that says `<kitchen> rms:contains <crystal>` duplicates a
crystal someone else carries, and `take` gives it to the caller. Holding is only authority if that
cannot happen, so the runner gains one check: a plan may not place a thing that a device is held by
(a device atom exists at that key), and is refused as Denied before anything is written. In a realm
without devices the check finds nothing.

So a stolen receiver is the receiver's one room, for whoever holds it now, and nothing more.
What this does not stop is a player typing `source urn:resmud:view:room:id:garden` at the door. That
reach exists without any device; see decision 1.

### What is seen, and what is not

Look only. `rml:conveys` says what a device carries, and this build honors one value, `rml:sight`.
Hearing would be the same technique over a new name, `…:hears`, when something in the game speaks:
today nothing does, so there is nothing to convey. Sight is pull: the view changes when the player
asks again, as `look` does.

### Views

`text/plain` is the only face. `urn:resmud:character:id:{c}:watch` is code for the reason
`urn:resmud:seen` is: it says *no*. It answers one framing sentence and then
`urn:resmud:view:room:id:{room}` unchanged, so a room reads the same through a device as in it.

| a player types | the door issues |
|---|---|
| `watch crystal`, `watch binoculars east` | `sink urn:resmud:verb:name:watch …`, which answers the view |
| `watch` | `source urn:resmud:watch` |
| `unwatch` | `sink urn:resmud:verb:name:unwatch` |
| `place stone` | `sink urn:resmud:verb:name:place stone` |

These words exist at a door only for a realm with devices, so elsewhere `watch` is still looked up
as a verb a player wrote and refused as `There is no verb called watch.` Each is listed in the
realm's `rml:allows`, so it appears in `help` as a line.

**For ACCESSIBILITY.md, at build time:** the framing sentence names the device and the room first,
so a screen-reader user is never left to infer from room text that it is not where they stand.
`look` alone stays where you are. `You see no rock here.` is unchanged with a device on, byte for
byte, so a device cannot be probed for what it cannot show. Whether something is on is always a
question you can ask, `watch`, and is never silent state: a view that has gone dark says so in a
sentence. Nothing is pushed, so WCAG 2.2.2 is not reached; a live feed would need a pause.

### Code

- **`watch`, `unwatch`, `place`** (edges): each changes state and can refuse.
- **`…:watch`** (refusing view): it says *no*, as `urn:resmud:seen` does.
- **The runner's check**: one more refusal, in the code that already decides what a plan may write.
- **`aim` and `gaze`**: the `holding` endpoint, which already reads a missing file as empty.
- **The door's per-turn corridor**: a host's choice of a `Scope`, which no name can make.

Everything else is a stored query, a template or an alias.

## Economics

- **What a gaze change recomputes:** the gaze atom's readers, one character's.
- **What a move recomputes:** a `take` or `drop` in the target room cuts its contents atom, so every
  device sight on that room recomputes; a key changing hands cuts that character's carries atom,
  and so its sight. A `go` recomputes every device sight, for the reason it recomputes every `sees`:
  who is here reads every character's atom.
- **What it costs a realm without devices:** nothing. No device name exists, and no gaze is read.
- **Not measured:** whether a composition read inside a corridor shares cache entries with the same
  one read outside. Each corridor is its own partition for the names it answers; a realm with many
  devices should be measured before this is assumed cheap.

## Refusals

Proposed text. Every refusal is a sentence in plain words, as the rules require.

| case | error |
|---|---|
| `watch` a device you do not carry | **Conflict**: "You do not have the crystal." |
| `watch` a thing that is no device | **Conflict**: "The lamp shows you nothing: it is not something to watch through." |
| `watch binoculars` with no direction | **MissingArgument**: "Say which way to watch, such as watch binoculars east." |
| `watch binoculars north` where there is no north exit | **Conflict**, the sentence `go` gives |
| `watch` with nothing on | **NotFound**: "You are not watching anything." |
| `watch` when the device has left your hands | **Conflict**: "The crystal shows you nothing now: you no longer have it." |
| `look` and a name a device does not show | **NotFound**: "You see no rock here.", unchanged |
| `take` a thing seen only through a device | **Conflict**: "The rock is not here: look to see what is in the kitchen.", unchanged |
| a verb that writes a device, an aim or a gaze | **InvalidArgument**, the runner's existing refusal |
| a verb that places a thing a device is held by | **Denied** |
| `watch` or `place` without `urn:cap:resmud:play` | **Denied**, by the declared capability floor |

## Done means

1. In a realm with no `device/`, `tests/sight.rs` passes unchanged, and `watch` is still a verb
   lookup.
2. `look rock` is refused before `watch crystal` and answered after it, with `look lamp` answered
   throughout, on the text face and at the door.
3. `look` alone is byte-for-byte the same with a device on as off.
4. `take rock` through the device is refused, and no file is written.
5. Dropping the receiver shuts the view with no write to the gaze, the corridor unchanged.
6. A second character holding the receiver sees the same room and no other.
7. Binoculars show nothing after a `go`.
8. A plan that places a device's key is refused, and nothing is written.
9. Every sentence above is pinned, and checked with `resmud::is_plain`.

## What was tested for this note

A throwaway test file ran against `main` at `ff84185` on 2026-11-09 and was not committed; it is in
the pull request's description. It showed, with the kernel and the real stored `sees` query:
extending only `…:sees:graph` changes nothing; a rebinding that reads its own name stops with
`DepthExceeded`; a fixed device built entirely from compositions and one rebinding adds `look rock`
and keeps `look lamp`; `take rock` through it is refused and writes nothing; dropping the key shuts
the view and cuts the cached `sees`; attenuating a player to one file of the realm keeps no scope; a
door reads another room's text by `source`; a verb places a thing it did not create; a verb rewrites
a thing in another room.

**Not tested:** binoculars and the aim atom, the gaze atom as the source of the target, the door's
per-turn corridor, the two-word `watch binoculars east` at the door, the compositions living in the
resource map rather than the spike's corridor, and anything's cost. No person has read any of this at a terminal or with assistive technology.

## Open: decisions for Brian

1. **Is sight a boundary?** This design makes the camera's grant hold for everything the game
   offers, and it is a game rule, as #56's is: a `source` line at the door still reads any room.
   Making it a boundary means closing that reach, and per-room read tokens minted by a door, since
   narrowing cannot name one room. My lean: build this as a game rule, and take the door's reach as
   its own item, because it is a limit of the door, not of devices, and nothing here depends on it.
2. **Spells.** A spell the brief mentions is a device with no body: authority from something other
   than a held thing. Proposed: leave it out of the first build, because a spell a player publishes
   is a verb granting sight, and that needs a grant of its own (who may write one, what it may
   reach). A realm-authored spell could be a device held by a thing like a scroll today.
3. **Do you know you are watched?** Proposed: nothing says so. An author who wants a visible camera
   writes it into the room's description. Should a placed eye be findable, by a line in the room or
   a verb that searches? It is the one place this design has a position on who may be observed.
4. **Add or replace.** Proposed: a device adds to what you see, so `look lamp` keeps working.
   Replacing (a magic eye you close your own eyes to use) is the same corridor with `…:sees:here`
   left out of the template, and could be a device's choice.
5. **Hearing.** Proposed: not until something in the game speaks; `rml:conveys` holds the place.
6. **How many at once.** Proposed: one gaze per character. Several is a list in the gaze atom and a
   corridor name that names them all, and a player reading a room list has to be told which device
   each came from.
