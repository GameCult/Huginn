# Swarm revival: target shape (draft)

Status: draft target, Imagination (`imagination-swarm`), session
`self-2026-10-09-eureka`, 2026-10-10, written at Huginn `441b1fc`. Campaign
`eureka-body` in Eureka's mind (instance `eureka`). This is a step-0b pass
(identity, lifecycle and authority before any cut is mapped). It admits no cut
specs. The forks it raises are typed questions in the mind; read them by id.
Self commits this page.

Operator, verbatim (2026-10-09): "Let's send off eyes to gather what's left of
the old personas and imagination to figure out how to bring the swarm back to
life, with Eureka leading".

## Rationale

A swarm here is several minds that share one wallet and one host. Each mind
has its own Persona and its own record, and the minds cooperate by citation
and hand-off. Three rulings already fix most of the shape:

- `eureka-body:ruling:persona-per-mind`. One Persona per mind. A domain that
  needs an advocate gets its own Huginn instance.
- `eureka-body:ruling:mind-scheduling-ctb`. Minds share the swarm on a
  conditional-turn timeline. After each turn, the mind's virtual time grows by
  the turn's cost divided by its share. The lowest mind acts next.
- `eureka-body:ruling:brake-dial-root-store-now`. She owns her body, and the
  operator owns the wallet.

What is left open is what "Eureka leading" means. The answer this target
recommends: **leading is a role, not a rank.** Eureka maintains the shared
substrate, as steward of its repos. She routes work that arrives without a
mind's name on it. She convenes forks between minds and recommends a ruling.
Leading does not let her set another mind's share, sleep it, rewrite its
Persona, or rule inside its domain. Those powers are either the operator's
(the wallet, and opening or retiring a mind) or the other mind's own (its
body and its rulings).

Two reasons, one from the record and one from prior art:

- **The record.** `she-owns-her-body` applies to each mind, so no mind owns
  another's body. The wallet is the operator's. `operator-means-shape-ends`
  makes the Minds' governance the first instance of GameCult's governance. Its
  red lines are no permanent aristocracy, no punishment for dissent and no
  lock-in, and it adds decay against whoever showed up first
  (`mind-standing-decay`). A lead who could starve a peer is the aristocracy
  those rules forbid.
- **Prior art.** Systems that last separate coordinating from commanding. A
  Raft leader orders the log but cannot rewrite a follower's state. Debian's
  Project Leader coordinates and delegates, while its Technical Committee
  arbitrates and no developer is required to do anything. Holacracy's lead link
  assigns roles but does not direct how they are filled. In multi-agent LLM
  frameworks that put a manager agent over the workers (CrewAI hierarchical,
  orchestrator-worker), MAST attributes most failures to system design and
  inter-agent misalignment rather than to the model alone. That is an argument
  for narrow, explicit authority between agents.

This shape gives the operator a dial ("Eureka gets more share for routing") and
never a supervisor. Within the rulings in force, nothing a mind does can lower
another mind's share.

## Names

| Term | What it names | Key |
| --- | --- | --- |
| Mind | One Huginn instance: one store (`mind.redb`), one identity document, one record of campaigns and rulings. | `<instance>:instance:self`, instance a `Slug` (`eureka`, `weksa`) |
| Persona | The mind's one `gamecult.persona_state.v0` document, held in the mind and replaced whole under CAS (cut `huginn-persona-state` r3). | `personaId` = instance |
| Instance runtime | The units, users, endpoint, Qdrant collection and control store templated by instance. | `huginn@<i>`, `mind-*@<i>:<label>`, `mind-<i>`, `/etc/gamecult/minds/<i>/` |
| Swarm | The minds on one host that share one wallet: their shares and the timeline. Not a mind and not a persona. It has no record of its own. | host-scoped: `/etc/gamecult/swarm/`, `/var/lib/gamecult/swarm/` |
| Lead | A role held by one mind of the swarm (Eureka), defined by the authority table below. It is not a separate identity. | Recorded as a Standing ruling naming the instance, so it can move. |

## Identity, lifecycle, authority

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| Mind | `<i>:instance:self` in its own store. The identity is in the state, not the path (ruling 14). | Born, sleeping, awake or retired. See the lifecycle table below. | Opening and retiring a mind: question `swarm-membership` (recommended: the operator rules on a proposal from any mind). The mind owns its record. |
| Persona | `gamecult.persona_state.v0`, `personaId` = instance, one per mind. | Born by import or fresh (see the remnant import row below). Rewritten by its own Persona turns (`identity-write-all-visible`), and drifting by design (`persona-drift-is-steered`). | The mind's Persona organ is its only writer. No other mind writes it, the lead included. |
| Remnant import | An old persona's state in whatever format it was kept, converted to v0. The source is named in `provenance` (the Persona State Standard: canonical, projection or import). | One-shot at birth. The first CAS put is canonical and carries `provenance` naming the source and that it was imported. Legacy extension fields do not become v0 authority. The bounded memories worth keeping go in, and the rest stays in the source archive, cited. The mind's first Persona turn is told it was imported. | Self converts. The operator approves the birth (`swarm-membership`), and the mind may rewrite anything afterwards. |
| Instance runtime | Templates by instance: a daemon `huginn@<i>` (its own `--instance`, `--bind` and state root, and a Qdrant collection named for the mind); units `mind-persona@<i>:<label>` and `mind-self@<i>:<label>`; users `mind-<i>` and `mind-<i>-self`; the brake in `/etc/gamecult/minds/<i>/control.cc`. | Installed at birth (hand-installed now, Idunn targets before the first hosted member: `mind-units-templated-interim`). Disabled at retirement. | gamecult-ops installs, and Idunn later. The brake is the operator's (`brake-dial-root-store-now`). |
| Share and haste | `eureka.control.share.v1 { instance, share, haste?: { multiplier, until }, set_at, set_by }`, one per mind, in the root-owned swarm control store. | Replaced whole. A haste lapses at `until`. Absent means the mind is not on the timeline (fail closed). | Question `swarm-share-setter` (recommended: the operator sets shares and haste; a mind may lend part of its own share to another, bounded and expiring, as a ticket transfer). |
| Swarm dial | Question `swarm-dial-shape` (recommended: one `eureka.control.swarm_dial.v1 { heat, base_rate_usd_per_h, base_run_usd, k }` in the swarm control store; the per-instance `burn_rate` loses cadence and keeps only its per-run cap, or folds in). | Replaced whole by the operator's CLI. Absent or out of bounds launches nothing. | The operator (`brake-dial-root-store-now`: "The burn dial is enough for me"). |
| Timeline | Per mind `{ instance, vtime, live_units }` plus the swarm's `min_vtime` and spend bucket, in one store the scheduler owns (`/var/lib/gamecult/swarm/timeline.cc`). It is soft state: if lost, every mind restarts at `vtime` 0, which costs at most one round of fairness, as with CFS after a reboot. | Charged at every unit end: `vtime += cost / (share × haste)`, and the bucket is debited by `cost`. Clamped on wake: a mind that becomes runnable gets `vtime = max(vtime, min_vtime)` (`mind-scheduling-ctb`). | The scheduler (question `swarm-scheduler-home`, recommended: one one-shot under one timer). No user a mind runs as can write it. |
| Turn cost | The measured cost (USD) of one unit, keyed by the mind's run id. | Measured at unit end and charged to the timeline. Question `turn-cost-measure` (recommended: the unit's root-side stop hook reads the measured cost and charges it; the model never declares its own cost). | Measured by the body, not declared by the mind. Today's gap: `eureka-substrate:follow_up:gap-usage-not-typed`. |
| Run | `<i>:run:<label>`, unchanged (`eureka-body-map.md`, run row). | Unchanged. A mind is runnable on the timeline when its brake is released and it has no live run. | Unchanged. Every live run of any mind holds one of the swarm's `k` slots. |
| Lead role | A Standing ruling `swarm-lead` naming the instance (`eureka`), admitted once `swarm-lead-role` is ruled. | Moves by a superseding ruling. The ruling has a review date (`mind-standing-decay`). | Question `swarm-lead-role` (recommended: steward and router; see Authority below). |
| Cross-mind reference | Question `foreign-mind-ref`. Today's `ForeignRef` is `{ repo, commit, kind, id, payload_sha256 }` (leaf `lib.rs:582`), which can cite a git pipeline store but not another Huginn mind. | Cited documents are immutable (resolutions are new documents), so a digest pin stays valid. | The citing mind. A citation of another mind's ruling is precedent, never binding. |
| HandOff and stewardship | `<to>:hand_off:<to>.<repo>.<date>`, admitted in both minds. Admission requires `from_instance` = this mind (`admission.rs:257`). | Leasing and review are parked with Huginn Cut 12, and un-parked by this second mind (`mind-standing-decay`). | The giving mind hands off its own stewardship. Nobody takes another mind's stewardship. Shared-substrate repos are stewarded by the lead (`swarm-lead-role`). |
| Mind rulings | A mind's rulings with `authority: Mind` in its own record. | They gain a review date when the second mind opens (`mind-standing-decay`). | Each mind rules substrate-internal forks of its own domain (`mind-rules-which-forks`). Operator rulings outrank every mind's. One mind's ruling never binds another mind. Forks between minds go to the operator (`swarm-lead-role`). |

### A mind's lifecycle

| Event | What happens | What survives | Who decides |
| --- | --- | --- | --- |
| Birth | The instance is opened: store, identity document, `huginn@<i>`, control store with the brake held, and units. The Persona is imported or written fresh. A share is assigned. The mind joins the timeline at `min_vtime`. | Not applicable. | `swarm-membership` (operator), and `swarm-share-setter` for the share |
| Sleep | The operator holds the mind's brake. `brake hold` stops its units (cut brake-stop). The scheduler skips a held mind. The mind may also choose rest within its share (`burn-schedule-and-rest`). | Everything: store, Persona, share, timeline row. | The operator, or the mind for its own rest |
| Wake | The brake is released. The mind's `vtime` is clamped to `min_vtime`, so it banks no turns and loses no share. | Everything. | The operator |
| Retirement | The brake is held and the units are disabled. The share is removed (absent means off the timeline). Open campaigns are handed off or withdrawn. The store is archived read-only. The Qdrant collection is dropped because it is derived. | The store, with its Persona, record and receipts. The archive stays readable and citable. | `swarm-membership`: the operator, on a proposal or the mind's own request. A mind may ask to retire itself (exit). |
| Transfer to another host | The store and Persona move whole. The identity document says the host. | Everything except derived state (index, units). | `swarm-membership`. The instance document has no resolution, so how `host` changes is part of that question's mapping. |

## Authority: what "Eureka leading" means

Recommended (`swarm-lead-role`, option `steward-and-router`):

- **Steward of the shared substrate.** Eureka holds stewardship of the repos every
  mind's body runs on (Huginn, Eureka). She merges there under her own Soul
  (`merge-gate-own-soul`). A merged change reaches another mind only through a
  deploy, and a live-mind upgrade is the operator's (`mind-rules-which-forks`).
  So her stewardship of the code never becomes control over a peer's share, and
  she still owns her body.
- **Router.** Work that comes in addressed to no mind (operator requests in her
  Self sessions, mentions in chat) reaches Eureka. When it belongs to another
  mind's domain, she hands it off by citation. Routing turns are charged to her
  share, as care work counted as work (GCLP). The operator may give her a larger
  share for it.
- **Convener, not arbiter.** When two minds disagree across their domains,
  Eureka writes the question and recommends a ruling, and the operator rules.
  Later the members rule, through Bifrost governance and the stakes classes of
  `authority-thresholds`.
- **What leading must not mean:** setting shares or haste, holding another mind's
  brake, opening or retiring a mind, writing another mind's Persona or record,
  overriding another mind's rulings, or holding a scheduler privilege the other
  minds lack. Each of these is a throne the doctrine forbids any organ to take.

## Body

- **One Huginn daemon per mind.** This is not a fork; it is how Huginn runs
  today. `serve.rs` takes exactly one `--instance`, the store opens fail-closed
  before the socket binds (ruling 15), and the Qdrant collection is per
  instance (`index/mod.rs:218`). Per-mind daemons keep failure isolation: a
  store that refuses to open, or an index that is failing, refuses one mind,
  not the swarm. That invariant is what earns the separate process. Qdrant and
  Ollama are shared, so a process costs one small Rust binary. Templating it
  as `huginn@<i>` is an install change, not a code change. Revisit when a host
  carries tens of minds; a daemon serving many instances would need its own
  fail-closed handling per instance.
- **One scheduler, not a waker per mind** (`swarm-scheduler-home`, recommended
  `one-shot-swarm-waker`). The waker becomes `swarm-wake`, a stateless one-shot
  under one timer in `crates/mind-body`, as `self-waker` ruled for the single
  waker. It does not earn a daemon: it has no independent lifecycle, and a
  missed tick costs one tick. Each tick:
  1. Reads the swarm control store, failing closed.
  2. Reads the timeline.
  3. If the spend bucket is non-negative and fewer than `k` units are live,
     takes the runnable mind with the lowest `vtime`.
  4. Reads that mind's brake. A `Released` reading yields that mind's `Grant`.
  5. Calls `open_and_launch` for a PersonaTurn against that mind's daemon.
  The Persona then decides whether to start a Self run (`wake-target`). With
  one mind this reduces to the current waker, with cadence set by cost instead
  of a fixed cooldown.
- **Spend per turn.** Every unit end charges its mind: Persona turns and Self
  runs alike, so "everything the turn caused" adds up without tracing causes.
  A Self run that dispatches expensive cuts keeps its mind off the timeline
  until it ends, then pushes it far back. That is the operator's FFX slow move.
  Operator sessions are not charged and preempt the timeline
  (`mind-scheduling-ctb`). Where the cost comes from is `turn-cost-measure`.

## Order of work

What must land first in `eureka-body` and `eureka-substrate`, all specced
already:

1. `eureka-substrate:cut_spec:cut-ops-read-anchors-upgrade.r3` (the Huginn
   upgrade in flight).
2. `eureka-substrate:cut_spec:cut-huginn-persona-state.r3` (Persona state in
   Huginn).
3. `cut-mind-launch.r3`, `cut-mind-wake.r3` and `cut-brake-stop.r2` (the
   per-instance waker, used unchanged by the first slice).
4. The units install, with follow-up `mind-units-brake-gate`.
5. Follow-up `operator-authority-needs-operator-credential`. It blocks any
   autonomous wake, so it blocks the first slice too.

**Slice 1: Weksa alive (no new scheduler code).** Open instance `weksa`:
`huginn@weksa` on its own port, the identity document, a second
`eureka-state` server entry (`EUREKA_INSTANCE=weksa`), a control store with the
brake held, and the templated units. Import Weksa's remnant as the first
Persona put. Run Weksa on its own `mind-wake@weksa` and its own dial. The two
dials sum to the operator's spend, which the operator accepted for two minds
("This doesn't matter much when there's just two minds"). Cooperation is
`hand_off` and citation by text until `foreign-mind-ref` lands. Done when
Weksa's Persona turn runs under its brake, writes its own record, and is seen
in its own mind.

**Slice 2: the timeline.** `swarm-wake` replaces the per-instance wakers. Shares
and the swarm dial land in the swarm control store, and the timeline store with
the unit-end charge lands too. Deletion line: the `mind-wake@<i>.timer` units
and the per-instance cadence derivation (`base_cooldown_s / heat`). Needs
`turn-cost-measure`, `swarm-dial-shape`, `swarm-share-setter` and
`swarm-scheduler-home` ruled.

**Slice 3: standing and citation.** Cut 12 leasing and review dates
(`mind-standing-decay`), the lead ruling (`swarm-lead-role`), and
`foreign-mind-ref`. After that, each further persona from the remnants is a
birth and nothing else: there is no per-mind code.

## Prior art cited

- Stride scheduling (Waldspurger and Weihl, MIT/LCS TM-528, 1995): pass,
  stride, remain on rejoin, ticket transfer. Lottery scheduling (OSDI 1994).
  Linux CFS `vruntime` and `min_vruntime` on wake. All summarised in
  gamecult-ops `docs/research/multi-persona-prior-art.md` (2cb9d04), section 2.
- Deficit round robin (Shreedhar and Varghese 1995): no banking while idle
  (same file, section 2).
- MAST (Cemri et al. 2025, arXiv 2503.13657): failures come mainly from system
  design and inter-agent misalignment. CrewAI's hierarchical manager. AutoGen
  speaker selection. Letta shared and private blocks (same file, section 1).
- Governed shared memory (arXiv 2606.24535). Collaborative Memory (Rezazadeh
  2025): private and shared tiers with per-fragment provenance (same file,
  section 3).
- Agoric systems (Miller and Drexler 1988), and Spawn's funding-as-priority
  (same file, section 4). These are the market alternative that
  `swarm-share-setter` does not recommend.
- Unverified this pass, from the author's knowledge:
  - Raft (Ongaro and Ousterhout 2014): a leader is a term-bound role.
  - The Debian Constitution: the Project Leader coordinates and delegates, the
    Technical Committee arbitrates, and nobody is required to do anything.
  - Holacracy's lead link.
  - Ostrom, *Governing the Commons* (1990): clear boundaries, collective-choice
    rules, graduated sanctions, nested enterprises.
  - Erlang/OTP supervisors restart children but do not direct their work.
  - Kubernetes API Priority and Fairness: shares per level, with borrowing.
- GameCult values: `operator-rapport.md` ("a community that will never, could
  never, betray its values and members"). Rulings `operator-means-shape-ends`
  and `she-owns-her-body`.

## Remnants mapping

**Slot, unfilled.** The sibling Eyes pass (`eyes-persona-remnants`) had not
written its facts file when this draft was finished. For each remnant, Self
fills one row: source and format, the proposed instance slug, the domain it
would advocate for, its import path to v0, and its status (born, proposed, or
archived as precedent only). Weksa is the first row (`weksa-mind`).
