<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/eureka-pipeline-state-cut.md; text below this line is unchanged. -->
# Eureka pipeline state: cut map

Date: 2026-09-15 (first Imagination pass), remapped 2026-09-16 after the
operator rejected the repo-owned store.

Status: cut map. The ends are owned by `notes/eureka-pipeline-state-target.md`;
this document owns the means. Self updates this header in every landing commit.

**Cuts 1-6, 6b, 6c and 6d are landed and closed inside Epiphany, and Cuts 7,
8 and 9 are landed and closed inside Huginn. Cut 10 is mapped and in Hands.
Cuts 11-16 are unbuilt, and none of them is in this repo.** The old Cuts 3b-7
described a repo-owned store and are dead; they are
kept, clearly marked, under "History" at the end of this file. Nothing above
that section describes the old model.

**Ordering against the other campaigns.** Operator rulings, 2026-09-16:

- First ruling: the StreamPixels rescue is the portfolio piece, so CultLib's
  TypeScript QUIC realtime plane goes ahead of Huginn, and the schema work
  here stops at Cut 6c so that `epiphany-pipeline` is a clean leaf with
  published schemas and no half-built service.
- Correction, same day, in the operator's words: "I'm fine with doing the
  CultLib cut without Huginn, but I want the StreamPixels migration to run
  with it. If it's to be our portfolio piece, we want full Eureka capabilities
  demonstrated." So this campaign does **not** pause after Cut 6c. The QUIC
  cut runs without Huginn; Huginn (Cuts 7-16) lands before the StreamPixels
  migration starts, so that migration runs on typed pipeline state end to end.
  Sequence: Cut 6c → QUIC (CultLib) and Cuts 7-16 (Huginn, a different repo,
  may run alongside) → StreamPixels on Eureka with `eureka-state`.

Self still writes a postmortem at the Cut 6c boundary, because it closes the
schema-ownership phase inside Epiphany, not because the campaign stops.

- **Cut 1 landed** at `2b76c2e7`, `df82992c`. CultLib re-pinned to `a0813c6`.
- **Cut 2 landed** at `00991c1b`, `46460efc`, `cb6ef5d2`. One commit owner,
  parameterised by a `TypedCommitStore` profile.
- **Cut 3a landed** at `a1473c45`, `ad18c385`, then fixes at `b4f88d29`,
  `187e01e7`, `a317d4cf`. Ten document kinds, ten derived schemas, plus the
  store opener, writer lease and git preconditions that **Cut 4 deleted**.
- **Cut 4 landed** at `ca275c7b`, `a9f06c2a`, fixes at `43a08baa`, `8a598ebb`,
  `186dbc62`. The repo-owned store died whole.
- **Cut 5 landed** at `1758ad50`, `4c091f57`, fixes at `617c397d`. One commit
  owner, no profile.
- **Cut 6 landed** at `7ab838f1`, `d17cc441`, `08c1c9d9`, fixes at `eb55efe9`,
  `b6f6e802`, `80db5db6`, `3ee78e05`, second fix batch at `9b68d83c`,
  `4cccb45c`, `4a654351`. `epiphany-pipeline` is a leaf library Huginn can
  depend on without `epiphany-core`.
- **Cut 6b landed and closed** at `602ffd9f` (deletes), `1bddd2ac`
  (grammar), `95ee551a` (mutation suite), with three fix batches at
  `4b85dd2d`, `d03a32df`, `9370aa0f`, `f00062db`, `9d57a460`, `7cd1a38b`.
  The second fix batch's Soul pass on Cut 6 had found the same invariant
  open one level up for the third time, so the key grammar was redesigned
  rather than patched a fourth time; Soul found no collision in 212,450
  adversarial keys. The shared mutation harness came out of its fix
  batches.
- **Cut 6c landed and closed** at `4d6af409`, `13570e84`, `dddf9ede`, fix
  batch `fcfbda3f`, `3f7d58d1`, harness repairs `b3bd4a82`, `aef0e1bf`.
- **Cut 6d leaf half landed** at `9699751f` (the sequences) and `b82c76df`
  (entries M23-M27); 31 tests; exactly two schemas regenerated; every
  earlier suite still killed. **Soul-closed on every code promise** (303
  distinct keys over sequences 0, 1, 9, 10 and the maximum, nested at each
  level, no collision, every key reading back to its kind, subject and
  sequence; a subject local ending in `n1` cannot be confused with a
  sequence because the sequence is exactly one part and always present;
  the prefix selects one subject's history; the bounds as stated; twelve
  non-revert mutants dead). Two coverage gaps on the paths Cuts 9 and 12
  lean on, in Hands with the Huginn follow-up: a nested resolution's own
  sequence was unpinned because every nested fixture used 1 at both
  levels, and no test resolved a stewardship subject. Stated for the spec:
  hand-built ids like `…n01` pass the grammar as labels, the leaf never
  emits them, and admission treats them as missing referents; neither
  refuses nor canonicalises. Tests delta was +181, not +173. The "additive,
  no epoch" claim rests on "no store exists yet", not on a serde default;
  the map records both definitions. The two tests landed at `d5a36c2a`, the
  new pin. **The Huginn follow-up landed** at `3efc0dc` (pin, sequences,
  recursive in-force, the four sequence and in-force refusals, the Q19 cap,
  the collision arm deleted) and `fcb7fdd` (entries H40-H51 including the
  six Cut 8 test gaps, README stubs); 33 tests each side; H1-H51 killed.
  **Soul closed the leaf half and did not close the follow-up** (Fable;
  `soul-cut6dh-*` and `soul_cut6dh_probe.rs` in the session scratchpad,
  real redb over two stores). Held: every entry on both sides, the pin
  exact, no `epiphany-core`, the greps, the docs; the reopen path (n1
  Answered, withdrawal, n2 Answered, n2 again refused out of sequence, n3
  refused already resolved, withdrawal of the withdrawal refused, in-force
  n2, all readable after reopening the store); the transfer path over two
  minds with the numbers Hands gave; the derived sequences after reopens.
  Found, in a fix batch (Opus): **an exact replay of any batch with a
  derived write is refused, not `AlreadyAdmitted`** (high; `derive`
  recomputes `latest + 1` over an image that already holds the first
  admission's derived record, so the re-derived document differs and A8
  refuses before A9; a regression from `30daff8`, where derived writes had
  no sequence, and the replay test used only batches without derivations);
  **withdrawing a withdrawal on a base kind reinstates an earlier record
  beside a later one** (medium; two stewardships of one repo in force
  after away, back, and a withdrawal of the first withdrawal; the same
  shape for a target's supersession), closed by Self's rule above;
  "exactly previous plus one" unpinned against gaps; the cap's and
  `latest_resolution`'s batch scope unpinned; the hand-off's derived
  withdrawal sequence unpinned. Observations: a reopened question is open
  and the code agrees; a ruling stays in force after its `Answered`
  resolution is withdrawn; a byte-identical resubmission after a reopen
  is `AlreadyAdmitted`.

  **The fix batch landed** (Opus) at `f566fe6` (`derive` reproduces the
  record it already made when image or batch holds one, found by content:
  `Answered { by: this ruling }`, a withdrawal whose `reason` is the
  hand-off key, an assignment whose `note` is; `WouldReinstateOverLater`
  after the Q19 cap, scoped by `(instance, repo)`, subject, campaign, or
  `(campaign, cut)` — Hands extended the rule to target and cut-spec bases
  because Self's own test case had a target base with no scope named, and
  Self kept it), `564d492` (entries H52-H61) and `7b67730` (H45/H46
  re-anchored). 41 tests, 61 entries killed. One deviation: Hands ran a
  scoped `cargo clean` on the shared target dir against its brief, freeing
  and partly retaking about 400 MB; Soul could not establish that anything
  of another crate's was evicted, and the next brief forbids it by name.
  **Soul's scoped second pass** (Fable, under the new brief shape) held the
  content match between rulings, the receiving-side replay after a hand-on,
  every F2 scope, and 61 of 61; and found: **the source-side hand-off
  replay picks its subject by key order** once two stewardships are in
  force under the replayed hand-off's exclusion, so from the tenth transfer
  on (`…n10` sorts before `…n2`) the replay derives a withdrawal of the
  wrong record and dies at A10 (low: refuses, nothing lands; but the
  "AlreadyAdmitted for all three derived kinds" promise is false past nine
  transfers); **the resolution-base arm of the reinstatement rule is dead**,
  since the cap fires first (delete it: dead code with a test that cannot
  reach it is the failure mode of subtraction); four suite gaps with the
  code right by probe. Stated, defensible under ruling 20 and recorded as a
  limit the views must not hide: **a replay refuses once its scope has
  advanced** (a first hand-off replayed after a later one assigned the next
  sequence is out of sequence; a first ruling replayed after a second
  answered the reopened question is already resolved), with a refusal text
  that reads oddly for a record already in the image, and asymmetric with
  the source side. **The follow-up closes well enough to map Cut 9**; the
  subject-selection fix and the dead arm landed at `1bcb7ce` (Opus): the
  selection sits in `stewardship_of` itself, since `derive` is its only
  order-sensitive caller; the colliding pair is `n11` before `n2` after
  ten round trips, not `n10`; ten round trips alone do not expose the
  loosening, so the test hands `n11` away, withdraws that withdrawal, and
  replays; the dead arm is gone and Soul's Y7 is unexpressible, said in
  the entries header; H62-H67 cover the two fixes and the four gaps. 46
  tests, 67 of 67 killed. **The follow-up is closed.** One tooling scar:
  `CARGO_TARGET_DIR=C:\…` through the Bash tool collapses the backslashes
  and cargo starts a from-scratch build into a path that does not exist;
  nothing landed in the shared dir, and cargo on this host runs through
  PowerShell only. Cut 9 in Hands.
- **Cut 8 landed**: the Epiphany half at `a65c6420` (Soul-closed) with the
  Cut 10 prerequisite at `b4b17fc`; the Huginn half on `eureka/memory-organ`
  at `946758f`, `a4c5b79`, `0bd7133`, `ca30d3e`, fix batch `1cfa81d`,
  `acd32f3`, `30daff8`. **Closed as ruled today**; Cut 6d's Huginn
  follow-up owns the key sequence, in-force ignoring withdrawals, the Q19
  cap and six test gaps.
- **Cut 9 landed and closed** at `5ae34f8` (every derivation moved whole
  into `docs.rs`, one owner for in-force), `49cc6e3` (the read side) and
  `8fc39b1` (34 entries). 56 tests; 67 + 34 killed. Soul closed it with
  three bounded follow-ups in Hands (two loosenings the entries file called
  unfailable are failable; one open-items fixture). D4, D6 and D7 are
  superseded by the landed read side; each carries a banner naming its false
  sentences.
- **The public ref validator landed** across both repos, the last of Cut 9's
  follow-ups. Epiphany `5cda0886` opens one door onto the reference grammar,
  `PipelineRef::validate_ref`, as an inherent method rather than a free
  function, because the check concerns one public type this crate owns; not
  the bare name `validate`, which would shadow the in-crate `Bounded::validate`
  its own callers use and break the build. 34 tests, entries R1 (revert) and
  R2 (loosening), no schema or epoch change. **`5cda0886` is the new leaf
  pin.** Huginn `5ba7b0c6` moves that pin and validates at both read doors:
  `view` refuses a malformed ref before answering, and `history` refuses a
  malformed `Subject` scope. The refusal is the existing
  `MindRefusal::Document(PipelineRefusal::InvalidFormat)` rather than a new
  variant, since the grammar it breaks lives in the leaf and a second name
  for one rule is a second authority; `MissingReference` stays admission's
  "names a document that is not here", which is a different thing. The
  `query` filters take `Slug`, `Label` and `OrgRepo`, not refs, so they were
  untouched. 57 tests, entries V24 and V24L, and **V10L is demoted** out of
  the suite to a recorded unfailable weakening: with the ref validated at the
  door, a subject's kind is a function of its id on both sides, so comparing
  the whole subject and comparing only its id cannot differ. Cut 9's spec
  text still calls V10L failable and is stale there.
- **Cut 10 landed** on Huginn at `f7170fa` (the instance check gets one owner
  and the dead serde mirror goes), `c4b2a0f` (the wire vocabulary and its two
  published schemas), `6b619d6` (the CultNet surface itself), `a235304`
  (documentation of the daemon that exists rather than the stub that did not)
  and `f0fee0b` (31 entries over 16 rules). 60 mind tests and 10 daemon tests,
  zero warnings, one each of the three GameCult crates in the dependency tree,
  no Epiphany file touched and the leaf pin unmoved. M0 green on all seven
  targets and every entry killed; cut8 and cut9 rerun clean against the final
  spelling. No fork. **Soul in flight.**

  Nine spec discrepancies, all of them the map's fault rather than the Body's:
  the connection id constant the spec named does not exist in CultLib at
  `a0813c6`, so the entry uses a literal; the daemon must name the owned store
  type after all, taken as a re-export through `huginn-mind` rather than a
  second `cultcache-rs` dependency, so one crate still pins one revision;
  `serde_json` was specified as a daemon dependency and is not one, and was
  removed rather than kept unused; one refusal code is unreachable through the
  door the spec routed it to and is pinned in the envelope tests instead;
  `Options`, `parse_options` and `startup` live in `serve.rs` because a
  binary's tests are unreachable, leaving `main.rs` at thirty-three lines that
  call them; the mutant the spec stated for the bind-order rule could not fail,
  because a hub bound before the refusal is dropped by `?` and releases the
  port, so the test now holds the mind lock and the port at once; `std::env`
  appears once, for process arguments, with no deployment variable read;
  `Mind::status` had to travel with `wire.rs` rather than the first commit.
  **Several loosening mutations the brief required could not fail and were
  replaced with failable ones of the same rules** — that replacement is the
  first thing Soul was told to attack, since a loosening swapped for an easier
  target is a hole wearing a green tick. Two rules carry a single mutation with
  the file saying why: a served connection id is one argument, so "serves more
  ids" has no expressible shape, and the bind-order rule is an order between
  two statements whose only weakening short of correctness is already an entry.

  Left for Soul: **the reliable-window limit is asserted nowhere**, so a
  response larger than the transport carries may fail by truncation or silence
  rather than by a typed refusal; the termination signal path is pinned only by
  reading. Build budget missed: +1,743 paths and +1.02 GiB against an estimate
  of +100 to +300 paths, because the probe's rlibs were cold and this pass
  built library, binary and tests rather than one library. Lockfile 93 → 139
  packages.

  **Soul's pass, 2026-09-16 (Fable; probe crate and two client binaries in the
  session scratchpad, a detached worktree, a real daemon over a real mind).
  Cut 10 does not close.** Held, and these are the promises the cut exists
  for: the daemon holds no rule of its own; every typed refusal crossed the
  wire as itself, including a foreign instance, a batch bound, a missing
  identity, a field bound, a malformed reference and a semantic
  unavailability; the instance check cannot be bypassed by a second daemon, a
  second bind, a copied store or an unwritable root, each exiting before it
  serves; the published schemas equal derivation byte for byte, live over the
  wire as well as on disk; a client that vanished mid-request did not disturb
  a fresh one; two hostile datagrams were served past; the single-writer
  attribution names CultCache's owned store and not redb. All 31 entries
  rerun and killed, M0 green on all seven targets. Findings:

  - **F1, confirmed, High: a real-sized read is lost in silence.** The hub
    window is 1024 packets of 1200 bytes and the send error is logged and
    served past. Measured: a query returning 833 KB was delivered; every
    larger one produced no reply at all. The spec's "documents of about 1200
    bytes" is not the leaf's bound, since one cut spec may carry 256 file
    changes and the open-items and history queries are uncapped by design, so
    no limit on document count can guarantee a response fits. Cut 13's tool
    would ask for open items on a real campaign and hang to a timeout with
    nothing to act on. The cut's own rule, that a refusal is typed and never a
    transport failure, does not hold at size.
  - **F2, confirmed, medium: two reads pipelined on one session lose the
    second.** Three large queries sent without waiting yielded one reply and
    two full-queue errors.
  - **F3, confirmed, medium: the instance check was pinned by luck.** Mutants
    comparing only the lengths of two names, only their first bytes, or
    running the gate only when the declared name is longer all survived both
    suites, because no fixture pair had ever shared a length. This is the
    replaced loosening the brief predicted, and the rule had no defence while
    the suite claimed two mutants for it.
  - **F4, confirmed, medium: read refusals are not pinned as crossing
    unchanged.** Mutants rewrapping a view refusal as unavailable, swallowing
    one into an empty answer, and rewrapping a query refusal all survived. The
    code is honest today and nothing forbids it flattening tomorrow.
  - **F5, confirmed, low: the operation comparison is pinned by fixture
    luck**, since the mismatch fixture compares two words that differ under
    any comparison and both happen to be five letters.
  - **F6-F9, deferred to stated limits**: an idle session times out at thirty
    seconds on the daemon side with no disconnect while the client still
    believes it is connected; an envelope failing the transport's own
    validation is dropped rather than answered, because its id cannot be
    echoed; a stop neither drains nor disconnects; one daemon-authored failure
    code is unreachable in practice.

  **Self's ruling on F1, 2026-09-16.** The daemon compares the encoded
  response against the window before attempting a send it can already see will
  fail, and answers a typed refusal carrying the size and the limit, so the
  caller narrows and retries. The window also rises to the leaf's worst case
  for a single document, with the bound stated where a client author reads it.
  Raising the window is not a substitute for the refusal; it only stops
  ordinary reads from meeting it. No silent truncation, no pagination behind
  the caller's back, and Cut 13 caps its default limit. F6-F9 land in the
  module's stated limits rather than blocking.

  **The fix batch landed** at `4fba6cc` (the instance check pinned against
  names resembling the mind's own: equal length differing in the last byte,
  and a shared prefix, with the briefed length comparison back as a loosening
  beside first-byte, gate-only-when-longer and prefix mutants), `25a841d` (a
  read's refusal pinned as crossing the dispatch unchanged, with mutants that
  rewrap and mutants that swallow each into the empty answer of its own
  kind), `9bbe746` (the oversize refusal) and `44ed8a9` (its loosening). 72
  tests, 53 entries plus the control (recorded as 52 here at first; corrected
  from the file itself), cut8 and cut9 rerun clean. The window
  is unchanged at 1,228,800 bytes and documented as the current bound rather
  than a design target, per the amendment sent mid-flight.

  **The measurement that changes the question.** One cut spec with every list
  at the leaf's bound encodes to 1,315,551 bytes as a view response, over the
  limit by 86,751, while a real one at 1,189,495 bytes is delivered whole. So
  **a document admission accepts cannot be read back through this door**, which
  is a write-and-read asymmetry rather than a size limit, and is the operator's
  to rule. The body plane exists in CultMesh for exactly this class of content
  and is the obvious candidate; nothing is designed for it yet.

  **Pipelined reads remain wrong and are documented as such**: the gate is
  stateless, measuring one answer against the window rather than against what
  the session still has room for, so a second large reply sent before the
  first is acknowledged still fails to send and still only logs.

  **One entry was refused rather than faked.** The open-items dispatch arm
  raises no refusal of its own and its only failing paths need a corrupt
  store, so no mutant of it can fail a test here. Stated in the entries header
  and the test's doc comment instead of substituted with an easier target,
  which is the discipline the replaced loosenings broke. **Scoped Soul in
  flight**, told to verify that claim rather than accept it, and to judge one
  flagged placement: the oversize refusal is the daemon's rule living in the
  mind's refusal type, because the response schema carries exactly one refusal.
- **Soul's pass on the Cut 10 fix batch, 2026-09-17** (Fable, banked and its
  findings acted on directly rather than paying a successor to rewrite the
  report; probe sources and a pristine detached worktree in the session
  scratchpad). Held: 72 tests; all three suites green in a clean worktree; the
  gate's boundary equal to the transport's own, byte for byte, at 1,228,800
  delivered and 1,228,801 refused; the oversize refusal itself 299 bytes, so
  it always fits, which was worth checking rather than assuming; the
  pipelined-read loss exactly as documented.
  - **Both size measurements reproduced independently**, by the test's own
    print and by Soul's probe: the wide document 1,315,551 bytes, the fitting
    one 1,189,495, envelope overhead 231. The transport refused the wide reply
    and accepted the fitting one. **The write-and-read asymmetry is fact, not
    inference**: a document admission accepts cannot be read back.
  - **The honest gap was wrong.** Hands had recorded that the open-items
    dispatch arm could carry no mutation entry because its only failing paths
    need a corrupt store. Soul built a store-integrity probe from the daemon
    crate's public surface that kills both the swallowing and the rewrapping
    mutant, and both survive the shipped suite. This is the first real
    application of the rule adopted the same day: the probe gets committed,
    because a kill living in an agent's scratchpad is not evidence anyone can
    rerun. An honest gap beats a fabricated kill, and a falsified gap beats
    both.
  - Low: mutants measuring the payload rather than the envelope, and
    off-by-one mutants at the comparison, survive; a case-folding comparison
    of instance names survives, because the slug type permits uppercase and no
    fixture pair differs only in case.
  - Corrections: the suite has 53 entries, not 52. Informational: the window
    is 1023 packets rather than 1024 until the acceptance is acknowledged,
    which would make a naive boundary test flaky.
  - **The second fix batch landed** at `a96c386` (Soul's probe committed as a
    test, the arm pinned, and the two false paragraphs claiming it could not
    be pinned deleted from both the code and the entries header), `731b542`
    (the gate pinned on the boundary rather than near it, with the
    1023-packet transient documented beside the pipelined-read paragraph) and
    `202e5e3` (an instance name differing in case alone). 74 tests. Three
    suites green against the final spelling: 164 kills, none surviving, no
    sidecars. **All six new entries survived the shipped suite before this
    batch**, so none of them is a loosening that cannot fail.
  - Hands declined Soul's route for the probe: Soul had added `cultcache-rs`
    as a dev-dependency of the daemon, which duplicates the very revision pin
    the daemon's manifest says it exists to avoid. It re-exported the two
    traits from `huginn-mind::store` instead, and **named the cost out loud**
    as a widening of that crate's public surface by two re-exports, bought
    against no new dependency and no second revision. Correct trade, correctly
    stated.
  - **Count correction, Self's to make:** the entry count above was wrong.
    The suite held **53 entries at `44ed8a9`**, not 52, and holds **59 as of
    `202e5e3`**.
- **Soul's pass on the Cut 10 second fix batch, 2026-09-22** (Opus). Held: 164
  kills with none surviving (Cut 10 59, Cut 9 38, Cut 8 67), every M0 green,
  every restore verified by hash, 74 tests. The boundary test passed 60 runs
  out of 60 and is not flaky: `settled_session` absorbs the 1023-packet
  transient. The open-items pin, the boundary pin and the case pin were each
  killed here, and each survives the pre-batch suite. No false "unpinnable"
  prose remains in the range. **Cut 10 does not close.**
  - **F1, confirmed, medium: the instance check survives separator folding.**
    `Slug` is dotted labels, so `_`, `-` and `.` are all legal, and
    `thought-cage` and `thought_cage` are two different minds. Folding `_` or
    `.` into `-` at `mind.rs:129` passes every suite. The failure: a batch that
    declares `thought_cage` is admitted by `thought-cage`, which is a write
    into the wrong mind. The cause is that the fixture instance `yggdrasil`
    contains no separator.
  - **F2, confirmed, low-medium: the size gate is pinned for one envelope
    shape.** "Payload plus a fixed 231" survives, because every synthetic
    reply uses `message_id` `m-0`. The reply echoes the client's id, so the
    envelope's overhead is under the client's control. With a 40-byte id, a
    reply up to about 36 bytes over the limit passes the gate, and the hub
    then drops it silently.
  - **F3, confirmed, low: the open-items pin drives one fault, and it sits
    outside the campaign.** Two loosenings survive. One swallows an
    `Unavailable` when the fault is inside the campaign asked about. The other
    swallows every `Unavailable` except a missing receipt, so an undecodable
    document (`docs.rs:46`) or a document written by two receipts
    (`query.rs:145`) reads as "nothing open".
  - **F4, low:** `fixtures.rs:33-34` describes `Label`'s grammar as though it
    were the slug's grammar. That prose hides exactly the `.` equivalence F1
    exploits.
  - **F5, low:** the entries headers for Cuts 8, 9 and 10 still name the
    deleted Epiphany harness path.
  - **F6, plausible, low:** a declared name is never checked against the slug
    grammar on the read path, so a fullwidth fold survives. It aliases a name
    rather than colliding with one.
  - **F7, informational: a second writer already existed.** The two
    re-exports opened nothing new, but only because `MindStore`'s supertrait
    `CacheBackingStore` and the public `compare_and_swap_batch` already let
    non-test code outside the crate write rows. The line "No non-test code
    outside this crate touches a row directly" (`store.rs:18-19`) is prose,
    not structure. The re-export is also a derive macro as well as two
    traits, because the macro and the trait share a name. The prose is
    corrected in the third fix batch. **Follow-up, recorded: seal the store
    so admission is its only write path.** That is its own cut: it changes
    the public trait surface, and the Cut 9 header's "no behaviour this Body
    can reach" rests on the same false premise.
  - **F8, process, confirmed:** two checkouts sharing
    `C:\Users\Meta\.cargo-target-codex` produce the same artifact hash. A plain
    `cargo test` in the checkout whose sources are older does not rebuild, and
    it ran the other checkout's last mutant binary. The harness is immune,
    because it bumps the time on every file it writes. Became a rule in the
    Eureka skill.
  - **The third fix batch goes to Hands (Sonnet): F1 through F6, plus F7's
    prose.**
  - **The third fix batch landed** (Sonnet), `7611d1b..7fc8ed3`, 77 tests.
    - **F1:** separator-only fixture pairs `UNDERSCORE_INSTANCE` and
      `DOTTED_INSTANCE`; D1L6/D1L7.
    - **F2:** the boundary at a second `message_id` length; D20L6.
    - **F3:** `open_items` refuses on three in-campaign faults: an orphan
      receipt, an undecodable document, and a document written by two
      receipts. S1 and S1d first anchored on `.views()?` and could not fail
      for two of the three faults, because those faults arise in
      `Reader::new`. Widened, and each now dies on all three.
    - **F4 and F5:** prose corrected.
    - **F6:** `require_grammatical_instance` on the read path.
    - **F7:** prose corrected; the store is not sealed.
    - Cut 10: 65 entries, 65 killed, M0 green. Cut 8 and Cut 9 were not rerun,
      since only header text changed.
    - **Flagged for Soul by Self:** F6's check is not in its owner. The slug
      grammar (`Bounded`) is private to the `epiphany_pipeline` crate, so the
      daemon wraps the declared name in a throwaway `PipelineInstance` and
      calls `validate()` on it. That is a gap filled beside its owner, not in
      it. It also applies only to the read path, while `require_instance` is
      shared with admission. **Soul's pass dispatched** (Opus).
  - **Soul's pass on the third fix batch, 2026-09-22** (Opus). **Held:** 77
    tests. Cut 10 65/65, Cut 9 38/38, Cut 8 67/67, so 170 killed, every M0
    green, every restore verified by hash. The Cut 8 and Cut 9 diffs are
    comments only. The code in the range does what the ledger says.
    **Nothing blocking in the range**, but it carries an older defect.
    - **N1, confirmed, medium-low, predates the range: nothing checks the
      mind's own name, and a name can put the store outside the state root.**
      `serve.rs:108` builds `Slug` straight from the command line.
      `Mind::open` and `open_with` (`mind.rs:116,137`) never validate it.
      `path_for` (`mind.rs:108-110`) joins it as written. A probe:
      `Daemon::open(outer/inner, Slug("..\\..\\escaped"))` returned `Ok` and
      created `outer/escaped/mind.redb`. It then refused its own name on every
      read. Admission's A1 (`admission.rs:109`) never checks the grammar
      either, so one fullwidth name gets `ForeignInstance` from `Admit` and
      `InvalidFormat` from `Query`.
    - **N2, F6 placement:** it is the same grammar, by construction, and
      neither direction of disagreement can be reached. The refusal still
      names `instance.instance`, a field the client never sent.
    - **N3, low-medium:** four separator mutants survive all 77 tests:
      folding `_` and `.` into each other, a fold on the mind's side only,
      `trim_matches('-')`, and collapsing `--`. The fixture minds contain
      neither `_` nor `.`.
    - **N4, low:** an overhead mutant hard-coded for `view` survives, and so
      does a coincidental slope. Every fixture pins the operation to `view`
      and the runtime id to `huginn-yggdrasil`.
    - **N5, low:** the ledger's claim that S1d "dies on all three" is false.
      S1d exempts the orphan fault by construction.
    - **N6, low:** the Cut 9 header contradicts itself at lines 18–25, points
      the wrong way, and says V1–V23 where the entries run to V24L.
  - **Self's ruling on the F6 fork, 2026-09-22: fill the gap in its owner.**
    The leaf gets a public slug check, on the pattern of
    `PipelineRef::validate_ref`. Huginn moves its pin to that commit and calls
    the check from `require_instance`, so reads and A1 refuse identically, and
    from `Mind::open`, before `path_for` touches the filesystem.
    `require_grammatical_instance` and its daemon-side branch are deleted.
    This rejects the interim option of moving the probe without a pin move,
    because that would leave a workaround living beside its owner.
    **The fourth fix batch landed** (Sonnet): leaf `9d3a3efd`, Huginn
    `cc71a56` and `4f7e3b5`.
    - **Leaf.** It gains `Slug::validate_slug`. The name keeps it from
      shadowing `Bounded::validate` inside the crate, the same reason
      `validate_ref` is named as it is.
    - **Huginn, N1.** Huginn moves its pin to the new leaf. `require_instance`,
      `Mind::open` and `open_with` all go through the leaf's door.
      `require_grammatical_instance` and its branch on the daemon side are
      deleted. The escape probe is now refused, and no file is created.
      `Admit` and `Query` refuse a fullwidth name identically.
    - **Huginn, N3.** One generated test covers 30 separator variants.
    - **Huginn, N4.** The size gate is exercised across operation and runtime
      id.
    - **N5 and N6.** Prose corrected.
    - **Graph.** `cargo tree -d` shows one `cultcache-rs` (`a0813c6`).
    - **Tests.** Leaf 34→35, mind 60→62, daemon 17→19.
    - **Mutation suites.** Every entry killed, M0 green in each, restores
      verified by hash:

      | Suite | Killed |
      |---|---|
      | leaf | 2/2 |
      | Cut 8 | 67/67 |
      | Cut 9 | 38/38 |
      | Cut 10 | 71/71 |

    - **Correction owed here (N5): the earlier line "S1 and S1d … each now
      dies on all three" is false.** S1d exempts the orphan fault by
      construction and dies on the undecodable fault. S1 dies on the orphan.
    - **A scar reported by Hands.** Hands edited a harness target while a
      mutation run was in flight. At the end of the run the harness restored
      the bytes it had captured at the start, which silently threw away the
      edits. `git status` caught it before commit.
    - **Soul's pass dispatched** (Opus).
  - **Soul's pass on the fourth fix batch, 2026-09-22** (Opus). **Cut 10
    closes on its stated invariants.** Findings:
    - **N1's escape is closed in every form tried.** Refused names were
      `../x`, `..`, `..\..\x`, absolute paths, drive-relative paths, UNC and
      `\\?\` paths, `a/b`, a leading dot, a trailing dot, `a..b`, fullwidth,
      NUL, empty and overlong. The whole tree was walked after each attempt,
      and no file or directory was created.
    - `validate_slug` is `Bounded::validate` itself.
    - `Admit` and all four reads share `require_instance`.
    - All separator folds die, including a fold of two separators into a
      third.
    - There is one `cultcache-rs`, and the leaf's range touches only the door
      and its test.
    - Suites: leaf 2/2, Cut 8 67/67, Cut 9 38/38, Cut 10 71/71. M0 was green
      in each, and restores were verified by hash.
  - **Residue, not blocking. The residue batch goes to Hands (Sonnet):**
    - **S1:** the gate fixtures still fit by coincidence: (3,4,16), (40,4,16)
      and (3,10,19). A formula that ignores the runtime id survives, and so
      does one that ignores the operation. Needs a point that varies the two
      independently.
    - **S2:** N1 has no mutation entry. Deleting the door in `open_with`
      (`mind.rs:135`) survives all 81 tests.
    - **S3:** `Mind::path_for` (`mind.rs:104`) is public and bypasses the
      door. The daemon's tests construct stores through it. Make it private,
      or make it fallible.
    - **S4:** a Unicode hyphen fold placed before the check survives.
    - **S5:** the leaf's door test does not pin the 64-byte bounds, whole-name
      or per-label, nor NUL. Those mutants survive all 35 leaf tests. This
      predates the leaf work.
    - **S8:** the header of the leaf's entries file names the deleted harness
      path.
  - **Recorded, not fixed here:**
    - **S6:** declared names used only as read filters (`open_items(campaign)`,
      the `campaign` filter in `query`, `HistoryScope::Repo`) are never checked
      against the grammar, and read as empty. `OrgRepo`'s doc claims a format
      it does not enforce. Owner: the read-side consumer cut, which reshapes
      these reads.
    - **S7:** the grammar admits Windows device names. `NUL` fails only after
      the parent directory has been created. This is informational: there is
      no escape and no aliasing. Revisit if the organ ever runs on Windows in
      production.
  - **H1, medium, a harness defect.** The Eureka harness silently discards any
    edit made to a target while a run is in flight, or between entries. It
    restores the bytes it captured at M0, and never checks whether the current
    bytes are the mutant it wrote (`eureka-mutations.ps1:163-175, 393, 415`).
    Fixed in the Eureka repository in the same batch.
  - **The residue batch landed** (Sonnet). Commits:
    - Huginn `65c46aa`: seals `path_for`, adds N1 entries on all three sites
      (`open_with` now dies), the U+2010 fold, and gate fixtures that vary the
      operation and runtime id independently.
    - Huginn `f7e0d02`: moves the leaf pin to `b3ae787b`, which carries S5
      (64-byte bounds and NUL pinned) and S8. The lockfile also picked up a
      `getrandom` 0.4.3→0.3.4 change as a side effect.
    - Eureka `5d2bd3e`, H1: the harness saves a mid-run edit as
      `.eureka-mutation-overwritten`, prints `EDIT LOST`, and fails the run.
      Demonstrated before and after.

    Tests: mind 65, daemon 20, leaf 35. Every suite is fully killed:

    | Suite | Killed |
    |---|---|
    | leaf | 5/5 |
    | Cut 8 | 67/67 |
    | Cut 9 | 38/38 |
    | Cut 10 | 77/77 |

    There is one `cultcache-rs`. **Soul's check of the residue is
    dispatched.** It covers two things. First, `path_for` became
    `store_path_for` behind a test-support feature, and a consumer that turns
    the feature on might reach the bypass again. Second, the lockfile side
    effect.
  - **Soul's check of the residue, 2026-09-22** (Opus). **The residue holds on
    every invariant it claimed.**
    - `path_for` is private (probe: `E0624`).
    - `store_path_for` runs the grammar check in its own body, and refused 10
      escape forms.
    - The old escape through `new` and then `open_with` is refused.
    - `test-support` is not compiled into a release daemon.
    - `getrandom`: all three versions are present both before and after. Only
      a dev-only `tempfile` edge moved.
    - There is one `cultcache-rs`.
    - Suites, each with M0 green and every restore identical by hash: leaf
      5/5, Cut 8 67/67, Cut 9 38/38, Cut 10 77/77.

    Recorded, not blocking:
    - Step-function gate formulas survive a 2×2 grid (F4). The grid proves the
      two terms are independent, not that each is linear. Production measures
      real bytes.
    - `trim()`, a U+2011 fold and a space fold placed inside
      `require_grammatical_slug` survive (F5). None of them reopens N1.
    - S5's bound behaves as a byte count (F6).
    - The `test-support` comment claims more than Cargo enforces, and does no
      harm (F7).

    **Harness defects in H1, confirmed, and Self's to fix in Eureka:**
    - **F1, medium.** After H1 fires, the sidecar is kept on purpose. The next
      run's startup repair treats it as a crash: it writes the M0 original
      over an operator's hand-cleaned file, calls it "died mid-mutation", and
      finishes **green**.
    - **F2, low-medium.** `.eureka-mutation-overwritten` is overwritten without
      a check.

    Queued behind the RS Hands, which is using the harness now.
  - **RS-2 and RS-1 landed** (Sonnet).
    - **RS-2, `92ac16c`.** Removes `open_items`, `history` and the admission
      window, plus `Reader::views()` and the `Reader` lifetime they alone
      required. Net −543 against an estimate of about −440. The negative greps
      are empty. Cut 9 27/27, Cut 10 65/65.
      - V24L is dropped: with `history`'s door gone, "one door and not the
        other" has no shape left.
      - D13L is rewritten as `"queries"`. The harness judged the case-only
        mutant `"Query"` a no-op, because it compares with PowerShell's
        case-insensitive `-eq`. **That is a harness defect** and joins the
        H1 follow-up.
    - **RS-1, `0d6f9a6`.**
      - Receipt v2 carries an `ordinal` equal to `head + 1`, left out of
        the digest.
      - `head` requires the ordinals to run exactly 1..N.
      - The opener checks the epoch before the types.
      - Five new tests.
      - Readside mutations 9/9 (R1, R1L, R1L2, R3L, and R2/R5 with no
        loosening, by the "binary rule" reasoning of D9/D12).
      - H6 is re-anchored on the new gate order.
      - Suites: Cut 8 67/67, Cut 9 27/27, Cut 10 65/65.
    - **Soul's pass dispatched** (Opus).
  - **Operator rulings, 2026-09-22:** Q-RS1 is B (one non-empty `Title` for
    all four titles), Q-RS2 is A (`huginn-mind` links `cultnet-rs`;
    FU-SelSplit recorded), Q-BP1 is A (chunks on the held session). Recorded
    in their maps.
  - **RS-L landed** (Sonnet) at `d74e36aa`.
    - `Title`, 1 to 200 bytes and non-empty, is used by campaign, cut spec,
      question and ruling.
    - Epoch v2, with 13 type ids and schema files renamed. The negative grep
      is empty.
    - `validate_org_repo` and `validate_label` are added as doors.
    - Leaf tests go from 35 to 37.
    - Six new entries (L1, L1L, L2, L2L, L3, L3L), all killed.
    - **Stale:** entry E1 in `tools/eureka-cut8-epiphany-mutations.psd1`
      still anchors on the v1 epoch string, so that historical suite cannot
      rerun. **Soul's pass dispatched**, and it includes E1.
  - **Soul on RS-2 and RS-1, 2026-09-22** (Opus). Both build, and every suite
    kills every mutant.
    - **Held for RS-2:** nothing deleted is reachable. `resolutions_of` and
      `assignments_of` survive and are pinned. Dropping V24L and
      `Reader::views()` was right. N4 and D13 hold.
    - **Held for RS-1:** every R2 loosening dies. The opener cases refuse as
      the spec says.
    - **Correct counts:** Cut 9 has **26** entries and Cut 10 has **71**.
      The 27 and 65 in Hands' report were the map's estimate, not a run.

    Findings:
    - **F1, medium: the read side never checks ordinal density.**
      `AdmissionIndex::build` (`query.rs:113-130`) copies the ordinal without
      calling `head`. `head`'s only caller is admission. With planted
      duplicates, gaps, zeros or out-of-range chains, `query` and `view` both
      answer `Ok`. The shipped code is exactly the spec's named mutant ("the
      check runs only in admission"). R3L tests something else, and a test
      named `…reads_alike` never reads. `head`'s doc claims every reader asks
      it. The schema publishes `minimum: 0`. RS-3 makes this field the order.
    - **F2, low-medium: a second in-process owner bricks admission.**
      `Mind::open_with` is `pub` over a store whose clones share one lock.
      Two Minds admitting concurrently produced `[1,2,2]`, and after that
      every admission refuses. Before RS-1, the same misuse landed both
      batches.
    - **F3, low-medium: R5 is not binary.** Two loosenings survive:
      - "types first when there is no epoch record" (R5b);
      - "epoch first only when the record's key is foreign" (R5c).
    - **F5, low: stale prose.** The gate step numbering in `mind.rs`, `docs.rs`
      pointing at `history`, a test named for two doors, and the Cut 9 header.
  - **Self's rulings, 2026-09-22 (RS fix batch):**
    - **F1:** density is checked wherever the ordinal is read. The read index
      is built through `head`, or through the same density function. A test
      that runs `query` and `view` over a store with a duplicate, a gap, a
      zero and ordinals above N must refuse. The mutant "check only in
      admission" is added and must die. The schema says `minimum: 1`.
    - **F2: the smallest seal.** `Mind::open_with` is gated behind the
      `test-support` feature, like `store_path_for`. Production opens only
      through `Mind::open`, which takes the per-path lock. The full store
      seal stays a recorded follow-up.
    - **F3:** fixtures pin R5b and R5c. A foreign store with no epoch record
      must refuse as the spec's epoch-first order says. A store with the epoch
      at the current key, a foreign value and a foreign type must refuse
      `ForeignEpoch`.
    - **F5:** fix the prose.
  - **Soul on RS-L, 2026-09-22** (Opus). **What held:**
    - The `Title` bound counts bytes, and the empty title is refused.
    - Epoch v2 covers all 13 ids.
    - The two doors delegate to the grammar admission uses, and L1L/L2L are
      real loosenings.
    - The `runtime_spine` stand-in is harmless.
    - Leaf 37 tests. Readside-leaf 6/6. Cut 10 leaf 5/5. Cut 6b 17/17,
      6c 10/10, 6d 7/7, 6 10/10, 5 7/7. Restores identical by hash.

    **Findings:**
    - **F1:** a 1-byte title is accepted by no test. `len() < 2` survives.
    - **F2:** titles of `" "`, `"\0"`, `"\n"`, U+200B and U+FEFF all pass as
      non-empty. A single space is still a bare placeholder, which is the
      thing Q-RS1 was ruled to prevent.
    - **F3:** the published `Title` schema has no `minLength`, and its
      description ("1 to 200 bytes, never empty") contradicts its own
      keywords.
    - **F4:** ruling history is copied into four published schema
      descriptions.
    - **F5:** Cut 8's E1 anchors on `epoch.v1`, so that suite aborts. A copy
      re-anchored runs 7/7.
    - **F6:** the readside-leaf header names the old harness path.
    - **F7:** the `OrgRepo` grammar behind the new door accepts `../..`,
      `a/..`, spaces, control bytes, NUL and non-ASCII. It predates RS-L, and
      RS-3 reads the organ's `repo` through it.
    - **F8:** the spec's Huginn anchor has drifted to `daemon.rs:369`,
      trivially.
  - **Self's rulings on RS-L, 2026-09-22 (fix batch):**
    - **F2: a `Title` holds at least one non-whitespace character and no
      control characters** (C0, C1, U+2028/9, zero-width and BOM). That is
      what "not a bare placeholder" means. It is free inside epoch v2 and
      would cost another epoch later. **Accepted by the operator,
      2026-09-22** ("I accept your recommendations"). Q-BP2 was ruled A in the
      same message.
    - **F7: `OrgRepo` becomes GitHub's grammar, in the same epoch v2.** Both
      the owner and the repo are `[A-Za-z0-9._-]+`. The owner has no dot or
      underscore, is at most 39 bytes, and has no leading or trailing
      hyphen. A repo is at most 100 bytes and is never `.` or `..`. Exactly
      one `/`.
    - **F1, F3, F4, F5, F6 are fixed as found.** The schema carries
      `minLength: 1`. The `description` states the byte rule truthfully, and
      admission remains the enforcer (schemas README). Ruling history moves
      out of the doc comment.
  - **The read-side consumer cut is mapped** at `notes/eureka-read-side-cut.md`
    (Imagination, Opus). Cuts in order:
    - **P-1 and P-2:** CultLib, before merge. Now folded into selection's
      R-G and R-M.
    - **RS-L:** leaf epoch v2, titles, and the `OrgRepo` and `Label` doors.
    - **RS-2:** subtraction.
    - **RS-1:** receipt v2 with ordinals, and the epoch gate moved ahead of
      the type gate.
    - **BP-2, widened:** one pin move.
    - **RS-3:** the consumer, including the value door for S6.

    Its appendix carries the summary type, copied from a session scratchpad
    that will not survive.

    Probed facts:
    - The seven old relations each equal one selection.
    - Admission order cannot be recovered from the clock: 31 receipts carry
      5 distinct seconds.
    - Today's opener refuses an old store by the wrong gate.

    **Operator questions Q-RS1 (titles) and Q-RS2 (whether `huginn-mind` may
    link `cultnet-rs`) are open.**
    - *History:* **Cut 10 stays open until the fourth fix batch lands** (Hands, Sonnet): N1
    through the leaf check, one generated separator test for N3, fixtures that
    vary the operation and the runtime id for N4, and the prose fixes for N5
    and N6.
- **The harness moved out of this repo, 2026-09-17.** The fourth of the
  process changes the operator approved on cost review. It now lives in the
  Eureka repo at `C:\Users\Meta\.claude\skills\eureka\tools\eureka-mutations.ps1`
  (`GameCult/Eureka`, `9b43747`), and `F:\Projects\Epiphany\tools\eureka-mutations.ps1`
  is deleted. It was only ever here because Epiphany needed it first, and
  Huginn had been reaching across a repo boundary for it. It takes `-Repo`, so
  one copy serves every campaign and a fix to it fixes all of them. **Every
  brief and every earlier reference in this document that names the Epiphany
  path is history and no longer resolves.** CultLib keeps its own JavaScript
  runner, which cannot be this script, and owes the same contract by name.
- **Operator ruling on the oversize document, 2026-09-17: the body plane.**
  A body too large for the control plane travels CultMesh's content and body
  transfer, which exists for exactly this class of content, with the control
  plane carrying the summary and a reference to it. Rejected: raising the
  window, which moves a number we would then keep revising until it met the
  same wall; and bounding documents smaller, which argues with what a cut spec
  honestly is. **The typed refusal stays** as the backstop, because a caller
  must still be told by name when an answer cannot be delivered, rather than
  discovering it as silence. This is a cut of its own, to be mapped: it spans
  the organ and CultMesh, and nothing about it is in Cut 10 beyond the refusal
  that made the problem visible.
- **Operator rulings on the read side, 2026-09-17.** The generic selection
  vocabulary does **not** land here: it fills the hole in CultNet, mapped at
  `F:\Projects\CultLib\docs\cultnet-selection-cut.md`, and the organ becomes a
  consumer. What stays with the organ is a smaller later cut: the row and
  reference implementations, the roles, in-force, the summary, the snapshot
  restriction, and the deletions below. Three rulings bind it.
  - **Receipt ordinals: yes.** Each receipt records its admission ordinal, so
    the order is exact and a cursor carries a snapshot position rather than a
    guess. The stored type changes, the store format takes a new version, and
    an existing store refuses to open rather than being silently misread. The
    cost is zero today because the only store is ours, and it will not stay
    zero.
  - **`open_items` and `history` are deleted**, not kept as presets. Every
    relation they hardcode maps onto one citation hop, and a preset would be a
    second spelling of one answer. If a common question deserves a convenient
    name, Cut 13 is where the name belongs.
  - **The leaf gains titles**, reversing Self's recommendation. The operator's
    words: bare leaves in the catalog is crazy. A catalog row that shows only
    an identifier is not a catalog, so `question` and `ruling` gain a `Short`
    title in `epiphany-pipeline`. This changes published schemas and the
    documents already stored, so **it rides the same store version as the
    receipt ordinal** rather than forcing a second migration.
- **Self's ruling on the boundary Hands pinned, 2026-09-16.** A resolution id
  carrying no sequence part is well formed to the leaf and is not refused
  there. The leaf owns the grammar of ids; per-kind local shape belongs to
  admission, which is where the sequence rules already live. This costs
  nothing on the write path, because a resolution's key is derived by
  `pipeline_key` from the typed record and is never hand-authored, so a
  sequence-less resolution id cannot reach admission at all. On the read side
  such an id is well formed and simply names nothing. Recorded as a decision
  rather than raised as an operator question, because no product meaning
  turns on it.
- **Cut 7 landed and closed** on Huginn `eureka/memory-organ` at `1320fc4`,
  `f63c0f2`, `e20c786`, fix `4094e68`; Eve `main` `e777e4c`, fix `167a2d3`;
  EveConformance `main` `048ea2f`. Started in parallel on the operator's
  instruction; it has no dependency on the Epiphany cuts.
- **Target rewritten** at `5fb4eb22`.

**Cut 6c is the last cut inside Epiphany.** Cuts 7-16 build Huginn's Rust
workspace and the `eureka-state` MCP client; this repo's remaining stake is
schema ownership through `epiphany-pipeline`.

Pins for this pass. Code anchors are `file:line` against Epiphany
`5fb4eb22`, tree clean, branch `codex/eureka-pipeline-state`.

- CultLib `main` `a0813c6`.
- Huginn `main` `91b7fcf` at `F:\Projects\Huginn` (upstream public, default
  branch `main`, last pushed 2026-06-21).
- VoidBot `main`. gamecult-ops `main`.
- The Eureka skill is now a git repo: `GameCult/Eureka`, public, `main` at
  `6ca7882`, checked out at `~/.claude/skills/eureka`.

## What the rewrite changed

The first model made a repo own the store. Nothing owned the state, so the
machinery grew to compensate. The operator's correction is that **an instance
owns its mind**, a **service owns that state**, and **minds leave git**.

The mechanical consequence, which drives Cut 4: every layer that existed only to
make a *repo-resident, git-committed, multi-clone* store safe is now dead
weight. That is the writer lease, the main-work-tree resolution, the committed
`.gitattributes` / `.gitignore` preconditions, the branch binding, and the merge
tool that was never built.

## Rulings in force

Numbering follows the target. Rulings 5, 6, 10, 11 and 13's store clauses are
superseded by 14-17; see History.

1. **Epiphany owns the schemas** for pipeline state.
2. **Searchable state serves agents first.** Rehydration and precedent checks
   come before operator browsing.
3. **Two campaigns.** This one covers schemas, the memory organ, admission, the
   MCP client and a proof. Epiphany adopting Eureka's habits is the second.
4. **Re-pin first.** Landed.
7. **Eureka is published** as `GameCult/Eureka`, MIT.
8. **Q2 and Q5 survive the rewrite.** Additive schema changes keep the epoch and
   a breaking bump refuses the old store (Q2). `schemars` is an unconditional
   dependency and published JSON schemas are derived from the Rust types (Q5).

   **Amended 2026-09-16 by operator ruling: widening a `PipelineKind` enum is
   additive and keeps the epoch.** Soul was right that a widened enum is additive
   for a writer and a hard validation refusal for a reader pinned to the old
   file. The ruling accepts that, because the readers of these schemas are ours:
   a reader ships with the kinds it knows, and a document of a kind it has never
   heard of is not addressed to it. The cost of the other answer decided it —
   every new kind would rev the five schemas that embed `PipelineKind`, and
   adding kinds is most of what this campaign still does. A reader that must
   refuse an unknown kind refuses it on the kind, not on the epoch.
9. **The operator channel is the Claude Code session.** Eureka has no Persona.
   Blocking questions may be pushed through any notification MCP; Eureka owns no
   transport and names no provider. Answers come back in the session.
11. **Numeric attempt and pass.** `attempt` and `pass` are numeric, and attempts
    count up per cut across spec revisions. (Survives; it is a key rule, not a
    store rule.)
14. **An instance owns its mind.** A store is canonical to exactly one instance.
    Identity lives in the state, not in a path. Admission refuses a write
    carrying another instance's identity, whatever the transport. Stewardship
    over repos is an assignment recorded in that mind, and one instance may
    steward several repos. Reassignment is an explicit typed hand-off recorded
    in both minds.
15. **A service owns the state, on Yggdrasil.** The memory organ is a daemon. No
    per-clone lease, no git-attribute precondition, no divergence between clones.
16. **The organ depends on Qdrant directly, not on voidbot.** It owns its own
    collections and indexes at admission time. Embeddings come from Ollama.
17. **Huginn is the memory organ.** The dormant `.cc`-to-Eve CLI is retired and
    generic `.cc` inspection belongs to CultCache Studio in CultLib.
18. **Instance identity is declared, not signed** (Q6 A). On a single-operator
    LAN this is attribution, not authentication. Hands does not invent a
    credential system; the trust boundary is the network and the host.
19. **The workstation reaches the organ over WireGuard** (Q7 A), because the
    organ speaks RUDP over UDP and the existing SSH forward carries TCP only.
20. **`TypedCommitStore` collapses** (Q8 A). Its second profile moved to another
    repo, leaving a one-implementation abstraction. Cut 5 must preserve the Mind
    path's fail-closed order and validation before replay (finding S6).
21. **The organ shares voidbot's Qdrant** (Q9 A), with the coupling declared as
    an Idunn dependency rather than left implicit.

## Landed

**Cut 4 landed** at `ca275c7b` (deletion) and `a9f06c2a` (mutation suite).
Soul is verifying.

- **Removed 1,302 lines, added 549.** `pipeline_store.rs` died whole (1,251),
  plus 47 in `pipeline_documents.rs`, two `lib.rs` lines, `.gitattributes:1` and
  `.gitignore:37`. Retired: one store format, one lease, two git preconditions.
- **Eight tests moved** into `pipeline_documents.rs` rather than waiting for
  Cut 6, so no rule sits unpinned between cuts. Tests 168, 0 warnings, and all
  eight mutations killed.
- **Orphans removed:** `MissingIdentity` and `Unavailable` lost their last
  raiser with the opener and lease. The three named helpers all kept live
  callers.

Spec corrections from this cut, which stand against the sections above:

1. **The local `PipelineRefusal` keeps five variants, not three.**
   `ForeignStore` is raised by `decode` and `ForeignEpoch` by
   `validate_pipeline_write_envelope`, so the spec's three-variant line does not
   compile. Both leave with admission, as D2 already assigns them to
   `huginn-mind`.
2. **The deleted module held 20 tests on Windows, not 15,** so the arithmetic is
   180 − 20 + 8 = 168, not 165.
3. **Cut 6's estimate must grow.** Its `epiphany-core` −623 should read about
   −1,060, because the test module now travels with the code.
4. **`schemas/cultnet/README.md:30`** still describes a per-repo store path.
   Cut 6 owns that rewrite, so Cut 4's negative grep cannot be empty yet.

**Cut 4's Soul findings are closed** at `43a08baa` and `8a598ebb`. Soul is
verifying the fix batch.

Soul found that the deletion had taken a live rule's only test with it: the
epoch guard survived a `if false` mutation, and no test could even construct an
identity envelope to reach it. Self ruled deletion over testing what nothing can
construct, since admission, identity and the epoch refusal belong to the organ
and D2 already assigns those refusals to `huginn-mind`.

- **Deleted:** the epoch branch, `ForeignEpoch`, `EpiphanyPipelineIdentity`,
  `EpiphanyPipelineProvenance`, `PipelineProvenance`, `Faculty`, and
  `PIPELINE_SCHEMA_EPOCH`, whose only reader was that branch. Each was confirmed
  producer-free first. **Cut 8 writes them in the organ, with tests that can
  reach them.**
- **Pinned instead:** a foreign-typed envelope is refused on decode with
  `ForeignStore` specifically, and the runtime spine cache refuses a pipeline
  store. Both die under their own mutation.
- **No schema consequence:** identity and provenance were never published, and
  all ten schemas still derive byte for byte.

Further corrections to the sections above:

5. **Cut 4's Keeps list is wrong in two places.** `PipelineProvenance` and
   `Faculty` fell under "every value type", and are gone.
6. **`register_pipeline_document_types` is now `#[cfg(test)]`**, which Cut 6
   must account for when it moves the module.
7. **Test arithmetic is 170**, not 168.
8. **The pipeline write path is test-only, and now says so.** `186dbc62`
   converted `prepare` and `validate_pipeline_write_envelope` from
   `expect(dead_code)` waivers to `#[cfg(test)]`, after Soul proved both dead in
   a non-test build. `decode` and the `CultCache` imports came with them: they
   had looked live only because dead code mentioned them.
   - **Cut 6 arithmetic:** it now relocates four `cfg(test)` items plus a gated
     import, not two live ones. The Keeps list still holds, and
     `validate_pipeline_write_envelope` still moves.
   - **Cut 6 must refresh** its Keeps citation for
     `validate_pipeline_write_envelope`; the line range no longer matches.
   - **Refusal picture unchanged:** none of the four variants has a reachable
     non-test raiser. The conversion made the existing state literal rather than
     waiver-shaped. Cut 8 owns it.
9. **The mutation suite is runnable and its anchors are exact** (`06100d8e`).
   It documents the interpreter this host actually has, refuses an unsupported
   one up front, and asserts each anchor matches exactly once, since a replace
   hits every match. All ten mutations still die by their own test, including
   the two that mutate code inside the newly gated functions.

**Cut 5 landed** at `1758ad50` (collapse) and `4c091f57` (mutation suite). Soul
is verifying.

- **Deleted:** `TypedCommitStore`, `MIND_COMMIT_STORE`, `validate_mind_writes`,
  the parameter and all five call-site arguments, plus one orphan import. No
  part of the profile shape survived. One commit owner remains, and two receipt
  construction sites: the owner's, and a test that plants one on purpose to pin
  the replay rule.
- **Unchanged behaviour, proven against the base:** the pinned receipt digest
  was checked by running the base test in a detached worktree at `cda36541`,
  not by a value captured from the new code.
- **Tests 171, 0 warnings.** Two profile tests retired because they tested the
  profile through the profile; their rules were re-expressed as three new tests.
- **Five mutations, all killed**, including both halves of S6: uniqueness
  before validation, and validation before replay.

Cut 5 corrections:

10. **Ruling 20's second half had no named test.** The Keeps list named only
    validation-before-replay, so the uniqueness-ordering half of S6 was
    unpinned. Cut 5 added a test for it rather than leaving half the invariant
    on trust.
11. **The Deletes table's line anchors were stale by 26 lines**, written against
    `5fb4eb22`. Matching was done by content. Later cuts should re-anchor before
    trusting a line number in this map.
12. **The subtraction estimate was −46/+8; the actual is +172/−122.** The
    owner-side collapse matched; the overshoot is the test rewrite the ledger
    never priced, because the Keeps list demanded three rules stay pinned while
    the tests that pinned them died with the abstraction. Carrying cost still
    falls: one struct, one const, one fn, one parameter and five arguments gone.

**Soul's Cut 5 pass** confirmed the collapse changed no Mind behaviour: it read
the full owner diff, including the places the pinned digest is blind to
(`committed_at`, companions, conflict re-read, replay lookup), and found nothing
reordered or re-worded. Both renamed tests assert what their names claim, and
byte-identity on refusal is now asserted in three places rather than base's two.

It found four gaps, which a Hands batch is closing:

- **S7 (medium, a regression from base).** Re-deriving the backing store at the
  CAS from the same path survives the suite. The replacement test only asserts
  "one file in this directory", which a re-derivation satisfies; the retired
  profile test had caught it by putting the store somewhere else. It matters
  beyond style, because the planned redb store permits one writable handle per
  path.
- **S8 (low-medium, pre-existing).** Validating only the first write of a batch
  survives, because no test commits a batch of two distinct writes.
- **S9 (low).** The claim of exactly one receipt construction site is false:
  there are two, the owner's and the planted receipt that makes the replay test
  work.
- **S10 (cosmetic).** Profile vocabulary survives in a renamed test's fixture
  data.

**Cut 5's findings are closed** at `617c397d`. Tests 172, 0 warnings; seven
mutations, every one killed by its own test.

- **S7:** a `RuntimeSpineBackingStore` carries only its path, so a same-path
  re-derivation is invisible to any assertion about files, contents or
  receipts. The test now counts resolutions through a `#[cfg(test)]` log and
  requires exactly one per commit, with a comment saying why the file
  assertions below it cannot see this. MS1 dies with `left: 2, right: 1`.
- **S8:** a new test commits two writes with distinct identities, the second
  refused, and requires the store byte-identical, so the valid first write
  lands nowhere. MS2 dies. M2's rule string was corrected: it pins that the
  batch is validated at all, and MS2 owns "every write".
- **S9:** corrected above. Two construction sites, one deliberate.
- **S10:** fixture data renamed; the pinned digest does not depend on it.

**Soul's second pass corrected Self's record.** Self had written that a
mutation reported a verdict it never earned. The committed script already threw
on a missing anchor, the affected mutation existed only in a working copy, and
every committed verdict was real. The Eureka changelog carries the correction.

**Cut 6 landed** at `7ab838f1` (extract), `d17cc441` (docs) and `08c1c9d9`
(mutation suite). The last cut inside Epiphany.

- **`epiphany-pipeline` exists.** Four direct dependencies, 36 transitive
  against `epiphany-core`'s 93, and **zero new packages in the workspace lock** —
  the lock gains exactly one entry, the member itself. `cargo tree` over normal,
  dev and build finds no path back to `epiphany-core`. The leaf is a leaf.
- **`epiphany-core` net −1,034**, and lost a dependency: `schemars` left with the
  code, the module having been its only user in the package.
- **Tests 12 and 163**, both 0 warnings, the arithmetic exactly 172 − 9 moved.
  Twelve mutations, all killed.
- **The move is verbatim in the library code**, confirmed by Soul reading the
  full base-to-head diff.

Cut 6 corrections:

13. **Cut 4's Keeps list said `schemars` stays. It did not.** The pipeline
    documents were its only user, so it left with them. The Q5 reason to keep it
    now lives in `epiphany-pipeline`.
14. **The add-side estimate was never raised.** Correction 3 raised the removal
    figure to −1,060 when the test module joined the move, and left the addition
    figure on its pre-Cut-4 623-line base: spec +700, actual +1,292. The two
    numbers always move together. Second ledger drift after a correction this
    campaign; the first was Cut 5's correction 12.
15. **`rg -n "pipeline" epiphany-core/src` cannot be empty**, and should not be.
    Sixteen hits remain, all inside one `#[cfg(test)]` spine test that Cut 4's
    fix batch put there deliberately. The intent holds — `epiphany-core` has no
    pipeline *surface* — so the negative grep is the narrower one over the type
    names. Hands refused to weaken the test to satisfy a grep, and Soul
    confirmed that was the right call.
16. **Ten schemas changed, not three added.** The five that embed `PipelineKind`
    gained three variants. Ruling 8's amendment settles that as additive.
17. **`epiphany-state.exe` "byte-identical" is withdrawn as false**, not merely
    unverified. It is 94,720 bytes smaller at head, which is what taking ~1,000
    lines of schemars-derived types and the `schemars` dependency out of
    `epiphany-core` should do. No receipt for the original claim exists anywhere.
    Soul also showed the same source hashes three ways from three different
    roots, so the honest replacement is no hash claim at all: that artifact's
    hash is not a comparison this branch can make.
18. **`7ab838f1`'s message says three things changed in the move; the diff
    carries five.** The two omissions are test fixture path literals, mechanical
    and harmless. Self's first relay of this was wrong in its other half: the
    `remove_dir_all` line was introduced by `08c1c9d9`, whose message devotes a
    paragraph to it.

**Cut 6's Soul findings are closed** at `eb55efe9`, `b6f6e802`, `80db5db6` and
`3ee78e05`. Tests 13, 0 warnings; the suite is now M1-M15 and every one is
killed by its own test, including both of Soul's survivors.

- **S1, the escape was not injective.** `GameCult_Epiphany/thing` and
  `GameCult/Epiphany_thing` both keyed to `GameCult_Epiphany_thing`; an instance
  stewarding both would silently have lost one document. The test above it
  claimed the property in a comment and checked one half of the pair. Both
  special bytes are now escaped to two-byte codes starting with `_`, which is
  reversible and therefore injective, and the test keys both halves.
- **S2, the instance key segment was unpinned.** The validation call was in the
  source; nothing would have noticed its removal. Soul's survivor is now M14.
- **S3, `pipeline_key` made `Instance` a root and `pipeline_id` did not**, so an
  instance could be written but never referenced and no resolution could take
  one as its subject. The key writer had been extended for the new kind and the
  id reader left behind.
- **S6, the derivation test's temp directory is per-run**, named by pid and
  nanos. The best-effort clear and its `.ok()` are gone rather than hardened:
  with no directory shared between runs there is nothing to defend. The
  mutation suite had been leaving mutated schemas in the shared one.
- **Recorded, not fixed:** S5, the `ForeignDocument` type-id literal can drift
  from `epiphany-core`'s real one and nothing connects them; the strong fix is a
  dev-dependency that crosses this cut's deletion line. Cut 8 owns it.
- **Informational for Cut 8:** the leaf pulls `redb` and `fs2` through
  `cultcache-rs`, so Huginn gets an embedded database in its graph from this
  leaf whatever else it chooses.

**The test shape was the common cause.** S2 and S3 were both invisible because
the three new kinds' tests asserted key strings and never parsed them back.
`keys_read_back_as_ids_of_their_kind` is the pin that would have caught both,
and it is now in the Keeps list for every kind added after this.

**Cut 6's second fix batch landed** at `9b68d83c` (one composer,
`KeyParts::key`, for every non-root, non-resolution key, with `.` escaped to
`_d`), `4cccb45c` (a resolution reads back by the key it has; `declared_kind`
added) and `4a654351` (the per-run schema directory's cost stated). It closed
Soul's second-pass findings:

- **F-A, the composed hand-off key was still not injective.** S1's fix made the
  repo *segment* injective and left the *composition* open: a dotted
  `to_instance` and a dotted date could trade bytes across the `.` separator.
  Fixed at the composer, not the segment.
- **F-B, a resolution was unreferenceable.** Soul's mutation adding
  `| PipelineKind::Resolution` to the root case survived, because nothing read
  a resolution's key back. Fixed with a Resolution arm in `pipeline_id`.

**Soul's third pass, on that batch, found the same invariant open again**, one
level higher, and Self stopped the patch sequence. Three consecutive fixes had
each made one composition site injective (segment, then composed local, then
root namespace) and left the site above it ambiguous. Per doctrine, escalating
guards mean the ownership is wrong. The three CONFIRMED High findings, all
`epiphany-pipeline/src/lib.rs` at `4a654351`, became Cut 6b:

- **Campaign and instance share one root namespace** (`:693-700`, `:553-556`).
  Campaign `yggdrasil` and instance `yggdrasil` key identically. Soul measured
  254 documents keying to 234 distinct keys, 20 colliding, 10 from this pair.
- **A resolution's key discards its subject's root kind** (`:703-707` with
  `:529-538`). `declared_kind` hardcodes `Campaign` for a bare slug; a mutation
  flipping it to `Instance` survived because the information was already gone.
- **A resolution of a resolution composes `resolution:resolution:…`**
  (`:564-568`), which `pipeline_id` reads back as `InvalidFormat`. The comment
  beside it names the path as live.

Three medium survivors go into the same cut: the composed local's total length
check is unpinned (a hand-off local reaches about 140 bytes against a
documented 64); the root check inside the composer is unpinned (`pipeline_key`
is `pub` and a colon in a root yields four segments); and "a kind's tail arity
is fixed by the arm" is enforced by a doc comment, not the type.

Cut 6 corrections, continued:

19. **`<Org_Repo>` escapes three bytes, not two.** The Keys paragraph below
    said `_` and `/`; `9b68d83c` added `.` → `_d` so that an escaped repo can
    sit inside a `.`-separated local without trading bytes with its neighbours.
    Cut 6b generalises the escape to every slug entering a local and the
    paragraph is superseded there.
20. **The key table never stated the resolution key.** `resolution:<subject id>`
    was the shape since Cut 3a and appeared in no table. Pre-existing; Cut 6b's
    grammar table is the first complete one.
21. **`rg -n "std::process" epiphany-pipeline/src` is not empty**, and should
    not be: the per-run temp directory from S6 reads `std::process::id()` under
    `#[cfg(test)]`. The intent (no process is spawned, no socket is opened)
    holds; the grep is narrowed to `Command|UdpSocket|BackingStore`. Second
    negative grep this campaign to drift from its intent; the first was
    correction 15.
22. **The shared cargo target directory was left at 8,642 paths against an
    8,284 baseline, and that is ruled acceptable for this pass.** The growth is
    one debug profile of this package's dependency tree, with no new package,
    target, profile or platform, and deleting it would evict a cache the next
    build re-creates. This is a judgment per pass, not a loosening of the rule:
    the path list is still recorded and compared every time. (`find | wc -l`
    counts the root directory and PowerShell's `Get-ChildItem -Recurse` does
    not, so 8,642 and 8,641 are the same count.)
23. **Cut 5's Soul addendum carries a second "byte-identical" claim about
    `epiphany-state.exe`.** Its other half is stronger than claimed and stands:
    the negative greps are empty across the whole tree including untracked and
    ignored files. But the binary claim has the shape correction 17 withdrew,
    and that artifact's hash is build-root dependent. Marked suspect, not
    evidence; no hash replaces it.
24. **`schemas/cultnet/README.md:5` promises the folder helps "foreign
    consumers inspect the wire shape", and `:40-44` now says a widened enum is
    additive because "these readers are ours".** Both sentences are true today
    and they will contradict each other the day Eureka has an outside reader.
    Recorded, not resolved: it needs a real answer if a foreign consumer
    appears, and the answer is a versioning policy, not a README edit.

**Mutation harness audit (Eyes, 2026-09-16).** Soul's third pass ran a no-op
control mutation and it "killed" `bounds_refuse_in_utf8_bytes`: the harness's
text round-trip had collapsed the test's `é` literal to one byte, so every
mutation through that path would have reported a kill whatever it changed.
Soul caught it only because it ran the control. Eyes then audited every suite
in this repo and in Ghostlight:

- **No committed suite has the defect.** `tools/eureka-cut4-mutations.ps1` (at
  `a9f06c2a`), `cut5` and `cut6` all use `[IO.File]::ReadAllText` /
  `WriteAllText`, measured byte-identical on every live target. The corrupting
  harness was Soul's own inline one and is not on disk anywhere; its text is
  unrecoverable. That is the shape finding S9 under History already names as
  Standing: a mutation with no artifact on disk is unverifiable.
- **Cut 1's M1-M3 are suspect for a different reason.** The ad-hoc
  `eureka-c1-mutate.ps1` joined lines with `\n`, rewriting two CRLF files to LF
  for the whole file while mutated; `core.autocrlf=true` hid the rewrite from
  `git diff`. Whether that changed a verdict is unknown and was not re-run.
  Cuts 3a, 4, 5, 6, the Cut 2 fix pass and all Ghostlight verdicts are not
  suspect.
- **No suite in either repo has a no-op control**, and the committed suites
  structurally forbid one: each throws when a replacement changes nothing
  (`cut4:86`, `cut5:216`, `cut6:237`). Ghostlight has never had a scripted
  harness; its mutations are hand edits restored by `git checkout`.
- **The measured rule** is encoding symmetry and end-of-line preservation, not
  `-Encoding utf8` by itself: mixed-encoding round-trips corrupt the `é`, and
  any non-`-Raw` `Get-Content` corrupts line endings regardless of encoding.

Cut 6b's suite is the first with a control (M0) and the rule is now in the
Eureka skill.

**Cut 6b landed** at `602ffd9f` (the three key-shape exceptions deleted;
does not build, by design), `1bddd2ac` (one grammar for every kind; 20 tests,
0 warnings) and `95ee551a` (`tools/eureka-cut6b-mutations.ps1`, the first
suite with a no-op control). Source outside tests 774 → 726, tests 734 → 881,
no schema, dependency, kind, field or epoch moved. Hands' spec discrepancies,
all kept: `local` takes `&[&str]` rather than a fixed array because the
Resolution and Finding arms carry a parent's parts; the whole-local bound
refuses `InvalidFormat` to match the reader; `key_segment` is a pure escape
and each caller validates its own field first; two "unchanged" tests carried
literal keys that moved; and `{CAMPAIGN}:resolution:R8` is now asserted
*accepted* with the Q11 rationale in its doc comment, so a future runtime
guard is visible.

**Soul's first pass on the grammar** (Fable, script
`soul-cut6b-mutations.ps1` in the session scratchpad, M0 green twice) built a
whole-key injectivity probe: 212,450 distinct keys across all thirteen kinds
with adversarial slugs, repos, labels and dates and resolutions nested to
depth four, **zero collisions**, every key three segments, every key reading
back to its own kind and identity, 157,010 resolution keys recovering exactly
their subject. `self` collides with nothing as a slug, a label or a campaign.
Hands' M1-M6 all killed on rerun; Soul's own S1, S6, S7, S8, S9 killed; S3
(`splitn(3, ':')`) survived as an equivalent mutant because `label_text`
refuses a colon in the third segment anyway. Three survived for real:

- **F2 (medium): the escape's `_`→`__` half is pinned only through an
  `OrgRepo`.** A mutant escaping only `.` when the value has no `/` survives,
  and hand-offs to `a_db` and to `a.b` then share a key. That is the pass-2
  collision class, one edit from reopening.
- **F3 (medium-low): the composer's root check is pinned for one byte.** Under
  `trim_end_matches('.')`, campaign slug `eureka-state.` composes a key the
  reader refuses, and the write validator recomputes and compares without
  reading back, so a document is admitted under an id no `PipelineRef` can
  name.
- **F4 (medium-low): the 64-byte boundary is unpinned on both sides.**
  Fixtures sit at 55 and 66+, so a `> 65` mutant survives and the writer
  admits a 65-byte local the reader refuses. The code is right; the pin was
  missing.

And four findings that are not code defects: **F1**, the "depth at most five"
claim is false (corrected above); **F5**, a dotted finding label now refuses as
`finding.key` rather than `finding.label`, consistent with every other kind
and accepted as the behaviour; **F6**, the authority map overstated the escape
rule for `Date` (corrected above); **F7**, Hands' "twelve fixed call sites" is
eleven literal slices plus two computed vectors, the Finding one fixed in
practice by `parent_cut` pinning the verdict local to two parts. **F8**,
pre-existing and informational: a cut-report key drops the spec revision
(a standing ruling) and a verdict key drops the report attempt (implied, not
stated). `pipeline_id` accepting `c:campaign:notself` and the like is grammar
versus reachability, admission's job, as the spec says.

The stale Cut 6 script entries M5 and M12-M15 match zero anchors at HEAD and
the script throws at its line 234 rather than reporting a kill. Loud, as the
rule requires. Target dir: Hands' growth to 9,279 was one debug profile;
Soul left 8,991 because cargo rotated out one of Hands' incremental sessions,
which cannot be recreated and is not a deletion.

**The first fix batch landed** at `4b85dd2d` (F1-F4 pinned; Soul's S2, S4 and
S5 now die; the depth test computes its expectation from the formula) and
`d03a32df` (one harness, `tools/eureka-mutations.ps1`, with per-cut entries
files `eureka-cut5-`, `-cut6-`, `-cut6b-mutations.psd1`; the three old scripts
deleted; `tools/` net −24 lines). The harness runs each entry with
`--exact <test>` and reports `TEST NOT RUN` for a name that runs nothing,
where the old scripts would have said SURVIVED. Cut 6's stale entries M5 and
M12-M15 were deleted with notes, on Self's instruction, which was a mistake;
see F3 below.

**Soul's second pass** (Fable; scripts `soul-cut6b-fix-*.ps1` in the session
scratchpad) attacked the harness directly and reran every entry with the
whole suite: no verdict was borrowed from a collateral kill. Held: S2/S4/S5
die, the four pins are as promised, all three suites report the same verdicts
through the new harness, refusal `value` is the failing part and asserting
`field` alone cannot hide a wrong-reason refusal. Found:

- **F1 (medium): the control could damage what it protects.** M0 wrote the
  target before comparing, having read it as text, so on a broken harness or
  a BOM-bearing target it reported "harness broken" and left the file
  changed with no restore source but the checkout the rules forbid.
- **F2 (low): "exactly once" counted non-overlapping matches**, so a
  self-overlapping anchor like `}\n}\n` in a run of three braces passed as
  one.
- **F3 (low-medium): the deletion notes for Cut 6's M12/M13 named the wrong
  pinning entry.** `key_segment` still exists and nothing in the committed
  suite removes its escape or breaks its injectivity; the tests do pin the
  rule (Soul's N7/N8 die), the suite claimed coverage it lacked. Self's brief
  ordered "delete, do not re-anchor"; the coherent fix was to re-anchor.
- **F4 (medium): dotted roots were unpinned on writer and reader.** No test
  used a campaign or instance slug with a dot, so `label_text` on the root in
  either `pipeline_key` or `pipeline_id` survived the whole suite.
- **F5 (low): the reader's kind check was unpinned for root kinds.** A mutant
  excusing `Instance` from the kind-segment comparison survived: exactly the
  "reader excuses a root" shape Cut 6b existed to kill.
- **F6 (low): the depth test restated the literal 64** rather than
  referencing the bound, so moving both to 60 passed it; the bound itself is
  pinned by F4's fixtures.
- **F7 (recorded): `-Test` splits on whitespace**, so a path with spaces
  cannot be expressed. Not reachable on this host.

**The second fix batch landed** at `9370aa0f` (harness: the original is read
as bytes and is the restore source on every path; M0 compares the re-encoded
bytes to the original before writing anything, then still writes through and
hashes so a broken write path is caught, restoring the original bytes before
any throw; anchor occurrences are counted overlapping; the offset reverse-edit
path is deleted; a BOM-bearing target is now tolerated rather than refused)
and `f00062db` (`dotted_roots_key_and_read_back`;
`keys_read_back_as_ids_of_their_kind` extended by three root-kind asserts, its
first change since Cut 6; `const LOCAL_MAX: usize = 64` shared by `local` and
the depth test; entries N3, N6, N7, N8, N9, N10 in the 6b file, M4 and S5
re-anchored on the constant; the Cut 6 notes corrected to M5→N3+M6, M12→N7,
M13→N8, M15→N6+M1). Tests 21, 0 warnings. Soul's attack script rerun against
the fixed harness: every variant leaves every target byte-identical and runs
no entry.

**Soul's third pass** (Fable; `soul-cut6b-fix2-*` in the session scratchpad)
reran all three suites through the committed harness (every entry killed, M0
green on every target, 0 warnings unmutated) and **closed the grammar
pins**: the dotted-roots test asserts exact tuples, the root-kind asserts are
appended to the read-back test with nothing weakened, a BOM-bearing target
round-trips byte-identically and rustc accepts a leading BOM, and its own
non-revert mutations died or were equivalent (swapped escape codes remain
injective; `chars().count()` equals `len()` for ASCII parts). What survived
is the harness under failure, not the grammar:

- **The multi-target write loop sits outside the `try` whose `finally`
  restores**, so a write that throws on the second target leaves the first
  mutated with no message. Low for today's single-target entries, medium
  against the harness's stated contract.
- **M0's write-through has no `finally`**: a write that dies after
  truncation leaves a truncated target, and only the hash-mismatch branch
  restores.
- **A killed process runs no `finally`**, and the harness has no timeout of
  its own, so a tool-enforced timeout is exactly the path that leaves the
  tree mutated. The next M0 would catch it without saying why.
- **The reader accepts a trailing dot on the root and on the local**
  (`pipeline_id`, `:556-557`); only the writer refuses, so a `PipelineRef`
  with trailing junk passes the reader against its own doc comment. One
  fixture closes it.
- `LOCAL_MAX` names the writer's bound; the reader's is `dotted_text`'s
  literal 64, which is also the `Slug` bound. Redundant, not split: the
  writer refuses first. A comment at `:651` still says "the 64-byte bound".
- Hands' line counts were +6/−1 and +73/−6, not +5/−1 and +68/−6.

**The third fix batch landed** at `9d57a460` (harness: every target write
inside the restoring `try`; M0's write-through in its own `try`/`finally`; a
shared restore that is hash-first, attempts every target, and prints
`RESTORE FAILED` with the original SHA-256 before rethrowing; a sidecar
`<target>.eureka-mutation-original` written before any write and removed
after a verified restore, from which a run that died mid-mutation is
repaired at the next start with a message; `-TimeoutSeconds`, default 1800,
killing the process tree inside the command runner's own `finally` with the
verdict `TIMED OUT (no verdict)`) and `7cd1a38b` (the reader refuses
`c.:target:x`, `c:target:x.`, `c:target:.x` and `c:target:a..b`; X1 and X11
are entries). Hands reproduced every attack: locked targets on either side
leave both unchanged; a tree killed mid-entry leaves the mutant and the
sidecar, and the rerun repairs and says so; a one-second timeout kills the
runner and leaves the target unchanged. Soul in flight on this batch
together with Cut 6c.

## Probes and source reads this pass

No cargo build ran this pass. Every new mechanism claim below was settled by a
source read; the one build-dependent claim (rmcp as a stdio server) was settled
by P4 in the first pass and is unchanged. No build outputs were created, so
none were deleted. Claims are marked **(source read)** or **(probe)**.

| # | Claim | Evidence |
|---|---|---|
| R1 | **No Rust Qdrant client exists anywhere under `F:\Projects`.** | (source read) `grep -rn qdrant --include=Cargo.toml --include=Cargo.lock --include=*.rs` over `F:\Projects` returns nothing in any live tree. |
| R2 | **Epiphany had one, and deleted it.** `856648de` "Delete unused semantic projection subsystem" removed `semantic_backend.rs` (1,018 lines), "Typed boundary around the Qdrant and Ollama HTTP APIs", plus 8,800 more lines of projector. | (source read) `git show 856648de --stat`; `git show 856648de^:epiphany-core/src/semantic_backend.rs`. |
| R3 | **That client was plain `reqwest::blocking` against Qdrant's REST API**, not a Qdrant crate: `PUT/GET/DELETE {base}/collections/{name}`, `PUT {base}/collections/{name}/points`, `POST .../points/query`, `.../points/scroll`, `.../points/delete`. Embeddings were `POST {base}/api/embed` returning `.embeddings`. | (source read) `856648de^:.../semantic_backend.rs:116,130,157,207,244,262,287,318,351,397,535,560`; deps at `856648de^:epiphany-core/Cargo.toml:37` (`reqwest = { version = "0.12", features = ["blocking","json"] }`). |
| R4 | **Epiphany has no CultNet *server* for documents.** Its live `cultnet_rs::` usage is service identity and trust anchors only. Its only UDP binds are the Persona Discord permit issuer, the Persona delivery client and Atlas publication. | (source read) `grep cultnet_rs::` over `epiphany-core/src` yields only `ServiceIdentitySigner`, `GameCultServiceTrustAnchorRecord`, `open/enroll_service_identity_at`, `derive_service_identity_id`. `UdpSocket::bind` appears only at `atlas/transport.rs:245`, `bin/epiphany-persona-discord-permit.rs:38`, `persona_discord_crossing.rs:322`. |
| R5 | **The permit issuer is a hand-rolled request/response loop**, not a reusable document server: `serve_persona_discord_permit_rudp` loops `transport.receive_once()`, matches one `DocumentPutRaw`, and replies with another. | (source read) `persona_discord_permit.rs:330-400`. |
| R6 | **Odin is the real harness to copy.** `odin-daemon` runs `CultMeshRudpDocumentServer::new(socket, SinkHandle, SnapshotHandle, CultMeshSystemClock, options)` and a `poll_once` loop, with Idunn activation, a process write lease, signal handling and presence-health publication. | (source read) `Odin/crates/odin-daemon/src/main.rs:14-40,542-560,600-700`. |
| R7 | **CultMesh's document server is port-shaped and mockable.** `CultMeshRudpRawDocumentSink::accept_raw_document(receipt)` and `CultMeshRudpSnapshotSource::raw_snapshot(&query)` are traits with blanket impls for closures; the clock is a trait. | (source read) `CultLib/packages/cultmesh-rs/src/rudp_document_server.rs:51-95`. |
| R8 | **A lean CultNet daemon needs six dependencies.** `odin-daemon` is `signal-hook, anyhow, chrono, cultcache-rs, cultmesh-rs, cultnet-rs, fs2, rmp-serde, serde`, and Odin's whole lock file is 156 packages. Epiphany's is 295. | (source read) `Odin/crates/odin-daemon/Cargo.toml`; `grep -c '^\[\[package\]\]' Cargo.lock` in both repos. |
| R9 | **`epiphany-core` is 40,941 lines across 37 modules with 39 `mod` declarations**, and pulls Ghostlight, `ed25519-dalek`, `ignore`, `semver`, `cultmesh-rs`, `cultnet-rs` and `windows-sys`. | (source read) `wc -l epiphany-core/src/*.rs`; `epiphany-core/Cargo.toml`. |
| R10 | **Epiphany's commit owner and its profile are crate-private**, so no external crate can reuse admission as it stands. | (source read) `reasoning_context.rs:1590` `pub(crate) struct TypedCommitStore`, `:1606` `pub(crate) const MIND_COMMIT_STORE`, `:1613` `pub(crate) fn commit_authorized_mind_mutation`. |
| R11 | **`RedbMessagePackBackingStore` stores one redb row per `(type, key)`**, transactionally, and creates parent directories. redb permits one writable handle per path, and the CultCache lock owns the open/transaction/close interval. | (source read) `cultcache-rs/src/lib.rs:960-1013`, `:979-982`. |
| R12 | **Nothing consumes `@gamecult/huginn`.** The only `package.json` naming it is Huginn's own. Eve's Huginn entry points at a checked-in fixture file, not the package. | (source read) Grep over every `package.json` under `F:\Projects`; `Eve/web/local-provider-catalog.json:72-83` gives `"url": "./fixtures/huginn-cc-surface.eve"`. |
| R13 | **Eve's Huginn fixture is static and already stale.** It describes `E:\Projects\CultCacheTS\.voidbot\state\huginn.cc` and declares `"freshness": {"state": "fixture"}`, `"splitTarget": "Huginn"`. | (source read) `Eve/web/fixtures/huginn-cc-surface.eve:16`; `huginn-cc-surface.conformance.json`. |
| R14 | **CultCache Studio exists** as a Unity editor surface in CultLib. | (source read) `CultLib/src/GameCult.Unity/Assets/Caching/Editor/CultCacheStudioWindow.cs`, `CultCacheStudioDrawers.cs`. |
| R15 | **Qdrant on Yggdrasil is voidbot's container**, `qdrant/qdrant:v1.17.1`, host network, bound `127.0.0.1`, storage `/srv/voidbot/qdrant`, started by `voidbot-retrieval.service`. | (source read) `gamecult-ops/compose/voidbot-retrieval.yggdrasil.yaml`; `systemd/voidbot-retrieval.service`. |
| R16 | **Two Ollama endpoints exist, and the Yggdrasil precedent is the local one.** `epiphany.service` embeds against `http://10.77.0.1:11435` with `qwen3-embedding:0.6b`; voidbot's indexer uses Nightwing `10.77.0.3:11434` with the same model. | (source read) `gamecult-ops/systemd/epiphany.service:14-15`; `runbooks/yggdrasil-replacement-2026-07.md:184,193`; `runbooks/voidbot-retrieval-recovery-yggdrasil.md:42-62`. |
| R17 | **Idunn v2 is recipe-plus-binding.** A repo publishes `deployment/idunn/recipe.toml` (`gamecult.idunn.target_declaration.v1`: steps, artifacts, `[service]`, `[state.slots]`, `[[provides]]`, `[[dependencies]]`); Yggdrasil admits a paired `gamecult.idunn.operator_binding.v2` naming runners, workload roots, route, brakes, rollout and placement. | (source read) `Odin/deployment/idunn/recipe.toml`; `Ghostlight/deployment/idunn/recipe.toml:168-187`; `gamecult-ops/idunn/yggdrasil/bindings/odin.toml.in`, `bindings/README.md`. |
| R18 | **Idunn brakes are typed and already separated.** `idunn.deployment_brake.v1` (scope `deployment`) and `idunn.lifecycle_brake.v1` (scope `continuity-restart`) are distinct records with distinct authorities. | (source read) `cultnet-rs/src/idunn_deployment_brake.rs:9-15`; `idunn_lifecycle_brake.rs:4-7`. |
| R19 | **The authority backup is an explicit path list, daily at 03:20 UTC.** It tars a fixed set including `var/lib/gamecult/epiphany` and `srv/voidbot/state`; a path not listed is not backed up. | (source read) `gamecult-ops/scripts/backup-gamecult-authority-yggdrasil.sh:80-105`; `systemd/gamecult-authority-backup.timer`. |
| R20 | **The workstation reaches Yggdrasil by a supervised SSH tunnel with a fixed forward table**, scheduled task `GameCult-Yggdrasil-Tunnel`. It already forwards `17875` (voidbot MCP) and `16333/16334` (Qdrant). | (source read) `gamecult-ops/scripts/start-yggdrasil-tunnel.ps1:13-26`; `runbooks/yggdrasil-ssh-tunnel.md:86-88`. |
| R21 | **Allocated `178xx` RUDP/service ports** are 17870 Idunn health, 17871 Odin rendezvous, 17873 VoidBot swarm publisher, 17874 Hermodr, 17875 voidbot MCP, 17876 Epiphany permit listener, 17877 Starfire permit requester, 17878 dings. **17872 and 17879 are unallocated.** | (source read) `gamecult-ops/inventory.md:247,455,502,537`; `runbooks/yggdrasil-replacement-2026-07.md:315-321`; `idunn/yggdrasil/bindings/*.in`. |

Carried forward from the first pass and still load-bearing:

- **P4 (probe).** rmcp 2.2.0 with `features = ["server","macros","transport-io"]` serves stdio JSON-RPC, handshakes at `2025-06-18`, emits both `inputSchema` and `outputSchema` from schemars, returns `structuredContent`, maps `ErrorData::invalid_params` to `-32602`, and spawns nothing.
- **P7 (probe).** cultcache-ts `inspectCultCacheBytes` decodes a Rust-written `cultcache.store.v1` record; Rust writes an empty member catalog, so the TS decode is generic.

Superseded probes: P5, P6, P8, P9, P10 and P11 all measured git layout, store locking at session scope, or git attribute classification. They were evidence for a repo-committed store and no longer bear on any live design. P1-P3 were Cut 1 and landed.

## D1. Package boundaries and where admission lives

This is the decision the brief asks for, so it is stated first.

**Ruling 1 says Epiphany owns the schemas. The target's End state assigns
admission, queries and the index to the organ** ("**The organ (Huginn).** One
service owns every mind it hosts: admission, with per-document rules and typed
refusals; ... queries ...; semantic search ...; a typed hand-off"). Those are
consistent, and together they settle the split:

- **Epiphany owns the document types, their keys, their bounds and the published
  JSON schemas.**
- **Huginn owns admission rules, receipts, storage, queries, the index, the
  hand-off and the CultNet surface.**

**Admission does not live in `epiphany-core`, and Huginn does not depend on it.**
Three source-grounded reasons:

1. **It cannot.** The commit owner, its profile and the wrappers are all
   `pub(crate)` (R10). Exposing them would publish Epiphany's Mind commit
   machinery as a public API to make an unrelated service compile.
2. **The weight is absurd.** Huginn would compile 40,941 lines and 295 lock
   packages, including Ghostlight and `ed25519-dalek`, to use about 2,000 lines
   of it (R9). Odin's comparable daemon costs 156 packages total (R8). This is
   exactly the build fan-out AGENTS.md's Source And Build Economy forbids.
3. **The rules are not the same rules.** Epiphany's admission is bound to a
   scheduler: launch requests, sealed reasoning bases, decision contexts.
   Pipeline admission is bound to an instance and a campaign. Sharing the
   function would mean sharing none of the interesting part.

**So a third package owns the shared half.** New leaf library
`epiphany-pipeline`, in the Epiphany repo, workspace member, `autobins = false`:

| | |
|---|---|
| **Owner** | The ten document kinds plus the three new ones, their value types, bound aliases, format rules, key derivation, and the derived JSON schemas. |
| **Dependencies** | `cultcache-rs`, `schemars`, `serde`, `rmp-serde`, `chrono`, `anyhow`. Nothing else. |
| **Consumers** | Huginn's `huginn-mind` (admission) and `eureka-state` (typed tool schemas), both by git rev. Epiphany itself does **not** consume it in this campaign; that is campaign two. |
| **Why a new crate** | A live cross-repo consumer needs these types without Epiphany's 295-package graph. `F:\Projects\CLAUDE.md` says authority separation alone does not justify a crate; this is not authority separation, it is a named external consumer and a hard dependency boundary. |
| **Why in the Epiphany repo** | Ruling 1. The schema publication path, `schemas/cultnet/index.json` and the derivation test stay where they are. |

`epiphany-core` keeps the schema-derivation test's *outputs* — the committed
`schemas/cultnet/epiphany.pipeline.*.v1.schema.json` files are Epiphany's
publication artifact — but the test that derives and compares them moves into
`epiphany-pipeline`, reading `../schemas/cultnet` the way it already does
(`pipeline_store.rs:1173`).

**Huginn's package boundary**, applying AGENTS.md's one-package-per-production-
entrypoint rule. Two entrypoints, so two binary-owning packages plus one library:

| Package | Kind | Owns |
|---|---|---|
| `huginn-mind` | library | Mind storage, admission rules, receipts, queries, derivations, the index port and the embedding port. No process, no socket. |
| `huginn-daemon` | binary `huginn-daemon` | The CultNet surface, the Idunn lifecycle, Qdrant and Ollama adapters, the serve loop. |
| `eureka-state` | binary `eureka-state` | The stdio MCP client. |

**The daemon test (`F:\Projects\CLAUDE.md`).** `huginn-daemon` earns its process:
it outlives every Claude Code session, owns an independent resource dependency
(Qdrant, a redb store), and isolates a dead session from a corrupted mind. That
is lifecycle, resource and failure isolation, and it protects the named
invariant "exactly one writer per mind". `eureka-state` earns a *separate*
entrypoint but is **not** a daemon: it is a per-session stdio child with a
different dependency set (rmcp, tokio) and a different lifecycle. `huginn-mind`
earns no process at all.

**Why `eureka-state` lives in Huginn, not Epiphany.** The client and the server
share the request and response document types. Putting the client in Epiphany
would make the wire contract have two owners in two repos. It also keeps rmcp
and tokio out of Epiphany's release bundle, which was the original Cut 4's
reason for a separate package anyway (`construction.rs:82-98` lists the nine
packaged binaries; none changes).

## D2. Documents

Unchanged from the landed Cut 3a except as noted: the value-wrapper pattern, the
`value_types!` single field list, the bound aliases (`Short` 200, `Line` 1,000,
`Para` 4,000 UTF-8 bytes), the format types (`Label`, `Slug`, `OrgRepo`, `Sha`,
`FullSha`, `Sha256Hex`, `Date`), key derivation, and the ten kinds all survive
verbatim. The epoch stays `epiphany.pipeline.epoch.v1`. Adding the three kinds
widens the `PipelineKind` enum in the five schemas that embed it, which ruling
8's 2026-09-16 amendment settles as additive.

**Deleted from the set:** `PipelineWriterHolder` and its wrapper
`EpiphanyPipelineWriterHolder` (`pipeline_documents.rs:334-349,492-497`). They
were the display record of the per-clone lease.

**Added: three kinds, each with a live consumer.**

| Kind | Type id | Value fields | Live consumer |
|---|---|---|---|
| `instance` | `epiphany.pipeline.instance.v1` | `instance: Slug`, `display_name: Short`, `created_at: Date`, `host: Short` | The mind's identity document. Admission's identity check (ruling 14); `whoami` in `eureka-state`; the hand-off's `from`/`to`. |
| `stewardship` | `epiphany.pipeline.stewardship.v1` | `instance: Slug`, `repo: OrgRepo`, `assigned_on: Date`, `note: Line` | Ruling 14's "stewardship is an assignment". Query "which repos does this instance steward"; the Rehydrate brief; admission's campaign-repo check. |
| `hand_off` | `epiphany.pipeline.hand_off.v1` | `from_instance: Slug`, `to_instance: Slug`, `repo: OrgRepo`, `documents: Vec<Short>[256]`, `reason: Para`, `handed_on: Date` | Ruling 14's "reassignment is an explicit typed hand-off recorded in both minds". Cut 12's import path. |

**Keys.** *Superseded by Cut 6b's grammar (correction 19, 20). Kept as the
shape Cut 6 landed; the live table is in Cut 6b.*

| Kind | Key as landed by Cut 6 | Key after Cut 6b |
|---|---|---|
| `instance` | `<instance slug>` | `<instance>:instance:self` |
| `stewardship` | `<instance>:stewardship:<Org_Repo>` | unchanged |
| `hand_off` | `<from>:hand_off:<to>.<repo>.<date>` | `<from>:hand_off:<to escaped>.<repo>.<date>` |
| campaign | `<slug>` | `<campaign>:campaign:self` |
| resolution | `resolution:<subject id>` (never stated here before) | `<subject root>:resolution:<subject kind>.<subject local>` |
| everything else | `<campaign>:<kind>:<local>` | unchanged |

`<Org_Repo>` is the `OrgRepo` with `_`, `/` and `.` escaped: `_` → `__`,
`/` → `_-`, `.` → `_d` (the third added at `9b68d83c`, correction 19). `/` is
not a `Label` byte, `.` is the local separator, and the key must segment
unambiguously. Every other byte passes through and is never `_`, so a reader
going left to right takes each `_` with the byte after it and never has a
choice: the encoding is reversible, therefore injective. Admission recomputes
it and refuses a mismatch with the existing `InvalidIdentity`.

**Corrected 2026-09-16, after Soul.** This read "`/` replaced by `_`", which is
not injective: `GameCult_Epiphany/thing` and `GameCult/Epiphany_thing` are both
well-formed `OrgRepo` and both keyed to `GameCult_Epiphany_thing`. An instance
stewarding both would have silently lost one document. The doubling trick alone
(`_`→`__`, `/`→`_`) does not fix it either — `a_/b` and `a/_b` both give
`a___b`. Sample keys move accordingly: `…:stewardship:GameCult_-Epiphany`.

**Resolution matrix additions.** `stewardship` resolves by
`Superseded{by: stewardship}` or `Withdrawn`. `instance` and `hand_off` are not
resolvable. Everything else is unchanged.

**`PipelineRefusal` splits.** The landed enum (`pipeline_store.rs:41-55`) mixes
document refusals with store, git and lease refusals. It becomes two:

- `epiphany-pipeline` keeps the document half: `FieldBound`, `InvalidFormat`,
  `InvalidIdentity`.
- `huginn-mind` owns the service half: `MissingIdentity`, `ForeignEpoch`,
  `ForeignStore`, plus every admission-rule refusal (`MissingReference`,
  `WrongReferenceKind`, `IdentityCollision`, `AlreadyResolved`,
  `IncompatibleResolution`, `CitesResolvedDocument`, `RevisionWithoutSupersession`,
  `InvalidOptions`, `InvalidChoice`, `RepoNotInCampaign`, `DuplicateLabel`,
  `CutReportWithoutSpec`, `RangeOutsideCommits`, `FalsifiedClaimWithoutConfirmedFinding`,
  `UnprovenClaimWithConfirmedFinding`, `FindingWithoutRange`, `FindingWithoutEvidence`,
  `UnknownInvariant`), plus `ForeignInstance { declared, mind }` (ruling 14) and
  `Unavailable`.

**Deleted refusals:** `NotRepoRoot`, `NoMainWorkTree`, `StoreNotMarkedBinary`,
`LockNotIgnored`, `WrongBranch`, `WriterLeaseHeld`. Every one of them is a
statement about a repo-resident store.

## D3. The organ's storage and admission

**One mind per instance, one redb store per mind.**
`<state_root>/minds/<instance>/mind.cc` as a `RedbMessagePackBackingStore`
(R11). Redb, not the single-file store, because a mind grows without bound and
the single-file backing store rewrites the whole snapshot on every write; that
is the monolithic-store footgun `~/.claude/CLAUDE.md` names under
Infrastructure. Odin uses the single-file store, but its topology store is small
and bounded.

**Identity.** The first write to a mind carries both the
`EpiphanyPipelineIdentity` (epoch) and the `instance` document, in one batch.
The opener refuses records-without-identity (`MissingIdentity`), a foreign epoch
(`ForeignEpoch`) and any unregistered type (`ForeignStore`), exactly as the
landed `pipeline_cache` does (`pipeline_store.rs:87-120`) — that logic moves
almost verbatim; only its backing store changes.

**Admission.** One public entry:

```
admit(&mut Mind, PipelineAdmissionBatch) -> PipelineAdmissionOutcome
```

with `PipelineAdmissionBatch { instance, provenance, documents (1..64) }` and the
outcome one of `Committed { receipt_id, committed_at, writes }`,
`AlreadyAdmitted { receipt_id }`, `Refused(PipelineRefusal)` or
`Conflict { identities }`.

Steps, in order:

1. **Identity check (ruling 14).** `batch.instance` must equal the mind's
   `instance` document, or `ForeignInstance`. This is the check that replaces
   the whole lease.
2. Validate bounds and formats.
3. Recompute keys.
4. Check references against the store image plus the batch.
5. Apply the per-kind rules (unchanged from the old D4, minus the campaign
   `origin` check, which no longer has a repo to inspect — `repo` must instead
   be one the instance stewards, giving `RepoNotInCampaign` a new source).
6. Derive writes (the `Answered` resolution for a ruling that answers a
   question; the paired `stewardship` records on a `hand_off`).
7. Commit.

**Receipts stay, and Huginn owns its own commit primitive.** `huginn-mind`
defines `HuginnCommitReceipt` with its own type id
`huginn.mind_commit_receipt.v1`, carrying the same shape that earned its keep in
Epiphany: authority, invariant owner, strong reads, writes, a content-digest
receipt id, and `committed_at`. It replays idempotently on an exact match and
returns typed `Conflict` on a lost CAS.

This is deliberately **not** a reuse of `EpiphanyMindCommitReceipt`. The landed
Cut 3a reused it with `store_id = "epiphany-pipeline"` because both lived in one
crate; across a repo boundary that reuse would drag the whole crate (D1).
Roughly 120 lines of receipt/replay/CAS logic are re-implemented. That is a real
duplication and it is the honest price of the service boundary — it is named
here, in the subtraction ledger, and as follow-up FU-3, rather than hidden.

**Admission time** lives on the receipt, never in a document, so exact replay
stays byte-identical. Unchanged.

**Concurrency.** The organ is the only writer to any mind it hosts, and it is
single-process. CAS stays as defence in depth against its own bugs and against
the import path. The old session lease has no successor: there is nothing left
for it to exclude.

## D4. Queries

*Superseded by Cut 9 as landed (`49cc6e3`), 2026-09-16. Still true: one
owner for every derivation, status derived at read time, the open-items
sentence. Now false: `get` is `view`; `query` returns a
`PipelineQueryPage { items, matched }`, not a `Vec`; `rulings_in_force` and
`stewardship` were never built and are query presets; the view is
`{ id, document, admission, status }`, not the seven-field shape below;
`PipelineQuery` has no `status`, `outcome`, `text_contains` or `instance`
and has `in_force: Option<bool>`; `semantic` is refused typed until Cut 11;
"in force when no resolution names it" is the non-recursive form, and the
landed rule is "no resolution that is itself in force" (Q17 B); `history`
exists and is missing here. The Cut 9 section is the owner.*

`huginn-mind` owns every derivation; no client re-derives status.

- `get(id) -> Option<PipelineDocumentView>`
- `query(&PipelineQuery) -> Vec<PipelineDocumentView>`
- `open_items(campaign) -> PipelineOpenItems`
- `rulings_in_force(Option<campaign>) -> Vec<PipelineDocumentView>`
- `stewardship(instance) -> Vec<OrgRepo>`

`PipelineDocumentView` is
`{ id, kind, document, resolution: Option<(id, PipelineResolution)>, receipt_id, admitted_at, faculty }`,
its admission fields joined from the receipts naming the document.

`PipelineQuery` keeps its landed field set — `campaign`, `repo`, `cut`, `kinds`,
`status`, `outcome`, `faculty`, `admitted_after/before`, `text_contains`,
`limit` ≤ 200 — and gains `instance` and `semantic: Option<{ text, top_k }>`.
When `semantic` is set the query runs through the index (D5) and the hit ids are
resolved back through the typed store, so the store stays the truth.

Derived state, never stored, unchanged from the landed design: a document is
**in force** when no resolution names it; **open items** are unresolved
questions, findings and follow-ups, in-force cut specs with no report, and cut
reports with no verdict.

## D5. The index

**Owner:** `huginn-daemon`, through two ports defined in `huginn-mind` so the
library stays testable without either service:

```
trait EmbeddingPort { fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>; }
trait IndexPort { fn upsert(&self, points: Vec<IndexPoint>) -> Result<()>;
                  fn search(&self, vector: Vec<f32>, top_k: u32, filter: IndexFilter) -> Result<Vec<IndexHit>>;
                  fn delete(&self, ids: Vec<String>) -> Result<()>; }
```

**Adapters.** `QdrantIndex` and `OllamaEmbedding`, both plain `reqwest::blocking`
against the REST endpoints R3 establishes, ported in shape from the deleted
`semantic_backend.rs`. That module is the precedent: it confined JSON to one
xenos-facing boundary and treated Qdrant as "a rebuildable projection rather
than canonical authority", which is exactly this design's relationship to it.

**Collections.** `huginn_pipeline_documents`, one point per document, vector
size 1,024 (`qwen3-embedding:0.6b`, R16). Payload `{ instance, campaign, repo,
kind, doc_id, admitted_at }`, with payload indexes on `instance`, `campaign`,
`repo` and `kind`. Receipts, identity and provenance are never indexed.
In-force status is never indexed; clients resolve it through `get`.

**Indexing is at admission, in the same call, but not in the same transaction.**
The typed commit lands first; the index upsert follows. If the upsert fails the
admission still succeeded, and the organ records the document id in a
`pending_index` slot and retries on its next poll. The index is a projection, so
a stale index is a degraded read, never a lost write. The reverse order would
let an index failure reject an admitted document.

**Rebuild.** `huginn-daemon --reindex <instance>` drops and rebuilds the
collection from the typed store. This is the affordance that makes the
projection disposable, and it is the negative proof that the index is not truth.

## D6. The CultNet surface

*Partly superseded by Cut 9 and Q13 A, 2026-09-16: the `RulingsInForce`
and `Stewardship` operations are presets of `Query`; there is no `History`
operation, which Q17 B's obligation requires; the wire types are
`huginn-mind`'s, carried by the client for types only. The Cut 10 refresh
is the owner of the surface's shape.*

**Copy Odin's harness (R6, R7).** `huginn-daemon` binds one loopback UDP socket
from `GAMECULT_IDUNN_CANDIDATE_BIND`, constructs
`CultMeshRudpDocumentServer::new(socket, sink, snapshot, CultMeshSystemClock::default(), options)`,
and runs a `poll_once` loop with `signal-hook` handling SIGTERM/SIGINT.

Requests arrive as `DocumentPutRaw` and replies go back as `DocumentPutRaw`,
the shape Epiphany's permit issuer already uses (R5) and that the document
server routes natively. Two request documents and two response documents:

| Document | Type id | Payload |
|---|---|---|
| request | `huginn.mind_request.v1` | `instance`, `operation: Admit \| Get \| Query \| OpenItems \| RulingsInForce \| Stewardship \| HandOff \| Import`, and the operation's typed argument |
| response | `huginn.mind_response.v1` | `request_id`, `outcome: Ok(payload) \| Refused(PipelineRefusal)` |

Read operations are also served through `CultMeshRudpSnapshotSource`, so a
plain CultNet snapshot client can read a mind without speaking the request
document at all. That is the affordance that keeps the MCP surface swappable.

**Schema publication.** `huginn.mind_request.v1` and `huginn.mind_response.v1`
are Huginn's contracts and are published from Huginn, not Epiphany. Ruling 1
covers pipeline *state* schemas, which stay in `schemas/cultnet/`. The organ's
transport contracts belong to the provider that owns the boundary, which is
exactly what `schemas/cultnet/README.md:52-56` already says.

**Port.** `rudp://10.77.0.1:17872`, the lower of the two unallocated `178xx`
ports (R21), with a private candidate range `27880-27887` by analogy with
Odin's `27872-27879`.

## D7. `eureka-state`

*Partly superseded by Cut 9 and Q13 A, 2026-09-16: the `query` output is a
page with `matched`; the `rulings_in_force` and `stewardship` tool rows are
presets; there is no `history` tool, without which the Q17 affordance
stops at the crate boundary; "types are the `epiphany-pipeline` types" is
false for the view and query types, which are `huginn-mind`'s. The Cut 13
refresh is the owner.*

A stdio MCP server that is a thin CultNet client. It owns no state, no cache and
no fallback.

| Tool | Input | Output |
|---|---|---|
| `admit` | `{ instance, provenance, documents }` | `PipelineAdmissionOutcome` |
| `get` | `{ instance, id }` | `{ found, view }` |
| `query` | `{ instance, query: PipelineQuery }` | `{ views }` |
| `open_items` | `{ instance, campaign }` | `PipelineOpenItems` |
| `rulings_in_force` | `{ instance, campaign? }` | `{ views }` |
| `stewardship` | `{ instance }` | `{ repos }` |
| `whoami` | `{}` | `{ instance, organ_endpoint, reachable }` |

Input and output types are the `epiphany-pipeline` types deriving `JsonSchema`,
returned as `Json<T>`, so tool schemas equal the published schemas (P4).
Refusals are typed outcomes, not JSON-RPC errors; malformed input is
`invalid_params`.

**No `repo_root` anywhere.** The old design threaded it through every tool
because the store was a file in a repo. The mind is now addressed by instance.

**Configuration.** Two environment variables, `EUREKA_ORGAN_ENDPOINT` (default
`rudp://127.0.0.1:17872`) and `EUREKA_INSTANCE`. No config file.

**Unreachable organ (target, "Availability is honest").** Every tool returns
`{ refused: Unavailable { detail } }` naming the endpoint and the failure. The
server never spools, never caches, never degrades to a local file. `whoami`
exists so an agent can check reachability in one call before starting a
campaign, and the Rehydrate brief calls it first.

**Registration is the operator's, not Hands'.** After Hands reports the binary
path, the operator runs:

```
claude mcp add --scope user --transport stdio eureka-state --env EUREKA_INSTANCE=<slug> --env EUREKA_ORGAN_ENDPOINT=rudp://127.0.0.1:17872 -- C:\Users\Meta\.eureka\bin\eureka-state.exe
```

Unprobed: the exact `--env` spelling on CLI 2.1.268. Hands confirms it from
`claude mcp add --help` and reports it; the operator runs the confirmed line.

## D8. The trust boundary

**State it plainly: on a single-operator LAN there is no authentication here,
and the design does not pretend otherwise.**

The organ binds loopback on Yggdrasil and is reached from the workstation over
the existing supervised SSH tunnel (R20). Anything that can open that socket can
declare any instance. `ForeignInstance` is therefore a **collision and
attribution** control — it stops instance A writing into B's mind by mistake,
and keeps history attributable — not an access control.

This matters because the alternative is available and already used in this
codebase: `cultnet-rs` ships `ServiceIdentitySigner`, `enroll_service_identity_at`
and trust anchors, and Epiphany's permit path signs with them (R4). Enrolling a
per-instance identity would make `ForeignInstance` enforceable. It would also
mean distributing and rotating a key to every workstation that runs Claude Code.

**This is a real fork; see Q6.** Do not let Hands invent a credential system.

## Cut 4. Delete the repo-store, lease and git layers

- **Repo/branch:** Epiphany `codex/eureka-pipeline-state`. No dependencies.
- **First:** confirm `git status` is clean at `5fb4eb22`.

**Deletes first.**

| Path | Lines | What dies |
|---|---:|---|
| `epiphany-core/src/pipeline_store.rs` | 1,251 | **The whole file.** See below. |
| `epiphany-core/src/pipeline_documents.rs:334-349` | 16 | `PipelineWriterHolder` and its `Bounded` impl |
| `epiphany-core/src/pipeline_documents.rs:492-497` | 6 | `EpiphanyPipelineWriterHolder` wrapper |
| `epiphany-core/src/pipeline_documents.rs:602-617` | 16 | `validate_pipeline_writes`, the commit-profile validator |
| `epiphany-core/src/lib.rs:21` | 1 | `mod pipeline_store;` |
| `epiphany-core/src/lib.rs:132` | 1 | `pub use pipeline_store::{...};` |
| `.gitattributes:1` | 1 | `/.epiphany/pipeline/pipeline.cc binary` |
| `.gitignore:37` | 1 | `/.epiphany/pipeline/*.lock` |

**Yes, `pipeline_store.rs` dies whole.** Every one of its parts is a repo-store
part:

- `PIPELINE_COMMIT_STORE` (`:76-81`) — the commit profile, dead with Cut 5.
- `pipeline_cache` (`:87-120`) — the epoch/foreign-type opener. Its *logic*
  survives, but it is re-authored against redb in Cut 8; it is not moved,
  because its signature takes a `RuntimeSpineBackingStore`.
- `git`, `git_line`, `committed_line` (`:125-162`) — the committed-blob checks.
- `require_repo_root`, `main_work_tree`, `common_dir_of` (`:165-201`) — ruling
  10's resolution.
- `PipelineStore` (`:204-226`) — the read path, now CultNet.
- `PipelineWriter`, `attach`, `require_branch`, `Drop` (`:230-331`) — the lease
  and branch binding.
- `live_holder`, `current_holder`, `holder_cache`, `read/write_writer_holder`
  (`:334-387`) — the holder record.
- `mod tests` (`:389-1251`, 863 lines) — 15 tests, all of which pin a deleted
  rule. The four that pin *document* rules — `every_pipeline_kind_round_trips_through_named_slot_zero`,
  `bounds_refuse_in_utf8_bytes`, `repo_fields_must_be_org_slash_repo`,
  `keys_are_derived_and_mismatch_refuses`, `composed_keys_cannot_collide`,
  `parent_ids_are_parsed_strictly`, `resolution_subject_is_a_full_id_of_its_kind`
  — **move to Cut 6**, not deleted. They are listed here as moves so the count
  is honest.

**Keeps.**

- Every document kind, value type, bound alias, format rule and key derivation
  in `pipeline_documents.rs`.
- `validate_pipeline_write_envelope` (`:621-645`), bounds plus key recomputation.
- All ten `schemas/cultnet/epiphany.pipeline.*.v1.schema.json` files and their
  `index.json` entries. Ruling 1 keeps them; Cut 6 adds three more.
- `.gitattributes:2`, `/schemas/cultnet/epiphany.pipeline.*.schema.json text eol=lf`.
- `process_observation::capture_process_instance` — other callers at
  `bin/epiphany-mvp-coordinator.rs:257,1703` and `bin/epiphany-swarm.rs:211,723`.
- `repository_body_observer::repository_git_command` — other callers at
  `repository_body_observer.rs:734,745,1041,1054,1063,1077`.
- `fs2` in `epiphany-core/Cargo.toml` — still used by
  `packaged_release/construction.rs`.
- `schemars` — Q5 keeps it; Cut 6 needs it.

**Per-file changes.** `pipeline_documents.rs:19` drops
`use crate::pipeline_store::PipelineRefusal;` and gains a local `PipelineRefusal`
holding only `FieldBound`, `InvalidFormat` and `InvalidIdentity`, with its
`Display` and `Error` impls carried from `pipeline_store.rs:57-63`. This is a
temporary home; Cut 6 moves the whole module out.

**Authority map.**

- **Owner:** nothing. This cut removes an owner and installs no replacement; the
  organ becomes the owner in Cut 8.
- **Inputs / outputs:** none.
- **Derived state:** none. The holder file is gone.
- **Forbidden writers:** after this cut, no Epiphany code may open a pipeline
  store, take a lease, or read git for pipeline purposes.
- **Shared paths:** none remain.
- **Deletion line:** the table above.

**Verification.**

- **Builds:** `cargo check -p epiphany-core --lib --tests`, then
  `cargo check -p epiphany-release-bundle --bin epiphany-state`. One at a time,
  `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`.
- **Tests:** `cargo test -p epiphany-core --lib`. Expect 180 − 15 = **165**, and
  the report states the exact number. The surviving document tests are re-added
  in Cut 6, so this cut legitimately reduces coverage; Soul checks that only the
  named 15 disappeared.
- **Negative greps**, all empty:
  - `rg -n "PipelineWriter|PipelineStore|WriterLeaseHeld|main_work_tree|check-attr|git-common-dir" epiphany-core/src`
  - `rg -n "\.epiphany/pipeline" -- . ':!notes'`
  - `rg -n "pipeline" .gitattributes .gitignore` returns only the schema
    `eol=lf` line.
- **Operator:** none.

**Subtraction ledger:** −1,291 lines, +about 20 (the reduced refusal enum). No
dependency, format or target change.

## Cut 5. Collapse `TypedCommitStore` back into the Mind commit owner

- **Repo/branch:** Epiphany, same branch. Depends on Cut 4.
- **Why this cut exists.** Cut 2 generalised the commit owner so a *second*
  profile could use it. Cut 4 deletes that second profile, and D3 puts the
  organ's receipts in another repo. `TypedCommitStore` is now a one-implementation
  abstraction surviving only because it exists — a named STOP condition in
  `~/.claude/CLAUDE.md`. Honest accounting: **Cut 2's generalisation did not pay
  off**, and leaving it in place to avoid admitting that is the failure mode the
  doctrine warns about.
- **Why it is a separate cut.** It is pure subtraction on the Mind path, and
  Soul must be able to falsify "Mind behaviour is unchanged" without pipeline
  code in the diff.

**Deletes first.**

| Path | Lines | What dies |
|---|---:|---|
| `reasoning_context.rs:1590-1597` | 8 | `struct TypedCommitStore` |
| `reasoning_context.rs:1599-1604` | 6 | `fn validate_mind_writes` (inlined) |
| `reasoning_context.rs:1606-1611` | 6 | `const MIND_COMMIT_STORE` |
| `reasoning_context.rs:1614` | 1 | the `store: &TypedCommitStore` parameter |
| `reasoning_context.rs:1435,1466,1516,1540,1566` | 5 | the `&MIND_COMMIT_STORE` argument at each of the five wrappers |
| `reasoning_context.rs:2521-2530`, `:2599-2610` | ~20 | `TEST_COMMIT_STORE`, `NOW_REFUSING` and the profile-parameterised test harness at `:2540` |

**Per-file changes.** In `commit_authorized_mind_mutation`
(`reasoning_context.rs:1613-1622`), re-inline the three choices the profile
carried: `runtime_spine_backing_store` at `:1641`,
`crate::runtime_spine::open_runtime_spine_cache` at `:1642`,
`validate_mind_write_envelope` at `:1644`, and the literal `"epiphany-mind"` at
`:1665` and `:1669`.

**Keeps — and this is the part that must not be lost.** Cut 2's *tests* proved
real rules and stay, re-expressed against the concrete owner:

- the Mind epoch refusal is pinned (mutation M2b dropped the check and failed);
- validation-before-replay is pinned;
- the owner reads and writes one store, never two.

Do **not** delete a verifier to make this cut smaller. AGENTS.md's Verification
Guardrails: preserve the claim in the smallest owning surface.

**Authority map.**

- **Owner:** `commit_authorized_mind_mutation`, still the only *production* code
  that builds receipts, replays, and runs batch CAS — now concretely, for Mind
  only. One test plants a receipt on purpose, which is how the replay rule is
  pinned; that is the second construction site and it is deliberate.
- **Inputs:** authority, invariant owner, strong reads, writes, companions, time.
- **Outputs:** `EpiphanyMindCommitOutcome`.
- **Derived state:** `store_id` inside the receipt's document versions, now the
  constant `"epiphany-mind"`.
- **Forbidden writers:** any new function building an `EpiphanyMindCommitReceipt`
  or calling `compare_and_swap_batch` with a receipt.
- **Shared paths:** the five Mind wrappers. There is no sixth.
- **Deletion line:** the table above.

**Verification.**

- **Tests:** `cargo test -p epiphany-core --lib` passes at the Cut 4 count.
  `disjoint_mind_mutations_merge_and_same_identity_conflicts`
  (`reasoning_context.rs:2482`) pins Mind CAS and replay unchanged.
- **Behaviour-unchanged proof, per the Hands brief:** a Mind receipt id captured
  by running the pin test **at base `5fb4eb22`** must equal the id after this
  cut. A value captured from the new code is not evidence.
- **Mutations:** drop the epoch check from the opener; make the validator a
  no-op; make the owner read one store and write another. Each must fail its
  named test.
- **Negative greps:** `rg -n "TypedCommitStore|MIND_COMMIT_STORE" epiphany-core/src`
  is empty.

**Subtraction ledger:** −46 lines, +about 8 re-inlined. No dependency change.

**If Soul or Hands finds a live second consumer this cut is wrong** — stop and
report rather than deleting. The map asserts there is none; `rg` is the check.

## Cut 6. Extract `epiphany-pipeline`

- **Repo/branch:** Epiphany, same branch. Depends on Cut 4.

**Adds: package `epiphany-pipeline/`.**

- `Cargo.toml`: `autobins = false`, `license-file = "../LICENSE"`, dependencies
  `anyhow`, `chrono`, `cultcache-rs` (pinned `a0813c6`), `rmp-serde`, `schemars = "1"`,
  `serde`. Dev-dependencies `serde_json`, `tempfile`.
- `src/lib.rs`: the whole of `pipeline_documents.rs` (623 lines after Cut 4),
  moved verbatim apart from the module doc's cut-map references, plus the
  reduced `PipelineRefusal`.
- The three new kinds from D2, their keys, and their entries in the
  `pipeline_kinds!` macro.

**Moves.**

- `epiphany-core/src/pipeline_documents.rs` → `epiphany-pipeline/src/lib.rs`.
- The seven document tests named in Cut 4 → `epiphany-pipeline/src/lib.rs`
  tests, minus their git-repo scaffolding (`campaign_repo`, `git_ok`, `attach`,
  `holder`), which has nothing left to set up.
- `pipeline_published_schemas_match_derivation` (`pipeline_store.rs:1172-1203`)
  → `epiphany-pipeline`, still reading `../schemas/cultnet` via
  `env!("CARGO_MANIFEST_DIR")`.

**Deletes.** `epiphany-core/src/lib.rs:20` (`mod pipeline_documents;`) and
`:131` (`pub use pipeline_documents::*;`). `epiphany-core` then has no pipeline
surface at all, which is correct: its consumption is campaign two.

**Per-file changes.**

- Root `Cargo.toml:2-8`: add `"epiphany-pipeline"` to workspace members. It is
  **not** added to `[dependencies]`, and no `[[bin]]` references it.
- `schemas/cultnet/index.json`: three new entries in the existing shape
  (`kind: document_payload`, `wireContracts: ["cultnet.schema.v0"]`) for
  `instance`, `stewardship` and `hand_off`.
- Three new `schemas/cultnet/epiphany.pipeline.<kind>.v1.schema.json` files,
  derived, never hand-written.
- `schemas/cultnet/README.md:29-31`: the Main Families entry currently says
  pipeline state is "stored per repo at `.epiphany/pipeline/pipeline.cc`".
  Replace with: stored in an instance's mind, owned by the Huginn memory organ;
  name the three added kinds.
- `schemas/cultnet/README.md:33-41`: the wire note says these contracts "cross to
  Eureka MCP clients and the voidbot projection". Replace the voidbot half: they
  cross to the Huginn organ and its `eureka-state` client. Keep the `[value]`
  payload sentence, the derivation sentence and the Q2 evolution sentence.

**Authority map.**

- **Owner:** `epiphany-pipeline`, for document shape, bounds, formats, keys and
  derived schemas.
- **Inputs:** none; it is a pure type library.
- **Outputs:** typed documents, derived JSON schemas, `pipeline_key`.
- **Derived state:** the published schema files.
- **Forbidden writers:** nothing in this package may open a store, spawn a
  process, or reach a network. It has no `std::process`, no socket and no
  backing-store dependency beyond `cultcache-rs` types.
- **Shared paths:** `huginn-mind` and `eureka-state`, both by git rev.
- **Deletion line:** `epiphany-core`'s two pipeline modules and their exports.

**Verification.**

- **Builds:** `cargo check -p epiphany-pipeline --lib --tests`, then
  `cargo check -p epiphany-core --lib --tests`.
- **Tests:** `cargo test -p epiphany-pipeline --lib`. The nine moved tests
  (seven plus the derivation test plus `decode_refuses_an_envelope_of_a_foreign_type`,
  which Cut 4's fix batch added after this was written), plus four new ones:
  `instance_stewardship_and_hand_off_round_trip`,
  `stewardship_key_escapes_the_repo_slash`, `hand_off_names_both_instances`, and
  `keys_read_back_as_ids_of_their_kind`. Thirteen in total.
- **Key⇄id round-tripping is the test shape this cut was missing.** The first
  three key tests assert key strings and never parse them back, which is why
  Soul found an unvalidated instance segment and a reader that refused the new
  root kind. `keys_read_back_as_ids_of_their_kind` walks every sample, keys it
  and parses it back. Resolution is the one kind whose key is not an id of
  itself; the test states that rather than skipping it.
- **The derivation test is the schema gate.** On mismatch it writes the derived
  file to a per-run directory under `std::env::temp_dir()` named by pid and
  nanos, and fails naming the path. There is no bless flag, and no directory is
  shared between runs. Thirteen schemas must now match.
- **Mutations:** add a variant to one value type's enum without regenerating its
  schema — the derivation test fails. Change `<Org_Repo>` escaping to keep the
  slash — the key test fails. (A *field* cannot be added to those macro-built
  value types without failing the whole target, which would prove nothing about
  the derivation test in particular; the variant changes exactly one derived
  schema.)
- **Negative greps:**
  - `rg -n "serde_json::Value|Vec<u8>" epiphany-pipeline/src` empty
    (`serde_json` is a dev-dependency only).
  - `rg -n "std::process|UdpSocket|BackingStore" epiphany-pipeline/src` empty.
    *Corrected (21): `process::Command|UdpSocket|BackingStore`; the per-run
    test directory reads `std::process::id()` under `#[cfg(test)]`.*
  - `rg -n "pipeline" epiphany-core/src` empty. *Corrected (15): the narrower
    grep over the type names.*
- **Operator:** none.

**Subtraction ledger:** Epiphany-core −623 lines; new package +about 700
including the three kinds, +about 400 derived JSON. Net repo change is small;
the point is the dependency boundary, not the line count.

## Cut 6b. The key grammar

Anchor: `epiphany-pipeline/src/lib.rs` at `4a654351`, 1,508 lines. Lands
**before** Cut 6c and does not absorb it. Spec by Imagination 2026-09-16;
in Hands.

### Why a grammar and not a fourth fix

Three passes each made one composition site injective and left the site above
it ambiguous. The key space had **four** shapes and no single reader: `<slug>`
for campaign and instance (two early returns at `:693-700`), `<root>:<kind>:<local>`
for ten kinds (`KeyParts::key`, `:671-683`), `resolution:<subject id>` (a third
early return at `:703-707`), and recursively `resolution:resolution:…`. Three of
the four are written by an early `return` that never reaches the composer, so
the reader `pipeline_id` (`:545-586`) needs a `match kind` with a root arm and a
recursive arm to undo them, plus `declared_kind` (`:529-538`) to guess the kind
of a string that does not carry one. Every confirmed defect lives in one of
those exceptions. A fourth pass would guard an arm that should not exist.

### The grammar

```
key      ::= root ":" kind ":" local
root     ::= slug
kind     ::= one of the thirteen `PipelineKind` names, literally
local    ::= part ( "." part )*
part     ::= label
slug     ::= label ( "." label )*            ; total <= 64 bytes
label    ::= [A-Za-z0-9_-]{1,64}
```

Two byte facts already in the source: `label_text` (`:118-126`) admits neither
`.` nor `:`; `dotted_text` (`:131-139`) is `label ("." label)*` bounded at 64
bytes with no empty parts. Two rules the cut adds:

- **R1. Every key has exactly three segments.** Roots included.
- **R2. No local part is a `Slug`.** A `Slug` or `OrgRepo` entering a local is
  escaped to one label first. So `local.split('.')` recovers its parts exactly
  at any arity.

Unambiguity then follows from the alphabet, not from checks: `:` is in no part,
so segment recovery is forced; the kind is a literal segment, so nothing is
inferred; `.` is in no part, so local recovery is exact. `declared_kind` has no
successor because there is nothing left to guess.

| Kind | Key |
|---|---|
| campaign `c` | `c:campaign:self` |
| instance `y` | `y:instance:self` |
| resolution of `c:question:Q1` | `c:resolution:question.Q1` |
| resolution of campaign `c` | `c:resolution:campaign.self` |
| resolution of that resolution | `c:resolution:resolution.question.Q1` |
| hand-off | `<from>:hand_off:<to escaped>.<repo escaped>.<date>` |
| stewardship | `<instance>:stewardship:<repo escaped>` |
| the nine others | `<campaign>:<kind>:<local>`, unchanged |

Roots take the reserved constant `self` as local; a reader recovers a root's
identity from segment 1 and never consults it. A resolution is keyed inside its
subject's root with the subject's kind and local as its own local. Nesting is
bounded by construction, not by a guard: a nested local is
`11 * (depth - 1) + len(subject local)` bytes against the 64-byte bound, so a
one-byte subject local nests six deep and `question.Q1` five, and
`pipeline_id` does not recurse. *(Corrected after Soul F1: this first said
"at most five"; the bound is the local, and the depth follows from the subject.)*

**The one escape, generalised.** `repo_segment` (`:619-636`, `_`→`__`,
`/`→`_-`, `.`→`_d`, injectivity verified by Soul) becomes `key_segment` and
gains one caller: `hand_off.to_instance`, the only place a raw `Slug` reached a
local (`:720`) and exactly Soul's pass-2 collision pair, closed from the other
side. Its output is not assumed to be a label; `Game Cult/x` escapes and is
refused downstream by `label_text`, which is already tested.

**Where the total length bound lives:** in one function,
`local<const N: usize>(field, parts: [&str; N])`, which runs every part through
`label_text` and the join through one 64-byte `bound`. It is not a type fact and
is not dressed as one; it is pinned by M4. Today the total check (`:681`) is the
second of two `dotted_text` calls on overlapping data, which is why deleting it
was invisible to every sample.

**Where tail arity lives:** `local` takes an array, not a chainable builder, so
a conditional tail cannot be written as a quiet extra chain call. That does
*not* prove one arm emits one arity, and after this cut arity is no longer
load-bearing: under R2 a local reads back at any arity. The property the doc
comment at `:663-670` defended is retired, and the invariant that replaces it,
"no local part carries the separator", has a runtime mutation (M6).

### Deletes first

| Path | Lines | What dies |
|---|---:|---|
| `lib.rs:526-538` | 13 | `declared_kind`. Defect 2's hardcoded `Campaign` dies with the function. |
| `lib.rs:550-570` | 21 | `pipeline_id`'s `match kind`: the root arm, the resolution arm and its recursion, and the comment defending them. |
| `lib.rs:638-684` | 47 | `KeyParts`, its constructors and `key`. Replaced by a free function, not renamed. |
| `lib.rs:692-707` | 16 | `pipeline_key`'s three early `return`s. The cut is not done until they are gone. |

97 lines. No file, schema file or type dies. The deletes go **before** `local`
is written; if they coexist with the new composer at any commit the cut has
produced a fourth shape.

### Keeps, moves, adds

Keeps: every kind, type id, field, bound and format type; no `value_types!` or
`pipeline_kinds!` entry moves; `PipelineRef` and its `Bounded` impl
(`:365-370`) unchanged. `repo_segment` → `key_segment`, same body.
`parent_local` (`:589-600`) unchanged. `parent_cut` (`:605-617`) keeps both
rules; `rsplit_once('.')` becomes an index into the split, net −4. The
outcome-invariance assertion in `keys_are_derived_and_mismatch_refuses`
(`:1077-1084`) and the `!key.starts_with("<campaign>:")` assertion in
`instance_stewardship_and_hand_off_round_trip` (`:1311-1315`) stay true.

Adds: `const ROOT_LOCAL: &str = "self"` and `fn local`. `pipeline_id` is
**replaced whole**, one shape with no `match kind`: exactly three segments, kind
segment equals `kind.name()`, root through `dotted_text`, every local part
through `label_text`, local bounded whole, returns `(root, local)`.

### Per-file changes, `epiphany-pipeline/src/lib.rs`

| Line | Change |
|---|---|
| `:12-13` | Module doc states the grammar in four lines. |
| `:117` | `Label`'s doc comment retargets `KeyParts::key` to `local`. |
| `:526-538` | Delete `declared_kind`. |
| `:540-586` | Replace `pipeline_id` whole. |
| `:605-617` | `parent_cut`: index into `local.split('.')`. |
| `:619-636` | Rename `repo_segment` → `key_segment`; generalise its doc comment. |
| `:638-684` | Delete `KeyParts`; add `local` and `ROOT_LOCAL`. |
| `:687-752` | `pipeline_key`: thirteen arms, one exit. Campaign/Instance emit `local(field, [ROOT_LOCAL])`. Resolution parses its subject id once, then `local(field, [subject_kind.name(), subject_local_parts…])`. `HandOff` gains `key_segment` on `to_instance`. |
| `:780-781`, `:824-934` | Thirteen expected sample keys move. |
| `:1088-1166` | `composed_keys_cannot_collide`: the probe built `KeyParts` directly and moves to `local`; the hand-off pair keeps its conclusion with a dotted receiver now escaping to `thought-cage_dGameCult`. |
| `:1256-1291` | `a_resolution_is_named_by_the_key_it_has`: `{CAMPAIGN}:resolution:R8` becomes well-formed grammar and leaves the refused list; reachability is admission's rule. Said in the doc comment. |
| `:1329-1363` | `keys_read_back_as_ids_of_their_kind`: **body unchanged.** |
| `:1419-1455` | `hand_off_names_both_instances`: add a dotted-receiver assertion. |

`schemas/cultnet/`: **no file changes.** `notes/eureka-pipeline-state-cut.md`:
the Keys table above is marked superseded and the scar at the old `:483-488`
stays.

### Authority map

- **Owner:** `pipeline_key` (`:687`), through the single composer `local`. No
  arm has a path to a key the composer never saw.
- **Inputs:** the document's own fields. No clock, store, registry or counter.
- **Outputs:** one `String`, or a `PipelineRefusal` naming the field.
- **Derived state:** the key is derived, never stored. `pipeline_id` is the key
  read backwards and holds no state. `declared_kind`'s inference is deleted.
- **Forbidden writers:** no arm of `pipeline_key` may `return` a key; no local
  part may be a raw `Slug`, `Short` or `OrgRepo` (a validated `Date` is
  `[0-9-]{10}`, already a label, and passes raw; Soul F6 corrected the first
  wording, which claimed every non-label value was escaped); nothing may infer
  a kind from an id's shape; reachability ("does this document exist", "may this
  kind be resolved") stays with admission in `huginn-mind` per the resolution
  matrix above. Cross-field rules stay out of `Bounded`.
- **Shared paths:** `pipeline_key`, `pipeline_id`, `PipelineRef::validate`
  (`:365-370`), `validate_pipeline_write_envelope` (`:759-772`). Four paths,
  one grammar, no fifth.
- **Deletion line:** the 97 lines above, before `local` is written.

### Verification

Builds: `cargo check -p epiphany-pipeline --lib --tests`, then
`cargo check -p epiphany-core --lib --tests` (untouched; its only mention of
the package is a doc comment at `runtime_spine.rs:8754-8758`).
`CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`, path-list baseline.

Tests: `cargo test -p epiphany-pipeline --lib`, thirteen existing plus six.

| Test | Rule it pins |
|---|---|
| `every_key_has_exactly_three_segments` | R1, over every sample plus a nested resolution: three segments, segment 1 a `Slug`, segment 2 in `PipelineKind::ALL` by name, each local part a `Label`. Also asserts a `Target` whose campaign is `Slug("a:b")` is refused by the `pub` `pipeline_key` (M5). |
| `roots_of_different_kinds_do_not_share_a_key` | Defect 1: campaign `yggdrasil` and instance `yggdrasil`, `assert_ne!`. |
| `a_resolution_names_its_subjects_kind` | Defect 2: resolutions of `c:question:Q1` and `c:ruling:Q1` key differently, as do resolutions of campaign `c` and instance `c`, and each reads back recovering its subject kind. |
| `a_resolution_of_a_resolution_reads_back` | Defect 3: a resolution whose subject is a resolution validates, keys, reads back; five nestings key, six refuse on the local bound with `field == "resolution.key"`. |
| `a_composed_local_is_bounded_whole` | The total bound: a hand-off whose escaped receiver is 60 label bytes composes about 80 and is refused `InvalidFormat { field: "hand_off.key" }`. |
| `no_local_part_carries_the_separator` | R2: `local("probe", ["a.b"])` and `local("probe", ["a", "b.c"])` both refused; a hand-off with a dotted `to_instance` keys to an escaped local. |

Unchanged in name and assertion: `keys_read_back_as_ids_of_their_kind`,
`keys_are_derived_and_mismatch_refuses`,
`every_pipeline_kind_round_trips_through_named_slot_zero`,
`pipeline_published_schemas_match_derivation`,
`decode_refuses_an_envelope_of_a_foreign_type`, `bounds_refuse_in_utf8_bytes`,
`repo_fields_must_be_org_slash_repo`, `parent_ids_are_parsed_strictly`,
`resolution_subject_is_a_full_id_of_its_kind`.

Negative checks: `rg -n "declared_kind|KeyParts" epiphany-pipeline/src` empty;
no `return Ok(` inside `pipeline_key`; `rg -n "match kind"` empty;
`git diff --stat 4a654351 -- schemas/cultnet/` **empty**;
`rg -n "to_instance.0.clone\(\)"` empty. If Hands finds any consumer of a
pipeline key outside `epiphany-pipeline/src/lib.rs`, the cut is wrong; stop.

### Mutations

Committed as `tools/eureka-cut6b-mutations.ps1` with byte-exact UTF-8 I/O,
anchors that must match exactly once, and **M0, a no-op control** that rewrites
the file through the same path and must leave every test green. A redesign's
mutations restore the old permissiveness rather than break a check.

| # | Survivor | Mutation, exactly | Killed by |
|---|---|---|---|
| M1 | Roots share a namespace | `Campaign` arm returns `Ok(value.slug.0.clone())` | `roots_of_different_kinds_do_not_share_a_key`; collaterally the three-segment test and the read-back test |
| M2 | Resolution key drops the subject kind | `Resolution` arm omits `subject_kind.name()` from the parts | `a_resolution_names_its_subjects_kind` (both pairs key `c:resolution:Q1`) |
| M3 | Nested resolution unnameable | `Resolution` arm returns `format!("resolution:{}", subject.id)` | `a_resolution_of_a_resolution_reads_back`; the three-segment test |
| M4 | Total bound unpinned | In `local`, delete the `bound` on the join, keep per-part `label_text` | `a_composed_local_is_bounded_whole` |
| M5 | Root check unpinned | In `pipeline_key`, drop `dotted_text` on the root | the refusal assertion in `every_key_has_exactly_three_segments` |
| M6 | Head may carry a dot | In `local`, `dotted_text` on `parts[0]` instead of `label_text` | `no_local_part_carries_the_separator` |

M6's stated limit: the array type is a compile fact with no runtime mutation,
and one-arm-one-arity is not proven and no longer needs to be.

### Ordering against Cut 6c

Cut 6c's central keep ("no key moves") is false after this cut, its retyping of
`ResolutionOutcome`'s referents to `PipelineRef` rests on the `pipeline_id`
this cut replaces whole, and this cut is the smaller diff in the shared sample
block. So 6b lands first, and Imagination reissues 6c against the landed
grammar with three edits: re-anchor by content (everything from `:526` shifts
by about −60 lines); replace "no key moves" with "keys moved in 6b and are
settled; 6c moves none further, checked by `git diff -- schemas/cultnet/`
naming exactly six files"; and add one fixture the new grammar makes available,
a `Superseded` entry naming a resolution (`c:resolution:question.Q1`).

### Subtraction estimate

−97 source, +38 outside tests, +110 tests; net −59 outside tests. Zero schema
files, types, kinds, dependencies, targets, formats or epoch. **Every key
moves**, which is the cut's whole cost and is free exactly once: nothing reads
these keys yet. Once `huginn-mind` reads them, the same change is a
stored-document re-key.

## Cut 6c. The Ghostlight shapes

- **Repo/branch:** Epiphany `codex/eureka-pipeline-state`. Depends on Cut 6b
  (landed at `1bddd2ac`, mutation suite at `95ee551a`) and its fix batch.
  Blocks Cut 8. Lands before Cut 8 (ruling C) and is the last cut inside
  Epiphany before the pause.
- **Anchors.** Every `file:line` below is against `epiphany-pipeline/src/lib.rs`
  at `95ee551a`, 1,608 lines, twenty tests, re-found by content after the key
  grammar shifted everything from the old `:526` down. The 6b fix batch lands
  on `lib.rs` before Hands starts, so **re-anchor by content again**: every
  anchor is a `struct`, `impl`, `fn` or sample opener that greps cleanly.
- **Reissued** 2026-09-16 by Imagination against the landed grammar. Three
  edits from the first issue: anchors re-found; the Keeps claim about keys
  rewritten; one fixture added that only the new grammar makes available, a
  `Superseded` entry naming a resolution by the key it has. Deletes, adds,
  `Promise`, the two unit enums, `StructuralDelta`, M17/M19/M20/M21, the
  authority map and the subtraction estimate carry over; M16 and M18 keep
  their rule and gain sharper forgeries; M22 is new.

**Why this cut exists, and why now.** Ghostlight's independently run campaign
(D9) named six pains. Four were already built: finding identity, the ledger
derivation, cut status and the direction of the supersession relation are
covered by keys, joins and `PipelineResolution` as they stand. What is left is
one genuine hole (promises), two fields wearing one shape (ruling authority,
finding origin), and four retypings that are breaking, not additive. Under
ruling 8 a retyping costs an epoch bump the moment a pinned reader exists.
Nothing reads these schemas today: a grep for `SubtractionEstimate`,
`MutationRecord`, `ResolutionOutcome`, `VerdictClaim`, `StructuralDelta` and
`subtraction_estimate` over Epiphany finds `lib.rs`, the four schema files
that embed them, and one prose mention in this map; over `F:\Projects\Huginn`
and the Eureka skill checkout it finds nothing (source read). A whole-drive
search timed out, so any other tree is unchecked. Cut 8 pins a git rev and
closes the window. **The four retypings cost nothing today and an epoch
tomorrow.**

**Rulings carried in:** A, Soul measures every promise and `Unproven` carries
the ones it could not reach; B, `Fixed { commit: Sha }` is admitted with its
referent outside the document set; C, this lands before Cut 8.
Multi-supersession is `Vec<PipelineRef>[8]`, cardinality being admission's
(Q10 asked against Cut 8). Q11 A: nesting stays. Q12: `OrgRepo` untouched; the
tightening is its own commit and is not folded in. No kind widens, so
`epiphany.pipeline.epoch.v1` holds. **Zero new kinds.**

### Deletes first

| Path | Lines | What dies |
|---|---:|---|
| `lib.rs:290-294` | 5 | `SubtractionEstimate` and the two-line comment on its list default. Its consumer becomes `StructuralDelta`. |
| `lib.rs:388-396` | 9 | `impl Bounded for ResolutionOutcome`. Replaced whole, not patched: every arm changes referent type, and a patched impl is how a length-only check survives. |
| `lib.rs:826-828` | 3 | The cut-spec sample's `SubtractionEstimate { … }` literal. |
| `lib.rs:296` | 1 | `MutationRecord`'s `mutation: Line` field, rewritten in place as `location`/`before`/`after`. |

The comment at `:298-299` names the dead type and is reworded to stand alone.
No file or schema file dies; `index.json` is untouched because no kind is
added and no type id renamed. Deletes go before any field is added: if
`SubtractionEstimate` and `estimate: StructuralDelta` coexist at any commit,
the cut has two estimate shapes.

### Keeps

- **All thirteen kinds, `pipeline_key`, `pipeline_id`, `local`, `key_segment`
  and the epoch.** Keys moved in Cut 6b and are settled; this cut moves none
  further, because it adds, retypes or removes no field a key is derived from.
  The check is mechanical: `git diff --stat <base> -- schemas/cultnet/` at the
  end names exactly the six regenerated schema files. `index.json` carries no
  hashes (its entries are id, kind, wire contracts, version, type, title and
  path, and the derivation test asserts that shape), so it is byte-identical.
- **`PipelineRef` and its `Bounded` impl (`:363-376`).** It becomes
  load-bearing for four more referents. It already parses the id against its
  declared kind through `pipeline_id`, whose kind-segment check (`:553-555`)
  refuses a mismatch; that is why retyping 1 is cheap and what the new fixture
  rides on.
- **The whole key suite**, `keys_read_back_as_ids_of_their_kind` included. All
  twenty tests stay unchanged in name and assertion. They are blind to this
  cut, which is why it carries its own tests.
- **`Short`** stays the type for full-id fields already parsed at key
  derivation (`cut_report.cut_spec`, `verdict.cut_report`, `finding.verdict`,
  `cut_spec.rulings`, `ruling.answers`). Retyping those is a larger cut.
- **The outcome-invariance assertion** in `keys_are_derived_and_mismatch_refuses`
  (`:1029-1037`): a subject has one resolution key whatever the outcome.

### Adds

Two `unit_enums!` entries and one `value_types!` entry. No kind, module,
dependency or target.

```
RulingAuthority { Operator, Standing, Defaulted }
FindingOrigin   { Introduced, PreExisting }
pub struct Promise { label: Label, text: Line }
```

`Promise` is not a document and has no key; it is identified by its report's
id plus its label, as `TargetInvariant` is by its target's (`:276`). It is not
resolvable and does not enter the resolution matrix.

### Per-file changes, `epiphany-pipeline/src/lib.rs`

| Line | Change |
|---|---|
| `:262-267` | Add `RulingAuthority` and `FindingOrigin` to the `unit_enums!` block. |
| `:290-294` | Delete `SubtractionEstimate` and its comment. |
| `:296` | `pub struct MutationRecord { label: Label, rule: Line, location: CodeLocation, before: Line, after: Line, commit: Sha, failed_as_expected: bool }` |
| `:298-299` | Reword the comment so it no longer names `SubtractionEstimate`. |
| after `:307` | `pub struct Promise { label: Label, text: Line }` beside `LandedName`. |
| `:308` | `VerdictClaim` gains `promise: Option<Label>` and `mutations: Vec<Label>[8]`. |
| `:319-322` | `PipelineRuling` gains `authority: RulingAuthority`. |
| `:327` | `subtraction_estimate: SubtractionEstimate` → `estimate: StructuralDelta`. |
| `:330-335` | `PipelineCutReport` gains `promises: Vec<Promise>[64]`. |
| `:337-341` | `PipelineFinding` gains `origin: FindingOrigin`. |
| `:378-386` | `ResolutionOutcome`: `Superseded { by: Vec<PipelineRef> }` with `#[schemars(extend("maxItems" = 8))]` on `by`, because the enum is outside `value_types!`, the only place `[max]` is emitted automatically (`:236`); `Answered { by: PipelineRef }`; `Fixed { commit: Sha, by: Option<PipelineRef> }`; `Deferred { to: PipelineRef }`. `Recorded`/`Withdrawn` unchanged. |
| `:388-396` | Replace `impl Bounded for ResolutionOutcome` whole: `Superseded` → `list(&format!("{field}.by"), by, 8)`; `Answered` → `by.validate("{field}.by")`; `Fixed` → validate `commit` as `{field}.commit` then `by` as `{field}.by`; `Deferred` → `to.validate("{field}.to")`; `Recorded`/`Withdrawn` → `reason.validate("{field}.reason")`. `Option<PipelineRef>` validates through the blanket impl (`:68-72`); `list` (`:99-105`) checks the maximum and validates every item. |
| `:800-808` | Ruling sample gains `authority: RulingAuthority::Operator`. |
| `:826-828` | Cut-spec sample: `estimate: StructuralDelta { … }` on its own lines opening with `estimate: StructuralDelta {`, carrying Cut 3a's real numbers `lines_added: 900, lines_removed: 0`, not transposed (M21). |
| `:835` | Report sample's `MutationRecord` literal gains `label`, `location`, `before`, `after`, `commit`. |
| `:831-845` | Report sample gains `promises: vec![Promise { label: l("P1"), text: "One derived key per document.".into() }]`. |
| `:846-852` | Verdict sample: the claim gains `promise: Some(l("P1"))` and `mutations: vec![l("M1")]`. |
| `:853-859` | Finding sample gains `origin: FindingOrigin::Introduced`. |
| `:866-870` | Resolution sample: `Answered { by: PipelineRef { kind: Ruling, id: id("ruling", "R8") } }`. Its key stays `eureka-state:resolution:question.Q1`. |
| `tests`, after `:1510` | Five new tests, named under Verification. No sample is added, so every `samples().remove(N)` index (`:889-926`) is unchanged. |

`schemas/cultnet/`: six of thirteen files regenerate: `ruling`, `cut_spec`,
`cut_report`, `verdict`, `finding`, `resolution`. Derived, never hand-written:
`pipeline_published_schemas_match_derivation` writes the derivation to a
per-run temp directory and names it in the failure; copy from there. The other
seven, `index.json`, `README.md` and the two non-pipeline schemas are
byte-identical.

### The fixture the grammar made available

Before 6b a resolution's key was `resolution:<subject id>`, which `pipeline_id`
could not read back as an id of kind `Resolution`, so no `PipelineRef` could
name one. After 6b it is an ordinary `<root>:resolution:<subject kind>.<subject
local>` and reads back. This cut is the first to put such a reference inside
another document's outcome, pinned in
`resolution_outcome_referents_are_parsed_ids_of_their_kind`:

- **Positive.** A resolution of `eureka-state:ruling:R8` with outcome
  `Superseded { by: [Ref{Ruling, ruling:R9}, Ref{Resolution, <the sample resolution's key>}] }`
  validates. The resolution id is derived from `pipeline_key(&resolution_sample())`,
  not spelled. That is Ghostlight's own case: a ruling partly overturned by two
  later records.
- **Forgery.** The same list with the resolution entry's kind changed to
  `Ruling` is refused `InvalidFormat` with `field == "resolution.outcome.by[1].id"`.
  Well-formed, right length, right root, wrong kind. This is M16's killer; a
  length-only check passes it.

Stated limit: a `Superseded` entry naming `eureka-state:resolution:R8` also
validates. That id is well-formed grammar no resolution derives; whether a
document with an id exists is admission's rule. Hands must not make the
library refuse it.

### Authority map

- **Owner, inputs, outputs, derived state:** unchanged. `epiphany-pipeline`
  owns shape, bounds, formats and keys. No status, ledger row or cut state is
  stored by this cut, and none may be added by it.
- **Moved:** a resolution's referent stops being an unparsed string and becomes
  a parsed `PipelineRef` (or a `Sha`). Before, `ResolutionOutcome`'s referents
  were length-bounded only, alone among referents in the library. After, they
  are validated where every other referent is, `PipelineRef::validate`, one of
  the four grammar paths 6b named.
- **Forbidden writers:** cross-field and cross-document rules stay out of
  `Bounded`. Hands must not implement any of these in this package:
  `operator_quote` only with `authority: Operator`; every promise in a cited
  report named by exactly one claim (ruling A); a `mutations` label existing in
  the cited report; a `Superseded` list being non-empty; a `Superseded`
  referent existing. They are admission and belong to `huginn-mind`
  (`:97-98`, `:706-708`).
- **Shared paths:** none new. Nothing consumes these types yet.
- **Deletion line:** the deletes table, before any field is added.

**Cut 8 inherits five named refusals** and its verification table grows by
five rows: `QuoteWithoutOperator`, `PromiseWithoutVerdict`,
`UnknownMutationLabel`, `EmptySupersession`, `UnknownSupersessor`.

### The multi-supersessor hole

D9 found that one resolution per subject cannot hold Ghostlight's two partial
supersessors, and the key permits exactly one resolution per subject. The
shape half closes here: `Superseded` takes `Vec<PipelineRef>[8]`. Cardinality
is an admission rule, not a shape, so a permissive list in the schema plus a
restrictive rule in Cut 8 costs nothing whichever way Q10 is ruled; the
reverse ordering costs an epoch.

### Verification

**Builds:** `cargo check -p epiphany-pipeline --lib --tests`, then
`cargo check -p epiphany-core --lib --tests`. `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`,
one package, the workstation as host and target; this library has no
platform-specific code.

**Tests:** `cargo test -p epiphany-pipeline --lib`. The existing tests plus
five, all existing unchanged in name and assertion.

| Test | Rule it pins |
|---|---|
| `resolution_outcome_referents_are_parsed_ids_of_their_kind` | A resolution's referent is a full id of the kind it declares. Covers `Superseded` (positive and forgery), `Answered`, `Deferred`, and `Fixed`'s optional `by`, each with a wrong-kind forgery asserting the refusal's `field`. |
| `fixed_resolution_requires_a_commit_sha` | Ruling B: a fix names the tree where the finding stopped being true. Forgery: uppercase hex of legal length. |
| `mutation_records_carry_a_dot_free_label_and_a_commit` | A mutation has a key-safe identity and is pinned to a tree. Forgeries: `label: "M1.a"`, `commit: "dirty-worktree"`. |
| `a_verdict_claim_names_the_promise_and_the_mutation_it_measured` | Ruling A's shape half: `promise` and `mutations` exist, are `Label`-typed, and `mutations` is bounded at 8 (nine refused `FieldBound { field: "verdict.claims[0].mutations", limit: 8, actual: 9 }`). |
| `the_sample_cut_spec_estimates_a_net_addition` | The estimate's two `u32` fields are not interchangeable: `lines_added == 900`, `lines_removed == 0`. A tripwire with stated limits (M21). |

**Negative greps over `epiphany-pipeline/src`:**
`rg -n "SubtractionEstimate|subtraction_estimate"` empty;
`rg -n "Superseded \{ by: Short|Answered \{ by: Short|Fixed \{ by: Short|Deferred \{ to: Short"`
empty; `rg -n "operator_quote|promise|mutations"` shows no `if`, `match` or
`?`-chained condition on another field; `git diff --stat <base> -- schemas/cultnet/`
names exactly six `.schema.json` files; `rg -n '"maxItems": 8' schemas/cultnet/epiphany.pipeline.resolution.v1.schema.json`
matches once under `Superseded.by`. **If `schemars` 1 rejects `extend` on a
variant field, stop and report; do not drop the bound from the schema.**

**Operator:** none blocking.

### Mutations, M16-M22

Entries file `tools/eureka-cut6c-mutations.psd1`, run through the shared
`tools/eureka-mutations.ps1` harness from the 6b fix batch (built-in M0,
byte-exact I/O, anchors matching exactly once, hash-checked restore).

A retyping is the mutation-hostile case: the compiler catches the type change,
so the tempting test is "construct a valid value, assert `Ok(())`", which stays
green under every mutation below. The key suite is structurally blind here
because a resolution's key is outcome-invariant. Where the cut's content is
types, the mutation is a type-level mutation (M19, M20, M22).

| # | Mutation, exactly | Killed by | The careless test it survives |
|---|---|---|---|
| **M16** | `Superseded` arm: `list(&format!("{field}.by"), by, 8)` → `bound(&format!("{field}.by"), 8, by.len())`, length only. | the referents test, via the wrong-kind forgery in a two-entry list | any test superseding with a good id |
| **M17** | The whole `match self { … }` body → `let _ = field; Ok(())`. | the same test's `Answered` and `Deferred` forgeries | every existing test |
| **M18** | `commit.validate(&format!("{field}.commit"))?;` → `let _ = commit;` | `fixed_resolution_requires_a_commit_sha` via `Sha("5F98228D9C")`; `hex` requires lowercase (`:114-120`) | a test using `Sha("notacommit")`, which a length check also rejects |
| **M19** | `MutationRecord { label: Label,` → `label: Short,` | `mutation_records_carry_a_dot_free_label_and_a_commit` via `label: "M1.a"` | a test asserting `label: "M1"` validates |
| **M20** | `commit: Sha, failed_as_expected: bool }` → `commit: Short, …` | the same test via `commit: "dirty-worktree"` | any test supplying a real sha |
| **M21** | Two-line anchor `estimate: StructuralDelta {` + `lines_added: 900, lines_removed: 0,` → numbers transposed. The anchor must include the opener; the numbers alone also match the report sample (`:839`). | `the_sample_cut_spec_estimates_a_net_addition` | every round-trip, bounds and schema test |
| **M22** | `VerdictClaim`: `mutations: Vec<Label>[8]` → `[16]` | the verdict-claim test via nine labels | any test supplying one label |

Stated limits: M21 is a fixture assertion and the only test-visible hazard in
retyping 4; once both estimate and delta are `StructuralDelta`, six-axis
comparability is a type fact. `Promise` and `promises: Vec<Promise>[64]` have
no mutation: a promise is two bounded strings, and the rule that matters
(every promise measured) is Cut 8's.

### Subtraction estimate

Removed: 17 source lines (one value type with its comment, one hand-written
`Bounded` impl, one sample literal, one field rewritten in place) and one
unvalidated-referent class. Added: about +125 source, about 85 of them the five
tests; about +23 outside tests. Derived JSON: six files regenerate, about +130
/ −45. Value types net zero. Kinds, dependencies, targets, formats, epoch: all
zero. Liability retired: the one field class whose referent was never parsed,
and a pair of estimate/actual types that could not be compared on four of six
axes, both of which would have been paid for at an epoch bump the moment Cut 8
pinned a rev.

**If Hands finds a consumer of any of the five names outside
`epiphany-pipeline/src/lib.rs` and the four schema files, this cut is wrong**:
stop and report rather than retyping.

**Landed 2026-09-16** at `4d6af409` (deletes; does not build by design, the
outcome having no `Bounded` impl until the next commit), `13570e84` (the
shapes; 26 tests, 0 warnings) and `dddf9ede` (entries M16-M22 in
`tools/eureka-cut6c-mutations.psd1`). Hands re-anchored by content, since
`lib.rs` had moved twice since `95ee551a`; the spec's "twenty tests" was
twenty-one by then. `schemars` 1.2.2 accepted the `extend` attribute on the
variant field, and `"maxItems": 8` appears exactly once, under
`Superseded.by`. Six schema files regenerated (+193/−22); `index.json`
byte-identical. All four suites green-and-killed afterwards, the nineteen
6b entries included: keys did not move. Structural delta: source outside
tests +45/−19, tests +163/−5, file 1,616 → 1,801 lines, no kind, dependency,
target, format or epoch change.

Corrections and scars from Hands:

29. **M19 and M20 first came back "DID NOT BUILD."** The sample and the test
    spelled the label and commit with the newtype constructors, so widening
    the type made the tree fail to compile instead of admitting the forgery.
    Fixed by constructing through `From<&str>`. A type-level mutation only
    reads as a kill when the fixture is spelled so the widened tree compiles;
    the spec implied that and did not say it.
30. **`5f98228` is a legal seven-character sha**, so it was no forgery;
    replaced by six characters.
31. **The estimate test carries one assertion beyond the spec**: the sample
    estimate equals the sample report's structural delta, which required
    aligning the sample's `formats_added`. Soul is asked whether that moved
    anything else.
32. **The refusal `value` is the failing part, not the whole id**, so the
    third-batch reader fixture asserts `field` only, as Cut 6b's did.
33. **"The nineteen 6b entries" is seventeen** (M1-M6, S2, S4, S5, N3,
    N6-N10, X1, X11). Self's count, corrected by Soul.

**Soul's pass on Cut 6c and the third batch** (Fable; `soul-cut6c-*` in the
session scratchpad) held every shape, schema and key promise: all thirteen
derived schemas byte-identical to the committed files, exactly six changed
against `117f54b7`, the bound under `Superseded.by` alone, `Fixed.by`
nullable and `commit` the sha pattern, `Promise` in the report schema with
its bound, the thirteen expected keys identical before and after, the
`Bounded` impl reading only its own fields, and the extra estimate
assertion moving nothing else. All four suites green-and-killed through the
harness; Hands' attack script reruns identically; a one-second timeout on
real cargo kills cleanly and leaves no lock. Non-revert mutations that
validated only the first supersessor, mis-indexed the refusal path, or
skipped `Some` in the blanket impl all died. Three fixtures were missing:

- **`Fixed.commit` is unpinned when `by` is `Some`** (medium): the only
  forgery had `by: None`, so an impl that trusts the reference and skips
  the sha passes the suite. That is half of ruling B.
- **The validator's supersession bound is unpinned**; only the schema
  attribute's 8 is, so the two could drift.
- **The `Recorded`/`Withdrawn` reason bound was never pinned**, before or
  after 6c.

And three harness follow-ups: a stale sidecar for a file outside the
current targets is ignored rather than repaired; repair overwrites hand
edits without recording them; a locked target during repair throws a raw
exception. A stated limit, not a defect: widenings of `Promise.label` and
the verdict claim's label fields cannot be reached by a `Bounded` test
because the samples spell them through the newtype (scar 29's class), and
no published schema carries a dot-free pattern, so that rule is Rust-side
only. Self-deferral passes the shape and is admission's. The 9,165 versus
9,807 target-dir discrepancy was files versus paths; paths is the
convention.

**The fix batch landed** at `fcfbda3f` (the three fixtures, entries S2, S14,
S10 killed; 26 tests) and `3f7d58d1` (harness: every sidecar under the repo
root is repaired at startup, not only the current targets; a `-Repo`
parameter, which is Q14's option A; bytes that differ from a sidecar are
saved to `<file>.eureka-mutation-overwritten` with their SHA-256 printed
before the restore; equal bytes say "sidecar matched; nothing to restore";
a locked target prints the harness's own words and keeps the sidecar).
Soul's XA-XD reproduced with those outcomes.

**The Epiphany half of Cut 8 landed** at `a65c6420`: the leaf's `prepare`,
`decode`, `register_pipeline_document_types` and
`validate_pipeline_write_envelope` are live and public, the validator
returns `Result<(), PipelineRefusal>`, the live registrar registers exactly
the thirteen kinds and the test cache registers the stand-in,
`PIPELINE_SCHEMA_EPOCH` exists in code again, `anyhow` and `rmp-serde` are
normal dependencies with the lock unchanged, and
`every_kind_is_at_the_epochs_version` pins the epoch's version against every
type id and the registrar's count. 27 tests, 0 warnings; `epiphany-core`
recompiled nothing; entries E1-E3 killed. Source outside tests net −3.

**Soul's pass** (Fable; `soul-cut8e-*` in the session scratchpad) attacked
the door from an external crate by path: the four public functions and the
epoch constant are reachable; the thirteen wrappers, `Bounded`,
`pipeline_id`, `ForeignDocument` and `derived_schema` are not (E0603,
E0432, E0599); all thirteen kinds round-trip `prepare`, `validate`,
`decode` with the key equal to `pipeline_key`'s; a structural tamper, a
forged key, a foreign type and one kind's type over another's payload each
refuse as named. The validator carries no `anyhow` inside; six direct
dependencies exactly; the lock and the schemas unchanged; the twenty-six
prior test names intact. Every suite killed through the harness, including
the new E1-E3, and its own non-revert mutations died (`.v01`, `epochs.v1`,
one kind at `.v2`, the inverse of S2, the reason bound at 1,001, only the
first supersessor validated). A double registration cannot move the
registrar's count because the cache dedupes, so that mutant is equivalent.
**The Epiphany half of Cut 8 closes; Huginn pins `a65c6420`.** Found, none
touching the leaf's contract:

- **A sidecar whose file is gone crashes the harness with no message, and
  it stays crashed** (medium): PowerShell 5.1 collapses an empty byte array
  from an `if` expression to `$null`, the comparison throws the .NET
  exception, the "could not repair" line sits outside the `try`, and every
  later run in that root dies the same way until a human deletes the
  sidecar. Bytes are never destroyed.
- **A locked target during repair leaves an `.eureka-mutation-overwritten`
  file whose name lies**, since it is written before the failed restore.
- **Seven doc comments describe the organ in the present tense** as a
  consumer that exists, and one claims a Huginn test that does not.
- **`decode` is a door, not a guard**: an envelope built without `prepare`
  is accepted by `decode` and, when the key and fields are valid, by the
  validator too, by design; but `decode` alone bound-checks nothing, and
  its doc does not say so. Cut 9's views and Cut 12's import must not read
  `decode` as validation.
- Reach, not defects: a forged supersessor at index seven is unpinned
  (fixtures forge index one and fail the count at nine); the epoch test
  pins the version suffix only, and the schema-derivation test is what
  kills a renamed type id.

**That batch landed** at `b3bd4a82` (harness: a missing target is its own
case, recreated from the sidecar and said so; a sidecar of a sidecar stops
the run naming both files before any repair; the overwritten copy is
removed again when a restore fails without opening the file; every
per-sidecar body is one `try`/`catch` so no raw exception escapes; Soul's
XA-XK reproduced) and `aef0e1bf` (doc comments: no organ described as
existing; `decode` says it is a typed read, not validation). 27 tests, 0
warnings, lock and schemas unchanged. No separate Soul pass: the next Soul
on Huginn runs this harness.

**The Huginn half of Cut 8 landed** on `eureka/memory-organ` at `946758f`
(store, mind, opener; 4 tests), `a4c5b79` (receipt and commit primitive; 4
tests), `0bd7133` (admission, refusal, the rule table; 26 tests, 0
warnings) and `ca30d3e` (entries H1-H20, all killed through Epiphany's
harness with `-Repo`, four targets, M0 green). `cargo check --workspace`
clean; one `cultcache-rs`, no `epiphany-core`; one inherited `syn`
duplicate the leaf's graph already carries. Target dir +656 paths, +0.39
GB, inside the budget. +3,491 lines over 11 files, +8 direct dependencies,
lock 3 → 93 packages, two Huginn document types, no binaries or targets.
Soul in flight.

Hands' discrepancies, all kept and recorded:

34. **The leaf's `PipelineRefusal` and `PipelineDocument` derive neither
    `Serialize` nor `JsonSchema`**, so `MindRefusal::Document` mirrors the
    refusal through a `#[serde(remote)]` definition compiled against the
    leaf's shape (drift breaks the build), and `PipelineAdmissionBatch`
    cannot derive either. **Cut 10's wire cannot serialise a batch until the
    leaf derives them.** That is an additive leaf change and a Cut 10
    prerequisite; recorded here so Cut 10's refresh carries it.
35. **`FindingWithoutRange` is unreachable**: a rangeless envelope fails the
    leaf's decode first. Variant kept; the test asserts the real refusal.
36. **The epoch record is derived when the batch carries the `instance`
    document**, not when the image is empty, so a replay of the first batch
    is `AlreadyAdmitted` and not a collision.
37. **`Mind::open` recognises the owned store's lock failure by its error
    text**, because the store gives no typed signal. One string match,
    commented; Soul is asked whether any other store failure can be misread.
38. **`receipt.rs` is 277 lines against a 220 cap**, by its tests and the
    two provenance shapes that live beside their only consumer.
39. **`prepare_entry_named` appears once outside `receipt.rs`**, in
    `mind.rs` for the epoch record. The spec's grep is narrowed to that.
40. **`Cargo.lock` was already LF**, so the whole-file rewrite Cut 7's Soul
    predicted did not happen; the lock gained 809 lines.

**Soul's pass on the Huginn half** (Fable; `soul-cut8h-*` and
`soul_probe.rs` in the session scratchpad) held identity at the layer where
it fails: a real redb mind copied under another instance's directory is
`ForeignInstance` and opens under its own name with its receipt; a second
process taking the same path gets `MindAlreadyOwned` in under a
millisecond with the store intact; a directory at the path and a read-only
file are `Unavailable`, never misread as ownership. The digest moves with
the strong reads and not with provenance, `now` or the store's timestamp;
`committed_at` equals the `now` passed in; the refusal mirror serialises
every variant byte-identical to a derive and a new leaf variant breaks the
build. H1-H20 all killed; pins, lock and leafness exact. **Cut 8 does not
close as specified.** Found:

- **The opener's epoch step checks neither the record's key nor its count**
  (medium): it takes the first record; a record keyed otherwise opens, two
  records open, and a current-plus-foreign pair refuses only by sort order.
  The spec said exactly one, keyed by the epoch string. No test pinned it.
- **"Withdrawing a resolution reopens its subject" is unimplementable under
  the key grammar** (medium): a resolution's key is outcome-invariant and
  one per subject, so after a withdrawal every later resolution of the same
  subject is `AlreadyResolved` forever. The sentence was Imagination's, not
  a ruling. Raised as Q17 below with Self's default applied.
- **A withdrawn stewardship can never be regained on the same mind**
  (medium for Cut 12): the derived stewardship keys `{mind}:stewardship:{repo}`
  and collides with the withdrawn one. Spec-consistent as written; raised
  as Q18 below, a key-shape question, before Cut 12.
- **The organ minted a leaf refusal** for an empty `campaign.repos`; the
  leaf's list bound has no minimum and says minimums are admission's. Low.
- **"One CAS" and "strong reads beyond the first" were unpinned** (medium as
  tests): a receipt written in a second swap, and only the first cited
  document pinned, both survived the suite.
- **Fifteen rule rows had no mutation**: stewardship checked against the
  batch only; spec mismatch on repo; promise-label equality; the in-force
  target; the matrix beyond one row per kind; the derived stewardship's
  date; derived writes' own validation; batch-level identity uniqueness;
  batch size; question options; duplicate labels; finding locations;
  answered-coherence. The real code answers every one correctly; the suite
  could not see a regression in any of them.
- **README and AGENTS again describe CultNet publication and a Qdrant
  connection in the present tense**, and README still says `.cc`.
- **`receipt.rs` is 194 non-blank, non-comment lines outside tests**, over
  the 220 cap only with its tests; recorded, the cap was pressure.
- **Stated limit, not a defect:** every stored envelope carries a
  `stored_at` that CultCache's `prepare_entry_named` stamps from the wall
  clock, so "no clock read inside the crate" is true of Huginn's decisions
  and false of the bytes it stores. The receipt names payload bytes and
  their digests, not the envelope's stamp; replay compares payloads; the
  CAS expectation is built from the live image at commit time, never
  reconstructed from a receipt. Cut 9's in-force derivation must not
  depend on `stored_at`. Recorded in the authority map's inputs.
- `Mind::open` recognises the store's lock failure by its error text; a
  CultLib re-pin that rewords it silently demotes `MindAlreadyOwned` to
  `Unavailable`. FU-9.

**The fix batch landed** (Opus) at `1cfa81d` (the epoch gate: exactly one
record, keyed by the epoch string, equal to it, with `found` carrying the
foreign value, the wrong key, or the record count, decided before any
value is read; `MindRefusal::EmptyRepos` in place of the minted leaf
refusal; docs without present-tense claims about unbuilt cuts), `acd32f3`
(Self's Q17 default reverted on the operator's ruling; the matrix and its
test byte-identical to `ca30d3e` plus a doc comment naming the ruling and
the leaf cut it needs) and `30daff8` (a recording store pins one swap
carrying documents and receipt together; every cited image document is a
strong read; one fixture and one entry per unpinned row, Soul's survivors
S8-S23 as entries H26-H39). 31 tests, 0 warnings; H1-H39 all killed
through Epiphany's harness; +525/−26 over seven files, one refusal variant
added. Hands named the gap the ruling leaves until the leaf cut lands: the
matrix now admits a second resolution after a withdrawal and the key still
refuses it as `AlreadyResolved`. **Cut 6d (resolution history, stewardship
by sequence) must land before Cut 9.**

**Soul on the fix batch** (Fable; `soul-cut8f-*` and `soul_probe2.rs` in the
session scratchpad) held every promise: the epoch gate's count-then-key-
then-value order under four non-revert forms; `EmptyRepos` in place of the
minted refusal; the matrix and its test byte-identical to `ca30d3e` after
the revert; one swap and every strong read pinned, with the second-CAS
and emptied-expectation forms dead by the store's own contract; H1-H39
killed; docs on `mind.redb` and the live crate. **Cut 8 closes on the
admission rules as ruled today.** Two findings sharpen what the Cut 6d
Huginn follow-up must do, and it now owns them:

- **Q19 A is not enforced**: a withdrawal of a withdrawal committed, chain
  unbounded, the rejected Q19 B shape as a standing record. The follow-up
  caps the chain at depth two.
- **The Q17 gap has two owners, not one**: after a withdrawal, a re-
  resolution is refused by the key (A10) and also by admission's in-force
  derivation (A8), which still counts the withdrawn resolution. The key
  sequence alone would not reopen a subject; in-force must ignore withdrawn
  resolutions too, as the 6d spec already says.

Test gaps, the real code correct by probe and the suite blind to a
regression, all folded into the follow-up: the in-force half of
`stewardship_of`; a document cited twice pinned zero times; derived writes
skipping A7; the in-force target decided from the image only; `Answered`
coherence only when the ruling is in the batch; branch compared
case-insensitively. Stated limits: the epoch pair's "both orders" pin lives
only in the helper call, because every store returns rows in key order;
a case-insensitive epoch key is equivalent since only the organ writes it.
One doc residue: README still names the two stub crates' purposes in the
present tense.

**The Cut 10 prerequisite landed** at Epiphany `b4b17fc` (correction 34):
`PipelineDocument` derives `Serialize`, `Deserialize` and `JsonSchema` with
an adjacent tag whose string is exactly `PipelineKind::name()`, so a wire
reader dispatches on the same string the key's kind segment carries and no
second registry maps spellings to kinds; `PipelineRefusal` derives the same
three with serde's default external tagging, which already puts every
variant's named parts on the wire. One test round-trips every sample and
every refusal through JSON and MessagePack and holds both schemas to
exactly the thirteen kinds and the four variants; entries D1 (tag renaming
removed) and D2 (a refusal field skipped) killed beside E1-E3. 28 tests, 0
warnings; no kind, field, type id, epoch, schema file, lock or dependency
moved. Huginn's `#[serde(remote)]` mirror comes out in Cut 10 when the pin
moves to this commit.

41. **Suspect, not accepted: `epiphany-core` recompiled once after the leaf
    change.** Hands called it legitimate because "every dependent's
    metadata is stale", but `epiphany-core` is not a dependent of the leaf
    (Cut 6 proved no path in either direction, and Soul's pass on the
    Epiphany half saw it finish in half a second untouched). Either a
    shared target-dir fingerprint was evicted by another session's build,
    or something depends on the leaf that this map says does not. **Self
    checked the second half at once**: `cargo tree --workspace -i
    epiphany-pipeline -e normal,build,dev` lists the leaf alone, so no
    workspace crate depends on it in any kind. What remains is the shared
    target dir: another session was building Ghostlight into it during this
    pass, and a fingerprint evicted by a different environment or profile
    reads as a "recompile" of an untouched crate. That is the build-economy
    cost of one shared `CARGO_TARGET_DIR` across sessions, already accepted
    (correction 22); not a dependency, and not Hands' explanation either.

## Cut 6d. Resolution history and stewardship by sequence

Imagination, 2026-09-16, revised the same day for Q19 A and Q20 A.
Implements Q17 B, Q18 A as corrected, Q19 A and Q20 A: **a hand-off is a
transfer of stewardship, symmetric and final as a record; a lease is two
transfers and nothing more** (no lease, expiry, return or intent field
anywhere); **a withdrawal cannot be withdrawn**; and **stewardship is keyed by
a per-(instance, repo) sequence, not by date**, exactly as resolutions are
keyed by a per-subject sequence.

- **Repo/branch:** Epiphany `codex/eureka-pipeline-state`. Anchors are
  `file:line` against `epiphany-pipeline/src/lib.rs` at **`b4b17fc`** (2,099
  lines, 28 tests), byte-identical at HEAD `542dc184` (`git diff --stat
  b4b17fc HEAD -- epiphany-pipeline/ schemas/cultnet/ tools/` empty, probed
  twice). HEAD moved three times during this pass, the map only; re-anchor by
  content as every cut since 6b has.
- **Depends on:** nothing unlanded. **Blocks:** Cut 9 (in-force derivation),
  Cut 12 (hand-off), and a Cut 8 follow-up commit in Huginn that this cut
  implies (below). Lands before any of them.
- **Cost:** two kinds' keys move (`resolution`, `stewardship`); two fields are
  added (`resolution.sequence`, `stewardship.sequence`); two schema files
  regenerate; no epoch moves; no kind widens. Keys are free to move today: no mind exists on Yggdrasil.
  Huginn pins `a65c6420` (`crates/huginn-mind/Cargo.toml:15`) and its fixtures
  spell both old shapes, so the pin move is a named follow-up, not a surprise.

### What the cut does

**Q17 B.** A resolution is the record of how a subject was closed and by
what. Its key gains a per-subject sequence as the **last** local part:
`<subject root>:resolution:<subject kind>.<subject local>.n<N>`. Withdrawn
resolutions stay under their subject as records; the subject can be resolved
again at `n<N+1>`; the history is the key prefix
`<root>:resolution:<kind>.<local>.n`.

**Q18 A + Q20 A.** A stewardship is keyed by repo and a per-(instance, repo)
sequence: `<instance>:stewardship:<repo escaped>.n<N>`. A repo transferred
away and later transferred back to the same mind is two records under one
prefix, `<instance>:stewardship:<repo escaped>.n`, and the in-force one is
the latest not withdrawn. `assigned_on` stays a field, validated as a `Date`
by the derived `Bounded` impl, and no longer enters the key.

Both are key-shape changes in the leaf plus one field each. Everything semantic
(what "in force" means, what `AlreadyResolved` refuses, how a transfer derives
its writes) is admission's and is specified here for the Huginn follow-up, not
built here. The leaf gains no cross-field rule (authority map, Cut 6b/6c).

### Decisions, with the reasons

#### D1. The sequence is a field the writer sets; admission checks it

This decision is made once and applied to both kinds: `resolution.sequence`
is per subject, `stewardship.sequence` is per `(instance, repo)`. The leaf
holds no image (`pipeline_key`'s inputs are "the document's own
fields. No clock, store, registry or counter", map Cut 6b authority map), so
the only way the sequence reaches the key is a field. Admission cannot set it
either: `admit_prepared` takes envelopes "prepared by anyone" and the receipt
digests the exact payload bytes (`receipt.rs`, map Cut 8 A9/A11), so a field
rewritten by admission would make Cut 12's import and exact replay
impossible. Precedent already in the leaf: `revision: u32` on `target` and
`cut_spec` is writer-set and keyed `r<N>`, and Huginn's `revision_rule`
(`admission.rs:605-622` at `1cfa81d`) checks it against the image. The
sequence follows the same pattern exactly.

- **Fields:** `sequence: u32` on `PipelineResolution` after `subject`, and
  `sequence: u32` on `PipelineStewardship` after `repo`.
- **Bound:** none in the leaf beyond `u32` and `LOCAL_MAX`; `n4294967295` is
  an 11-byte label. The leaf refuses neither `0` nor a gap, as it refuses
  neither `revision: 0` nor `revision: 7` without a predecessor; those are
  admission's (`revision_rule` refuses 0). Stated in both fields' doc comments
  and pinned as a stated limit in the new tests.
- **Admission's rule (follow-up), the same for both:** `sequence ==
  latest + 1`, where `latest` is the greatest `sequence` among the records of
  the same scope (resolutions of that subject; stewardships of that
  `(mind, repo)`) in image ∪ batch other than this document; and no record of
  that scope is in force other than this document. Refusals below.
- **Derived writes** (a ruling's `Answered` resolution; on a transfer, the
  source mind's `Withdrawn` resolution of its stewardship and the receiving
  mind's new stewardship) get their `sequence` from `derive()` the same way,
  since `derive` already reads `docs` (`admission.rs:417-458` at `30daff8`).

#### D2. Spelling: `n<N>`, the last part

Every numeric key part the leaf has carries a one-letter marker (`r2`, `h1`,
`s2`, `:724,:731,:739,:748`), and `parent_cut` reads the marker to tell a
spec's tail from a report's. The sequence takes `n` ("the nth resolution").
Nothing parses it back: admission reads `sequence` from the document, never
from the key, and the reader `pipeline_id` stays one shape with no `match
kind` (R1, R2 hold; `:579-599` unchanged). The marker buys legibility and the
house convention for one byte per nesting; the alternative, bare digits, was
considered and rejected because a key a human reads in a Studio row should
say what its tail is.

The sequence is the **last** part so that the subject's resolutions, and only
they, share the prefix `<root>:resolution:<kind>.<local>.n`. Under R2 no part
carries a dot, so `question.Q1.n` cannot match `question.Q10.n1`, and the
withdrawal of `Q1`'s first resolution keys under `resolution.question.Q1.n1.n1`,
outside the prefix. The same spelling and position serve stewardship: the
history of a repo on a mind is the prefix `<instance>:stewardship:<repo
escaped>.n`, and `key_segment` escapes `.` to `_d`, so an escaped repo can
never end in `.n`. Both prefix properties are pinned by leaf tests (below) and
are what Cut 9's history queries ride on.

#### D3. Nesting arithmetic changes

A depth-one local is `<kind>.<subject local>.n<s>`. Each nesting prepends
`resolution.` (11 bytes) and appends `.n<s>` (3 bytes for one digit), so a
chain `d` deep composes `14 * (d - 1) + L1` bytes, `L1` the depth-one local.
At `LOCAL_MAX = 64`: `ruling.A.n1` (11 bytes) and `question.Q1.n1` (14 bytes)
both key **four** deep and refuse at five (53 and 56 fit; 67 and 70 do not).
The bound stays the local's alone (`local`, `:673-682`); no guard is added
(Q11 A). The depth test derives its expectation from `LOCAL_MAX` and the two
literal widths, so the numbers are not restated.

A stewardship local is `<repo escaped>.n<N>`, so the escaped repo has
`LOCAL_MAX - 3` bytes for a one-digit sequence (61), not 64. It does not
nest. The resolution of a stewardship is `<instance>:resolution:stewardship.<repo
escaped>.n<N>.n<M>`, two sequence parts, the inner one the stewardship's; a
withdrawal of that is one more `resolution.` and `.n1`, and Q19 A stops the
chain there.

#### D4. What "withdrawn" means, and what `AlreadyResolved` becomes

Documents are immutable and status is derived (Cut 9: "status is derived at
read time, never stored"), so the only way a resolution stops counting is
another document naming it: **a resolution of the resolution with outcome
`Withdrawn`**, the matrix row Cut 8 first specified ("resolution: `Withdrawn`
only: withdrawing a resolution reopens its subject", map `:2376`) and Soul
found unimplementable under the outcome-invariant key. The sequence makes it
implementable. Huginn's landed fix batch already restored that row
(`acd32f3`, `admission.rs:673` at `30daff8`: `(K::Resolution, O::Withdrawn {
.. }) => true`) and its doc comment says the sequence "is mapped separately";
this is that map.

In-force, one derivation for every kind, recursive and well-founded:

> `in_force(kind, id)` holds when no resolution `R` with `R.subject == (kind,
> id)` exists in image ∪ batch such that `in_force(Resolution, key(R))`.

`key(R)` is strictly longer than `id`, so the recursion descends a bounded
chain (D3) and terminates. Today's `in_force` (`admission.rs:255-267`) is the
non-recursive special case that counts a withdrawn resolution as if it still
stood; that is the exact defect Soul named.

The in-force resolution of a subject is then "the latest not withdrawn". The
two readings of that phrase ("the latest, if not withdrawn" and "the latest
among the not-withdrawn") coincide **only if a withdrawal cannot itself be
withdrawn**: otherwise `n1` withdrawn, `n2` admitted, then `n1`'s withdrawal
withdrawn leaves two resolutions standing. **Q19, ruled A:** the chain is capped at admission (a resolution
of a resolution is admissible only when that resolution's own subject is not
a resolution), so reinstating a withdrawn closure is done by resolving the
subject again at `n<N+1>`, which is what "keep a log, attached to the
subject" asks for. Under A the two readings coincide and "the latest not
withdrawn" is exact.

`AlreadyResolved { subject }` moves from A10 to A8:

- **A8, resolution row:** the subject has an in-force resolution other than
  this document → `AlreadyResolved { subject }`. "Other than this document"
  is content identity (the same `subject`, `sequence`, `outcome`, `rationale`,
  `resolved_on`), so an exact replay passes A8 and reaches A9's
  `AlreadyAdmitted`, as the ruling row's `own` closure already arranges
  (`admission.rs:496-501`). A wrong sequence → `ResolutionOutOfSequence {
  subject, expected, actual }` (new `MindRefusal` variant, Huginn's enum, not
  the leaf's).
- **A10:** the `Resolution` arm of `refuse_collisions` (`admission.rs:727-729`
  at `30daff8`) is deleted. A resolution key is no longer one-per-subject, so a collision on
  it means what it means for every other kind: that identity is taken.
  `IdentityCollision` names it. The map's A10 sentence "a subject resolves at
  most once because its resolution key is outcome-invariant" is retired.
- **The ruling row** keeps `AlreadyResolved` (a ruling answering a question
  whose resolution is in force); the derived `Answered` resolution's sequence
  is `latest + 1`, so a question withdrawn-and-reopened can be answered again.

#### D5. Stewardship: keyed by sequence, in force = the latest not withdrawn

- **Key:** `local(&key_field, &[&key_segment(&value.repo.0), &sequence])`
  with `let sequence = format!("n{}", value.sequence);`, after the existing
  `org_repo_text("stewardship.repo", …)?`. `assigned_on` does not enter the
  key, so the arm validates nothing about it; the field is validated where
  every field is, by the `Bounded` impl `value_types!` emits (`:248-253`),
  and `validate_pipeline_write_envelope` runs that before the key check.
  Q20's first draft put the date in the key and a `validate` call in the arm;
  both are gone, and no test may assert a date-shaped refusal from
  `pipeline_key`.
- **Date-invariance:** two stewardships of one repo with one sequence and
  different `assigned_on` key once, exactly as two resolutions of one subject
  with one sequence and different outcomes key once. Pinned in the new test.
- **Escaped repo bound:** the local is `<repo escaped>.n<N>`, so the escaped
  repo has 61 bytes for a one-digit sequence. `a_composed_local_is_bounded_whole`'s
  stewardship boundary moves with it (below).
- **In force (follow-up), mirroring the resolution row exactly:** `sequence
  == latest(mind, repo) + 1` → else `StewardshipOutOfSequence { repo,
  expected, actual }`; and no in-force stewardship of `(mind, repo)` other
  than this document → else `AlreadyStewarded { repo }`. Both are new
  `MindRefusal` variants; the first is the sequence rule the coordinator
  named, the second is the in-force half `AlreadyResolved` plays for
  resolutions, and it is the one Cut 12's transfer-back depends on (a repo
  cannot be assigned twice while one assignment stands). Today the row is
  the key collision (`:2362`), which a sequenced key no longer provides.
  With both, at most one stewardship of a repo is in force on a mind, so
  "the latest not withdrawn" is "the one not withdrawn" and `stewardship_of`
  (`admission.rs:271-284` at `30daff8`) keeps returning `find_map` of the
  in-force one with no tie-break.
- **Same-day limit, retired.** With the date out of the key, a repo
  transferred away and back to the same mind on one day is `n1` withdrawn and
  `n2` standing; nothing collides. The hand-off key itself still carries
  `handed_on` (`:721`), so two hand-offs in one direction for one repo on one
  day still share a key; that is the pre-existing hand-off shape, which this
  cut does not touch, recorded under Findings.

#### D6. A hand-off is a transfer; the derivations are symmetric already

Operator, 2026-09-16: "We might start with, say, Odin as steward over a whole
swarm of infra tools, and spin off a new steward only when the workload
justifies it. Odin wouldn't be getting it back in that case."

`derive` at `admission.rs:433-452` (`30daff8`) reads only `from_instance ==
mind` (withdraw the in-force stewardship) and `to_instance == mind` (assign a
new one, `assigned_on: handed_on`). A transfer back is a hand-off with the
instances swapped; the source side withdraws the transferee's stewardship
(a resolution at that stewardship's next sequence), the receiving side
derives a new stewardship at `latest(mind, repo) + 1`, which no longer
collides with the withdrawn original. **No field names a return, and none is
added.** `hand_off` is unchanged in shape and key; `PipelineHandOff`
(`:374-377`) does not move and `hand_off.v1.schema.json` does not regenerate.

What changes for Cut 8's per-kind rules: the hand-off row's two derivations
each set a `sequence` (`derive` computes both from `docs`), and the derived
stewardship's key is `<mind>:stewardship:<repo escaped>.n<N>`. What changes for Cut 12's
spec: the sentence "with the superseding `stewardship` on the source side"
(map `:2707-2709`) is wrong at HEAD and was before this cut — Cut 8 derives a
`Withdrawn` resolution on the source side, not a supersession — and one test
is added, `a_repo_transferred_back_is_stewarded_again` (transfer A→B, then
B→A; A's in-force stewardship is `n2`; `n1` is still readable by key; the
prefix `A:stewardship:<repo escaped>.n` yields both). Cut 12 models no return
specially.

### Deletes first

| Path | Lines | What dies |
|---|---:|---|
| `epiphany-pipeline/src/lib.rs` | 0 | Nothing. The cut adds one field and two key parts; the resolution and stewardship arms are already one exit each through `local`, and the cut changes what they compose, not how. |
| Huginn `crates/huginn-mind/src/admission.rs:727-729` at `30daff8` (follow-up, not this commit) | 3 | `refuse_collisions`'s `Resolution` arm. Goes before any new rule is added in that commit. (The matrix's Q17 A paragraph already died at `acd32f3`.) |

An honest zero in the leaf, not an omission.

### Keeps and moves

- **Keeps:** thirteen kinds, `PipelineKind`, every type id, the epoch
  constant (`:667`), `pipeline_id` (`:579-599`) untouched, `local`,
  `key_segment`, `ROOT_LOCAL`, `LOCAL_MAX`, `parent_local`, `parent_cut`,
  `PipelineRef`, `ResolutionOutcome` and its `Bounded` impl, the hand-off arm
  (`:714-723`) and key, `PipelineHandOff` and `PipelineStewardship.assigned_on`
  as fields, the outcome-invariance assertion (`:1102-1109`: the sequence is
  the same across outcomes, so the assertion stays true as written), every
  `value_types!` and `pipeline_kinds!` entry,
  `validate_pipeline_write_envelope`, all twenty-eight test names.
- **Moves:** the resolution sample key (`:941`), the stewardship sample key
  (`:951`), and every literal resolution or stewardship key in the tests
  (listed per line below). The nested-depth arithmetic in
  `a_resolution_of_a_resolution_reads_back` and the stewardship boundary in
  `a_composed_local_is_bounded_whole` are key-shape assertions and move with
  the shape; both are named below with the exact new expression.
- **Re-anchor, not move:** `tools/eureka-cut6b-mutations.psd1` M2 pins the
  exact `let parts = …` line of the resolution arm; that line changes, so M2's
  `Old` and `New` are re-anchored (below). Precedent: N7/N8 re-anchored Cut
  6's M12/M13 in the same file.

### Adds

| Add | Owner | Live consumer | Protected invariant | Why an existing owner cannot serve |
|---|---|---|---|---|
| `PipelineResolution.sequence: u32` | the leaf (shape and key) | `pipeline_key`'s resolution arm now; `huginn-mind`'s resolution row and `derive` in the follow-up | A subject's resolutions are distinct records with a total order the key carries, so a withdrawn one is kept and the next one is nameable (Q17 B) | The key must come from fields (D1); no existing field distinguishes two resolutions of one subject. |
| `n<N>` as the last part of a resolution local | `pipeline_key` | the same | History is one key prefix per subject (D2) | The composer has no other path to a key. |
| `PipelineStewardship.sequence: u32` | the leaf | `pipeline_key`'s stewardship arm now; `huginn-mind`'s stewardship row and `derive` in the follow-up | A repo's stewardships on a mind are distinct records with a total order the key carries, so a transferred-away repo can come back as a new record (Q18 A, Q20 A) | No existing field distinguishes two assignments of one repo to one mind; the date was rejected (Q20) because same-day records would collide and a date orders nothing the sequence does not. |
| `n<N>` as the last part of a stewardship local | `pipeline_key` | the same | History is one key prefix per `(instance, repo)` | The composer has no other path to a key. |
| Three tests and `tools/eureka-cut6d-mutations.psd1` (M23-M27) | the leaf's suite | the shared harness | Each rule above has a runtime mutation | — |

No kind, module, dependency, target, format or epoch. No type outside
`value_types!`.

### Per-file changes, `epiphany-pipeline/src/lib.rs` at `b4b17fc`

| Line | Change |
|---|---|
| `:16-20` | Module doc gains one sentence: "A resolution's local ends in its per-subject sequence, `n<N>`, and a stewardship's in its per-repo sequence, so a subject's resolutions and a repo's assignments each share a prefix." |
| `:363` | `pub struct PipelineResolution { subject: PipelineRef, sequence: u32, outcome: ResolutionOutcome, rationale: Para, resolved_on: Date }`, with a doc comment: the record of how a subject was closed and by what; `sequence` is per subject and set by the writer, `1` for the first; whether it is the previous plus one, and whether an earlier one still stands, are admission's. |
| `:369-371` | `pub struct PipelineStewardship { instance: Slug, repo: OrgRepo, sequence: u32, assigned_on: Date, note: Line }`; doc: "an assignment of a repo to a mind; `sequence` is per `(instance, repo)`, set by the writer, `1` for the first, so a repo transferred away and back is two records under one prefix. `assigned_on` is a field, not a key part." |
| `:695-701` | Rewrite the arithmetic comment: each nesting prepends `resolution.` and appends `.n<s>`, `14 * (n - 1) + L1` bytes against `LOCAL_MAX`. |
| `:702-707` | Resolution arm: `let sequence = format!("n{}", value.sequence);` before `let parts = …`; the parts line becomes `let parts = std::iter::once(value.subject.kind.name()).chain(subject_local.split('.')).chain(std::iter::once(sequence.as_str())).collect::<Vec<_>>();`. The tuple line `(field, subject_root, local(&key_field, &parts)?)` is **byte-identical** (M3 anchors it). |
| `:710-713` | Stewardship arm: `org_repo_text("stewardship.repo", &value.repo.0)?; let sequence = format!("n{}", value.sequence);` then `local(&key_field, &[&key_segment(&value.repo.0), &sequence])`. No validation of `assigned_on` here. |
| `:937-941` | Resolution sample gains `sequence: 1`; expected key `format!("{CAMPAIGN}:resolution:question.Q1.n1")`. |
| `:948-951` | Stewardship sample gains `sequence: 1`; expected key `format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1")`. |
| `:1251` | `ruling.R8` → `ruling.R8.n1`. |
| `:1270` | `question.Q1` → `question.Q1.n1`. `:1272` (`{CAMPAIGN}:resolution:R8` validates as well-formed grammar) unchanged; `:1274-1284` refused list unchanged. |
| `:1296` | `campaign.self` → `campaign.self.n1`. |
| `:1413`, `:1428`, `:1432` | Stewardship keys gain `.n1`. `:1436-1441` (no org) and `:1443-1452` (60-byte name still refused, now at 73 bytes) unchanged. |
| `:1530` | The nested probe's subject id: `question.Q1` → `question.Q1.n1` (a real resolution key; the key composed is `resolution.question.Q1.n1.n1`). |
| `:1616-1620` | `campaign.self` → `campaign.self.n1` in both the key and the read-back tuple. |
| `:1824-1831` | Doc comment: the new arithmetic and depths (four for both fixtures at 64). |
| `:1840` | Expected `resolution.question.Q1.n1.n1`. |
| `:1841-1843` | Recovery: `let (subject_kind, rest) = local.split_once('.')`; `let (subject_local, _sequence) = rest.rsplit_once('.')`; assert `format!("{root}:{subject_kind}:{subject_local}") == inner`. |
| `:1846` | `let nesting = "resolution.".len() + ".n1".len();` The loop and its two assertions are otherwise unchanged; `deepest` is still derived from `LOCAL_MAX`. |
| `:1886-1912` | The stewardship boundary: `let stewardship_rest = "GameCult_-".len() + ".n1".len();` and `stewarded(64 - stewardship_rest)`, `stewarded(65 - stewardship_rest)`. Comment says the local is `<repo escaped>.n<N>`. The hand-off half is untouched. |
| tests, after `:1938` | Three new tests, under Verification. No sample is added, so every `samples().remove(N)` index is unchanged. |

`tools/eureka-cut6b-mutations.psd1:38-39` (M2): `Old` becomes the new parts
line above; `New` becomes `let parts = subject_local.split('.').chain(std::iter::once(sequence.as_str())).collect::<Vec<_>>();`
(the kind dropped, the sequence kept, so the mutant still compiles and still
keys `question.Q1` and `ruling.Q1` to one key, which
`a_resolution_names_its_subjects_kind` kills as before). No other 6b, 6c, 8e
or D entry anchors a line this cut touches (read: M1, M3-M6, S2, S4, S5, N3,
N6-N10, X1, X11 anchor `local`, `key_segment`, `pipeline_id` and the campaign
arm; 6c anchors `ResolutionOutcome`'s impl, `MutationRecord`, `VerdictClaim`
and the estimate literal).

`schemas/cultnet/`: **exactly two files regenerate.**
`epiphany.pipeline.resolution.v1.schema.json`: `properties` gains
`"sequence": { "format": "uint32", "minimum": 0, "type": "integer" }` (the
shape `revision` already has in `epiphany.pipeline.target.v1.schema.json:101-105`)
and `required` (`:219-224`) gains `"sequence"` after `"subject"`.
`epiphany.pipeline.stewardship.v1.schema.json`: the same property, and
`required` (`:37-42`) gains `"sequence"` after `"repo"`. Derived, never
hand-written: `pipeline_published_schemas_match_derivation` writes the
derivation to a per-run temp directory and names it; copy from there.
`epiphany.pipeline.hand_off.v1.schema.json` is **byte-identical**: no field
moves on `PipelineHandOff`, and keys are not in schemas. The other ten and
`index.json` (no hashes) are byte-identical. **No epoch moves.** Honesty
about "additive": the epoch constant's doc comment (`:660-666`) says additive
means "a new named field with a serde default", and neither `sequence` has
one — a default of `0` would key a document to `n0`, which admission refuses,
so the default would be a lie.
This lands without a bump on the same ground Cut 6c's four required fields
did: no store is written at this epoch and no reader is pinned to the file
(Huginn pins the crate rev, not the schema). Recorded under Findings as the
rule the campaign will owe an epoch to once a mind exists.

`git diff --stat b4b17fc -- schemas/cultnet/` names exactly the two files.

### Authority map

- **Owner:** `pipeline_key` (`:687`), through `local`, for both keys. Unchanged
  owner; changed inputs.
- **Inputs:** the document's own fields, now including `resolution.sequence`
  and `stewardship.sequence`. `stewardship.assigned_on` is **not** a key
  input. No clock, store, registry or counter. A sequence is a field the
  writer sets, never derived here.
- **Outputs:** one `String` or a `PipelineRefusal` naming the field
  (`resolution.key`, `stewardship.key`, `stewardship.repo`).
- **Derived state:** the key, never stored. The history of a subject is a
  key prefix, not a list anything maintains. In-force status, the sequence
  rule, the withdrawal chain and the single-in-force-stewardship rule are
  admission's, computed at rule time, never stored (Cut 8/9), and are **not
  in this crate**.
- **Forbidden writers:** the leaf may not check either `sequence` against
  anything (not `> 0`, not a gap), may not refuse a nested resolution, may
  not decide which stewardship is in force, may not put `assigned_on` (or any
  date) into the stewardship key, and may not parse a sequence back out of a
  key. `pipeline_id` gains no arm. Nothing may put a sequence anywhere but
  last. Admission (Huginn) may not set or rewrite `sequence` on a submitted
  document; it computes it only for the writes it derives.
- **Shared paths:** the four grammar paths of 6b, unchanged:
  `pipeline_key`, `pipeline_id`, `PipelineRef::validate`,
  `validate_pipeline_write_envelope`. Huginn's `admit` and `admit_prepared`
  converge before A1 and see the new keys through A3 with no new code.
- **Deletion line:** none in the leaf. In the follow-up: `refuse_collisions`'s
  `Resolution` arm and the matrix's Q17 A default go before any new rule is
  added.

### Verification

**Builds.** `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`, path-list
baseline before and after. `cargo check -p epiphany-pipeline --lib --tests`;
`cargo test -p epiphany-pipeline --lib` (28 existing + 3 = **31 tests**, 0
warnings); `cargo check -p epiphany-core --lib --tests` (untouched; `cargo
tree --workspace -i epiphany-pipeline -e normal,build,dev` lists the leaf
alone, map correction 41, so no recompile is legitimate). Host = target =
workstation; the leaf has no platform code.

Baseline probed this pass in a detached worktree at `b4b17fc` under the
scratchpad: 28 passed, 0 failed, 15 s wall with warm rlibs; worktree removed.

**Tests.** Existing twenty-eight unchanged in name; assertions unchanged
except the key-shape moves named per line above.

| Test | Rule it pins |
|---|---|
| `a_subject_keeps_every_resolution_it_had` | Q17 B in the key: two resolutions of `question:Q1` with `sequence` 1 and 2 key to `…question.Q1.n1` and `…n2`, `assert_ne`; each reads back through `pipeline_id` with the local's last part `n<sequence>`; the same document with `sequence` 1 under `Answered` and `Withdrawn` keys once (outcome-invariance holds per sequence). Stated limits: `sequence: 0` composes `n0` and `u32::MAX` composes `n4294967295` (21-byte local under `ruling.A`); the leaf refuses neither, admission does. |
| `a_subjects_resolutions_share_a_prefix_no_other_key_has` | D2: over the keys of `Q1` n1..n3, `Q10` n1, `ruling:Q1` n1, the campaign-root resolution n1, and the withdrawal of `Q1.n1` (`resolution.question.Q1.n1.n1`), `starts_with("{CAMPAIGN}:resolution:question.Q1.n")` selects exactly the three. Pinned as a string property, which is what a prefix query is. |
| `a_repo_keeps_every_stewardship_it_had` | Q18 A + Q20 A in the key: one repo with `sequence` 1 and 2 keys to `…GameCult_-Epiphany.n1` and `…n2`, `assert_ne`; each reads back through `pipeline_id` with the last part `n<sequence>`; the same document with `sequence` 1 on `2026-09-15` and on `2026-09-16` keys **once** (date-invariance: `assigned_on` is a field, not a key part); over the keys of `GameCult/Epiphany` n1..n2, `GameCult/Epiphany_thing` n1 and `GameCult/Huginn` n1, `starts_with("{INSTANCE}:stewardship:GameCult_-Epiphany.n")` selects exactly the two. Stated limit: `sequence: 0` composes `n0`; the leaf refuses nothing on the value. |

**Negative greps over `epiphany-pipeline/src`:** `rg -n "sequence"` shows the
two fields, the two arms, docs and tests and **no** `if`, `match`, `==`, `>`
or `+ 1` on either outside `tests`; `rg -n "assigned_on" -- src/lib.rs`
shows the field, the samples, the derived stewardship nowhere (the leaf
derives nothing) and **no** occurrence inside `pipeline_key`; `rg -n "match kind|declared_kind|KeyParts"` empty;
`rg -n "fn pipeline_id" -A 20` shows no change (`git diff b4b17fc --
epiphany-pipeline/src/lib.rs` has no hunk in `:572-599`); `git diff --stat
b4b17fc -- schemas/cultnet/` names exactly two files; `rg -n '"sequence"'
schemas/cultnet/epiphany.pipeline.{resolution,stewardship}.v1.schema.json`
matches twice in each (property and required) and `rg -n '"sequence"'
schemas/cultnet/epiphany.pipeline.hand_off.v1.schema.json` is empty;
`rg -n "lease|expiry|returns_to|intent"` empty.

**Suites that must still kill every entry:** `eureka-cut6b-mutations.psd1`
(seventeen entries, M2 re-anchored), `eureka-cut6c-mutations.psd1` (M16-M22),
`eureka-cut8-epiphany-mutations.psd1` (E1-E3, D1-D2), `eureka-cut5-`, `eureka-cut6-`
if still run per the harness header, and the new `eureka-cut6d-mutations.psd1`.
All through `tools/eureka-mutations.ps1` with M0 green.

#### Mutations, `tools/eureka-cut6d-mutations.psd1`

Each restores the old permissiveness of the shape it pins. Anchors are
content Hands lands, matched exactly once.

| # | Rule | Mutation, exactly | Killed by |
|---|---|---|---|
| M23 | A subject's resolutions are distinct records | Resolution arm: `.chain(std::iter::once(sequence.as_str()))` → removed (the old one-per-subject key) | `a_subject_keeps_every_resolution_it_had` (n1 and n2 key equal); collaterally every moved sample key |
| M24 | A repo's stewardships on a mind are distinct records | Stewardship arm: `&[&key_segment(&value.repo.0), &sequence]` → `&[&key_segment(&value.repo.0)]` (the old one-per-repo key) | `a_repo_keeps_every_stewardship_it_had` (n1 and n2 key equal); collaterally every moved stewardship key |
| M25 | The stewardship sequence is the last part | Stewardship arm: `&[&key_segment(&value.repo.0), &sequence]` → `&[&sequence, &key_segment(&value.repo.0)]` | the same test's prefix assertion (`n1.GameCult_-Epiphany` leaves the prefix) |
| M26 | The sequence is the last part | Resolution arm: sequence chained **before** the subject local (`once(kind).chain(once(sequence)).chain(local parts)`) | `a_subjects_resolutions_share_a_prefix_no_other_key_has` (`question.n1.Q1` leaves the prefix) |
| M27 | The nested subject is recovered whole, sequence stripped | In the resolution arm, `subject_local.split('.')` → `subject_local.rsplit_once('.').map_or(subject_local, \|(head, _)\| head).split('.')` (drop the subject's own sequence when nesting) | `a_resolution_of_a_resolution_reads_back` (the recovered subject is `question.Q1`, not the inner key) and `every_key_has_exactly_three_segments`'s nested probe |

Stated limits: the `n` marker is pinned only by the sample-key equalities in
`keys_are_derived_and_mismatch_refuses` and the two prefix literals, a
fixture pin like 6c's M21; a marker change is a spelling change and no
mutation restores a permissiveness by it. The `u32` type has no runtime
mutation. Date-invariance has no mutation of its own: putting `assigned_on`
into the key is not a permissiveness the old code had, and M24 already pins
that the second part is the sequence and nothing else (a mutant that appends
the date as a third part dies on M24's test's `assert_eq` of the local's
part count, which Hands asserts as `2`).

**Operator checks before landing:** none. Q19 and Q20 are ruled.

### The Cut 8 follow-up commit this cut implies

Huginn `eureka/memory-organ`, one commit after **`30daff8`** (the fix batch
landed as `acd32f3`, which restored the `resolution → Withdrawn` matrix row
and flipped the nested assertions, and `30daff8`, entries H21-H39; anchors
below are `file:line` at `30daff8`). Pinned rev moves from `a65c6420` to this
cut's landing commit, which also carries `b4b17fc`'s derives: the
`#[serde(remote)]` mirror of `PipelineRefusal` in `refusal.rs` can come out
then, but that is Cut 10's stated work and not this commit's.

| File | Change |
|---|---|
| `crates/huginn-mind/Cargo.toml:15` | `rev` → the 6d landing commit. |
| `Cargo.lock` | the `epiphany-pipeline` source line. |
| `src/refusal.rs` | `ResolutionOutOfSequence { subject: String, expected: u32, actual: u32 }`, `StewardshipOutOfSequence { repo: String, expected: u32, actual: u32 }`, `AlreadyStewarded { repo: String }`. `AlreadyResolved { subject }` kept. The two `OutOfSequence` variants are the sequence rule for each kind; `AlreadyResolved`/`AlreadyStewarded` are the in-force half. |
| `src/admission.rs:23-26` | Module doc: "in force" is recursive: no resolution names the document that is itself in force. |
| `:248-267` | `resolutions()` yields `(key, &PipelineResolution)`; `in_force_unless` becomes the recursion of D4, with `own` taking the whole `&PipelineResolution` (content identity) rather than the outcome. Add `latest_resolution(subject) -> u32` and `latest_stewardship(mind, repo) -> u32` over image ∪ batch (0 when none). |
| `:271-284` | `stewardship_of` unchanged in shape; its result is now unique by the `AlreadyStewarded` rule. |
| `:417-458` | `derive`: the ruling's `Answered` resolution and the source side's `Withdrawn` resolution each get `sequence: latest_resolution(subject) + 1`; the receiving side's stewardship gets `sequence: latest_stewardship(mind, repo) + 1` with `assigned_on: handed_on` as before. Keys follow from the leaf, no other code. |
| `:591`, new `resolution_rule` steps | Before the matrix: `sequence` rule (`ResolutionOutOfSequence`), then in-force (`AlreadyResolved`). After the matrix, Q19 A: a subject of kind `Resolution` whose own subject is a `Resolution` → `IncompatibleResolution`. |
| `:598` | Stewardship row, mirroring the resolution row: `sequence == latest_stewardship + 1` else `StewardshipOutOfSequence`; then no in-force stewardship of `(mind, repo)` other than this document, else `AlreadyStewarded`. |
| `:654-680` | `matrix`: already restored at `acd32f3`; unchanged. |
| `:724-733` | `refuse_collisions`: delete the `Resolution` arm (`:727-729`); one refusal, `IdentityCollision`. |
| `src/fixtures.rs:100-107`, `:263-270` | `stewardship(instance, repo)` sets `sequence: 1`; add `stewardship_n(instance, repo, sequence)`; `resolution(subject, outcome)` sets `sequence: 1`; add `resolution_n(subject, sequence, outcome)`. |
| Tests moving | `two_minds_in_one_state_root_stay_separate:800`, `the_resolution_matrix_is_admissions:1024,:1042`, `a_ruling_answering_a_question…:1132,:1135`, `a_hand_off_derives_this_minds_side_only:1427,:1448`, `id_of:1474-1477` (appends `.n1`): every `…:stewardship:GameCult_-Epiphany` gains `.n1` and every `…:resolution:question.Q1` gains `.n1`. Fixtures in `:52-116` of the landed batch (H21-H39's tests) that spell either literal move the same way; grep `stewardship:GameCult\|resolution\", \"question.Q1\"` after `30daff8` and re-anchor. |
| `subject_resolves_at_most_once:977` | Renamed `a_subject_with_a_resolution_in_force_refuses_another`: second resolution of `Q1` at `sequence: 2` → `AlreadyResolved`; at `sequence: 1` → `ResolutionOutOfSequence { expected: 2, actual: 1 }`; the answering ruling → `AlreadyResolved` (unchanged). |
| new `a_withdrawn_resolution_reopens_its_subject_and_stays_readable` | `Q1` resolved n1; withdrawal of `…question.Q1.n1` (`Withdrawn`) lands; `Q1` resolved n2 lands; `get` of n1 still returns it; a withdrawal of the withdrawal → `IncompatibleResolution` (Q19 A); a third resolution at n3 while n2 stands → `AlreadyResolved`; a second answering ruling after the withdrawal derives `…question.Q1.n2`. |
| new `a_repo_is_stewarded_once_at_a_time_and_again_after_a_transfer` | second stewardship of `REPO` at `sequence: 2` while n1 stands → `AlreadyStewarded`; at `sequence: 1` → `StewardshipOutOfSequence { expected: 2, actual: 1 }`; after a hand-off away, a hand-off back (swapped instances) lands and derives `<mind>:stewardship:GameCult_-Epiphany.n2` with `assigned_on` the second `handed_on`; `envelope` of n1 still present; the source mind's derived withdrawal is `…:resolution:stewardship.GameCult_-Epiphany.n1.n1`. |
| `tools/eureka-cut8-mutations.psd1` | H14's expectation stands (`IdentityCollision` on a ruling). Any H21-H39 entry anchoring `stewardship_of`, `derive` or `refuse_collisions` lines re-anchored. New: H40 resolution sequence rule → `true`; H41 in-force non-recursive (a withdrawn resolution still counts) → the reopen test; H42 stewardship sequence rule → `true` → the stewardship test's `expected: 2, actual: 1` case; H43 `AlreadyStewarded` dropped → the same test; H44 Q19 cap dropped → the reopen test's withdrawal-of-withdrawal; H45 `derive` uses `sequence: 1` for every derived write → the reopen test's second ruling and the transfer test's `n2`. |
| `README.md`, `AGENTS.md` | unchanged; Cut 12's map section corrected by Self ("superseding" → "withdrawal"). |

Estimate: +about 260 lines in Huginn (rules 50, refusals 9, fixtures 16,
tests 150, entries 48), −about 3 (the A10 arm). One build, `cargo test -p
huginn-mind --lib`, plus the cut-8 suite through the harness with `-Repo`.

### Subtraction estimate

Epiphany, this cut: source outside tests +about 12 (two fields, two
`format!("n{}")` lines, two chained parts, four doc lines), −0; tests +about
100 (three tests) and about 25 lines of moved literals; two schema files
+5/−0 each; one 6b entry re-anchored. Kinds, dependencies, targets, formats,
epoch, `index.json`: zero. Liability retired: the one key that could not
name the second closure of a subject, and the one key that could not name
the second stewardship of a repo — both of which Soul found as medium
defects against Cut 8, and both of which would have cost a stored-document
re-key the moment Yggdrasil's mind held a resolution. Also retired before it
ever shipped: the date-keyed stewardship's same-day collision.

Huginn, the follow-up: +260/−3 as above. Net across both: about +380, all
tests and rules for a capability the operator asked for by name.

### Build budget

Epiphany: `epiphany-pipeline` lib + tests only; `epiphany-core` check, no
rebuild expected. Debug, workstation host = target, no features, no
codegen. Footprint delta: within the noise of a warm leaf rebuild (the probe
this pass rebuilt the leaf's test binary in 15 s); expected +0 to +40 paths.
Huginn follow-up: `huginn-mind` lib + tests; the leaf rlib rebuilds at the
new rev (+about 30 paths). Retention: the shared target dir is the operator's;
nothing deleted.

### Operator questions

None open. Two were raised by this spec and ruled the same day:

- **Q19. Can a withdrawal be withdrawn? Ruled A, 2026-09-16.** A resolution
  is admissible with `Withdrawn` only when its subject's own subject is not a
  resolution (chain depth two at admission). To reinstate a withdrawn
  closure, resolve the subject again at `n<N+1>`. History stays append-only,
  "the latest not withdrawn" has one reading, and the in-force recursion
  never re-raises an earlier record over a later one. The rejected B (any
  depth; in-force is "the latest among the not-withdrawn") could express a
  standing record that is not in force. H44's mutant is exactly B.
- **Q20. Stewardship keyed by date or by sequence? Ruled A (sequence),
  2026-09-16.** `<instance>:stewardship:<repo escaped>.n<N>`, writer-set
  `sequence: u32` checked by admission as previous plus one, exactly as
  resolutions; `assigned_on` stays a field. The first draft's date key was
  rejected for the same-day collision and because a date orders nothing the
  sequence does not.

The Q18 correction (transfer, not lease) raised nothing new: the derivations
were already symmetric in the source.

### Findings not assignable to this cut

- **"Additive" has two definitions in the repo.** The epoch constant's doc
  (`lib.rs:660-666`) says a new field is additive with a serde default; the
  campaign's practice (6c's four required fields, this cut's `sequence`) is
  "additive while no store is written at the epoch". Both are right today and
  will disagree the day Yggdrasil holds a mind. Owner: the map's ruling 8
  text and the constant's doc; not this cut's.
- **Cut 12's spec says "the superseding `stewardship` on the source side"**
  (map `:2707-2709`); Cut 8 landed a `Withdrawn` resolution there
  (`admission.rs:437-442`). Stale before this cut; Self's to correct.
- **Cut 8's map text at A10** ("a subject resolves at most once because its
  resolution key is outcome-invariant", `:2344`) and the stewardship row
  ("the key collides, A10", `:2362`) both describe the shape this cut retires.
  Self's, at the follow-up's landing.
- **Cut 9's `open_items_and_rulings_in_force_follow_resolutions`** needs the
  recursive derivation and a reopened-subject fixture, or it will pin the
  non-recursive defect as correct. Cut 9's spec should say so before Hands
  reads it.
- **Same-day granularity survives only in the hand-off key** (`:721`,
  `<from>:hand_off:<to>.<repo>.<handed_on>`): two hand-offs in one direction
  for one repo on one day share a key. The stewardship half of this limit is
  retired by Q20 A. If the Odin-swarm workflow ever transfers a repo twice
  in a day in one direction, the hand-off key wants the same treatment; not
  asked, not this cut's, recorded.
- **`Date` derives `Ord` on its string** (`:212`), correct for `YYYY-MM-DD`
  only because `Bounded::validate` enforces the shape. Nothing orders dates
  after Q20; the sequence orders.

### Pinned HEADs

- Epiphany `codex/eureka-pipeline-state` at `542dc184` (map only since
  `87d3420c`: `993600fd`, `76167f0a`, `542dc184`); `epiphany-pipeline/src/lib.rs`,
  `schemas/cultnet/` and `tools/` byte-identical to `b4b17fc`. Tree clean.
- Huginn `eureka/memory-organ` at `30daff8`, tree clean. The fix batch
  landed as `acd32f3` (matrix row `resolution → Withdrawn` restored, nested
  assertions flipped) and `30daff8` (nineteen rules pinned, entries
  H21-H39). Follow-up anchors above are against `30daff8`. Pin in
  `crates/huginn-mind/Cargo.toml:15` is still `a65c6420`.
- CultLib untouched and unread this pass (Soul is in that tree).
- Probe artifacts: a detached worktree `scratchpad/cut6d-wt` at `b4b17fc` ran
  `cargo test -p epiphany-pipeline --lib` once (28 passed) under the shared
  target dir and was removed; Huginn's sources at `1cfa81d` were copied to
  `scratchpad/huginn-*-1cfa81d.rs` by `git show` and read there while the
  batch was in flight, then the landed `30daff8` was grepped for every site
  the follow-up anchors.

## Cut 7. Retire Huginn's TypeScript body

- **Repo/branch:** Huginn `main` at `91b7fcf`, on a new branch
  `eureka/memory-organ`. No dependency on the Epiphany cuts.

**Deletes first.**

| Path | Lines | Note |
|---|---:|---|
| `src/cli.ts` | 23 | the `huginn <file>` CLI |
| `src/huginn-eve-dsl.ts` | 57 | `buildHuginnEveDsl` |
| `src/index.ts` | 8 | package entry |
| `dist/` (9 tracked files) | 88 | tracked build output |
| `package.json` | 35 | `@gamecult/huginn` |
| `package-lock.json` | 70 | |
| `tsconfig.json` | 26 | |

Total **307 tracked lines and 16 files.**

**This is safe to delete outright** because nothing consumes it (R12). The
census's one hesitation — `src/index.ts:2` re-exporting `inspectCultCacheBytes`
— is resolved: a grep of every `package.json` under `F:\Projects` finds no
dependant.

**Nothing needs to move to CultCache Studio.** Ruling 17 says generic `.cc`
inspection *belongs* to CultCache Studio, and CultCache Studio already exists
and already inspects and edits `.cc` state (R14). `buildHuginnEveDsl` is a
57-line string builder over CultLib's `inspectCultCacheBytes`; CultLib owns the
inspector, and the Studio owns the presentation. Porting a stale text projection
into a working editor would be additive work that buys nothing. **Record the
retirement; move no code.** If the operator later wants a headless `.cc`-to-Eve
projection, it is a CultLib cut with its own consumer.

**Eve's fixture and catalog entry stay, with a provenance correction.**

- `Eve/web/fixtures/huginn-cc-surface.eve` and its `.conformance.json` are
  conformance material for Eve's own lowering path, exercised by
  `docs/renderer-parity.md:58`. They do not import or execute Huginn (R13).
  Deleting them would remove an Eve test to tidy another repo.
- But the conformance file's `"ownerRepo": "Huginn"` and
  `"exitCriteria": "Move to Huginn when .cc schema inspection ... become runtime-owned"`
  are now false: Huginn will never own that surface. **Cut 7 changes those two
  fields** — `ownerRepo` to `Eve`, `exitCriteria` to a sentence saying the
  surface is a retired-projection fixture retained for lowering conformance —
  and adds one line to `docs/renderer-parity.md:58` marking it as such.
- `Eve/web/local-provider-catalog.json:72-83` keeps its entry; it already
  declares `"freshness": {"state": "fixture"}` and points at the file, not the
  package. No change.

**Huginn's legacy `.voidbot` Persona state is out of scope and stays untouched.**
Recorded, not solved, per the target's Not-in-scope list:

- `.voidbot/state/huginn.cc` (35,297 bytes) holds eight legacy `void.*` document
  types and zero `gamecult.persona_state.v0` documents, and identifies its
  jurisdiction as `repo:CultCacheTS`.
- `.voidbot/voice/identity.json` says the Persona and repo are Huginn, and
  carries the lost path `E:\Projects\Huginn`.
- These two disagree about who the Persona is. That contradiction predates this
  campaign, survives it, and belongs to whoever owns the portable-Persona
  migration. **Hands does not touch `.voidbot/` in any cut.** Recorded as FU-1.

**Adds.** An empty Rust workspace: root `Cargo.toml` with
`members = ["crates/huginn-mind", "crates/huginn-daemon", "crates/eureka-state"]`,
`resolver = "3"`, `[workspace.package] edition = "2024"`, `license = "MIT"`,
`publish = false`, following Odin's layout. `.gitignore` gains `target/` and
drops the four stale entries the census flagged (`dist-test/`, `dist-inspector/`,
`release/`, `release-inspector/`, `.gitignore:2-5`).

**Docs.** `README.md` (74 lines) and `AGENTS.md` (81 lines) are rewritten, not
deleted: Huginn is the memory organ that owns instance minds. Both currently
describe the `E:` drive body and deleted EpiphanyAgent commands. The rewrite
states the new invariant, the CultNet surface, the Qdrant dependency, and that
`.cc` inspection is CultCache Studio's.

**Authority map.**

- **Owner:** Huginn becomes the memory organ. Before this cut it owned a
  projection; after it, it owns state.
- **Inputs:** none yet; Cut 8 adds them.
- **Outputs:** none yet.
- **Derived state:** none.
- **Forbidden writers:** after this cut nothing in Huginn emits Eve DSL, reads
  `.cc` for inspection, or publishes an npm package.
- **Shared paths:** Eve's fixture is now Eve's alone.
- **Deletion line:** the table above, plus the two corrected Eve fields.

**Verification.**

- **Builds:** `cargo check --workspace` in Huginn succeeds on an empty
  workspace (three stub crates with empty `lib.rs`/`main.rs`).
- **Negative greps:** `rg -n "buildHuginnEveDsl|@gamecult/huginn|cultcache-ts" F:\Projects\Huginn`
  empty; `rg -n -F 'E:\Projects' F:\Projects\Huginn --glob '!.voidbot/**'`
  empty. *(Corrected after Soul: the original regex form was a ripgrep
  parse error, not an empty result.)*
- **Unchanged check:** `.voidbot/` is byte-identical.
  `git diff --stat 91b7fcf -- .voidbot` is empty.
- **Eve:** its renderer-parity fixture check still passes.
- **Operator:** confirm `npm` publication of `@gamecult/huginn` is not expected
  to continue. The package was never published to a registry consumer this map
  can find, but unpublishing is the operator's call.
  **Ruled 2026-09-16:** not publishing, nothing to unpublish. The operator
  first said npm was unreachable ("our entire subnet is blocked"), then
  corrected it the same day: "apparently that was true yesterday, but I was
  able to get to npm signup just now, so we're not actually blocked on
  publishing. Still for a later pass." So registry publication is deferred
  by choice, not impossible; the QUIC map's Q5 stays on C for that campaign
  on that reason. Huginn itself publishes nothing either way.

**Subtraction ledger:** −307 lines, −16 files, −1 npm package, −1 CLI
entrypoint, −1 sibling `file:` dependency. +about 30 lines of workspace
scaffolding. Two Eve fields corrected.

**Landed 2026-09-16** at Huginn `1320fc4`, `f63c0f2`, `e20c786` on
`eureka/memory-organ` and Eve `e777e4c` on `main`. Hands verified the
spec's Body claims before deleting: no `package.json` under `F:\Projects`
depends on `@gamecult/huginn` but its own; the Eve fixture neither imports
nor executes Huginn; the catalog entry needs no change. Corrections:

25. **The deletes table sums to 15 files and 315 lines, not 16 and 307.** The
    seven paths were unambiguous, so Hands deleted exactly those; `dist/` is
    96 lines by numstat, not 88. Spec arithmetic, not a Body fault.
26. **`Cargo.lock` is committed**, following Odin's layout, though the spec's
    adds did not list it. Kept.
27. **`.gitignore` keeps `node_modules/`**, which the spec did not name and
    which is now stale too. One-line follow-up.
28. **The conformance fixture also carries `"splitTarget": "Huginn"`.** The
    spec named two fields; the third is as false as the two. Soul is asked
    whether anything reads it.

Verification: `cargo check --workspace` clean (three stub crates, zero
dependencies); both negative greps empty; `.voidbot` diff empty; Eve's four
`web/*.test.mjs` pass (13 tests), the fixture compiles through
`compileEveDsl`, and the edited conformance JSON validates against Eve's
schema. Huginn net +118 / −444 across 26 files; README 74 → 47, AGENTS
81 → 48. No mutation suite: nothing here is a rule a unit test pins.

Consumer grep over the whole tree found no live consumer of the package, CLI
or DSL builder. The surface id `cultcache.huginn.inspector` survives in
Ghostlight's vendored Eve copy and the `Eve-aetheria-authority` worktree,
both of which still say `"ownerRepo": "Huginn"` until re-vendored, and in
VoidBot's provider-advertisement prose describing the old inspection role.
Recorded as FU-7: re-vendor, and retire the VoidBot hand-off prose when
VoidBot's Persona doc is next touched. (First written as FU-2, which was
already taken; Soul caught the collision.)

**Soul on Cut 7** (default model) held the deletion line (`.voidbot` tree
hash identical at both ends, so no mode or EOL flip is possible), the
no-consumer claim over every manifest, lockfile, import string and runbook
under `F:\Projects`, the workspace (`cargo 1.95.0`; `resolver = "3"` and
edition 2024 both satisfied; `--locked` clean), and every Eve check. Found:

- **A second `ownerRepo` authority disagrees with the first.**
  `EveConformance/tools/parity/parity-manifest.json:466` still says Huginn,
  and `run-parity.mjs:826` compares it to Eve's metadata, so the parity check
  for this fixture now fails. The spec named only Eve's two fields; the
  reader lives in a third repo. Nobody would have seen it, because the
  harness cannot reach a report anyway: the manifest wants an Aetheria
  conformance file that has not existed since July. Pre-existing hole,
  recorded here, not this cut's.
- **Three more Huginn-naming fields** in the fixture material: `purpose`,
  `splitTarget`, and the `.eve` file's first-line comment. No programmatic
  reader; four consecutive fields that contradict each other.
- **AGENTS.md describes Cuts 8-10 in the present tense** with no stub
  caveat; README carries one at `:33-35`. AGENTS is the surface an agent
  rehydrates from. The spec asked for that content, so it is a tension
  between the spec and the "describe the live system" invariant.
- **The spec's second negative grep is unrunnable as written**: the escaped
  backslashes are a Unicode-property error to ripgrep. Corrected below to a
  fixed-string search.
- **Committed blobs carry CRLF, including `Cargo.lock`**; no
  `.gitattributes`; the predecessor blobs and Odin's lock are CRLF too, so
  house pattern, not a deviation. The first cut that adds a dependency will
  rewrite the lock as LF and produce a whole-file diff. Recorded.

**The Cut 7 fix batch landed** at EveConformance `048ea2f` (manifest
`ownerRepo` → Eve; the isolated `run-parity.mjs:824-833` comparison passes),
Eve `167a2d3` (`purpose` and the `.eve` comment reworded; `splitTarget`
omitted, because the schema declares it an optional string, five Eve-owned
fixtures already omit it, and no reader consumes it on fixture metadata;
schema validation true, 13 tests pass, the fixture compiles), and Huginn
`4094e68` (AGENTS carries the stub caveat; the "does not" list is one
sentence naming Studio). Three one-line-class edits with their checks
pasted; no separate Soul pass. The next Soul on Huginn, at Cut 8, rereads
AGENTS against the Body. **Cut 7 is closed.**

Operator ruling on the npm check: not publishing, nothing to unpublish, and
npmjs is unreachable from this network (see Verification above).

## Cut 8. `huginn-mind`: storage, identity and admission

Refreshed by Imagination 2026-09-16 against the landed key grammar and the
Cut 6c shapes; the first issue, written against Epiphany `5fb4eb22` and
Huginn `91b7fcf`, was stale in every anchor and is replaced whole.

### Pins

Every `file:line` below is against these trees, all verified clean and pushed
this pass.

| Repo | Branch | HEAD | Notes |
|---|---|---|---|
| Epiphany | `codex/eureka-pipeline-state` | `ca7e230c` | in sync with `origin`. `epiphany-pipeline/src/lib.rs` last moved at `dddf9ede` (1,937 lines, 26 tests); anchors are against that file at that commit, which is byte-identical at `ca7e230c`. |
| Huginn | `eureka/memory-organ` | `4094e68` | in sync with `origin`. Three stub crates, zero dependencies, `Cargo.lock` holds only the three members (CRLF, no `.gitattributes`). |
| CultLib | `main` | `4a2fdaf` | `git diff --stat a0813c6 4a2fdaf -- packages/cultcache-rs packages/cultnet-rs packages/cultmesh-rs` is **empty**: the three Rust runtimes are byte-identical at the pin Epiphany uses and at `main`. Huginn pins `a0813c6eed24d30bf88073ef615b633c77ebfcd6`, the same rev, and must: a second rev of the same git URL is a second `cultcache-rs` package, and `epiphany_pipeline`'s `DatabaseEntry` wrappers would not unify with Huginn's cache. |
| gamecult-ops | `main` | — | read only for R15 (Qdrant is voidbot's container, `compose/voidbot-retrieval.yggdrasil.yaml`) and the `[[dependencies]]` shape (`Ghostlight/deployment/idunn/recipe.toml:173-187`). Cut 8 names no dependency; that is Cut 14's. |

Body facts the brief asked to verify, not trust:

- **`epiphany-pipeline` has exactly four normal dependencies** (`Cargo.toml:17-21`: `chrono`, `cultcache-rs`, `schemars`, `serde`) and three dev-dependencies (`:23-26`: `anyhow`, `rmp-serde`, `serde_json`). A throwaway crate pinning it by git rev resolved **91 packages, no `epiphany-core`, no Ghostlight, one `cultcache-rs`, `redb 4.3.0`, `schemars 1.2.2`** (probe: `cargo generate-lockfile` in the session scratchpad, no compile). After this cut it has six normal dependencies; see Epiphany half.
- **`epiphany-core` pins CultLib at `a0813c6`** for `cultcache-rs`, `cultmesh-rs`, `cultnet-rs` (`epiphany-core/Cargo.toml:15-17`; root `Cargo.toml:24-25`). Huginn pins the same rev, above.
- **Huginn's crates are empty stubs**: `crates/huginn-mind/src/lib.rs` 0 bytes, `crates/huginn-daemon/src/main.rs` is `fn main() {}`, `crates/eureka-state/src/lib.rs` 0 bytes; each `Cargo.toml` has an empty `[dependencies]`.
- **The receipt logic in `epiphany-core` at HEAD** is `commit_authorized_mind_mutation` (`reasoning_context.rs:1584-1701`, 118 lines), `EpiphanyMindCommitReceipt` and its `validate` (`:543-602`), `EpiphanyMindCommitAuthority` (`:604-616`), `EpiphanyMindCommitOutcome` (`:618-624`), `EpiphanyMindDocumentVersion` (`:159-202`), `mind_commit_receipt_id` (`:1836-1851`), `validate_unique_envelope_identities` (`:1853-1861`), `validate_mind_document_versions` (`:1863-1875`), `current_write_collisions` (`:1877-1893`). **About 260 lines, not the 120 FU-3 names.** Everything but the receipt struct and outcome is `pub(crate)` or private (R10 still holds).
- **The leaf's write path is test-only.** `PipelineDocument::prepare` (`lib.rs:475-487`), `decode` (`:489-499`), `register_pipeline_document_types` (`:502-511`) and `validate_pipeline_write_envelope` (`:739-757`) are all `#[cfg(test)]` and `pub(crate)`, and the `anyhow`/`CultCache`/`CultCacheEnvelope` imports they need are gated with them (`:32-39`). Their doc comments say "until the organ prepares, decodes and validates them (Cut 8)". **So Cut 8 touches Epiphany**, and the old section's silence on that was wrong. Without this, Huginn would have to re-declare the thirteen `DatabaseEntry` wrappers and their type ids: a second authority over the wire shape.
- **No epoch constant exists anywhere in code.** `PIPELINE_SCHEMA_EPOCH` died with Cut 4's fix batch (Landed, correction list, "Cut 8 writes them in the organ"). The string `epiphany.pipeline.epoch.v1` survives only in prose (`schemas/cultnet/README.md:41-44`, the map). Ruling 1 makes the epoch a schema fact Epiphany owns; a Huginn-local literal would be the S5 drift shape. It goes back into the leaf as one `pub const` (Epiphany half).
- **`cultcache-rs` at `a0813c6` offers two redb stores**, both keyed one row per `(type, key)` with a transactional `compare_and_swap_batch` that refuses a replacement whose identity already exists and is not in `expected` (`lib.rs:1098-1150` transient, `:1492-1543` owned). `RedbMessagePackBackingStore` (`:966`) reopens the database per operation; `OwnedRedbMessagePackBackingStore` (`:1340-1445`) holds an fs2 exclusive lock on `<path>.lock`, the file handle and the open database for its lifetime, records the file identity (dev/inode on unix, `GetFileInformationByHandle` on Windows, `:1772-1795`), and clones share that ownership. Both implement `CacheBackingStore` (`:1713`, `:1844`). `CultCache::add_backing_store` gives a type exactly one home store (`:1968-2015`) and `pull_all_backing_stores` refuses an unregistered type (`:2022-2046`).
- **`cultnet-rs`** (`packages/cultnet-rs`, 0.1.0) is not a Cut 8 dependency and is not read further; R6/R7 carry for Cut 10.

### What changed against the old Cut 8 section

1. **Epiphany is touched.** A small "Epiphany half" lands first: the leaf's prepare/decode/register/validate path stops being `cfg(test)`, `anyhow` and `rmp-serde` become normal dependencies, the epoch constant returns, and the live registrar stops registering the test stand-in. Huginn then pins that commit.
2. **The store is `OwnedRedbMessagePackBackingStore` at `<state_root>/minds/<instance>/mind.redb`**, not the transient redb store at `mind.cc`. Owned gives the single-writer invariant a mechanism (the lock is held for the daemon's life, so a second opener of the same mind is refused structurally, in or out of process) instead of a sentence. *The mechanism is CultCache's, not redb's: the owned store's exclusive file lock, its file-identity check and the transactional `compare_and_swap_batch` are `cultcache-rs` semantics, in CultLib since the engine's return on 2026-09-03 (`b672fb7`) and pinned across runtimes by the CultCache migration on 2026-09-13/14; redb is the engine under the owned store. Operator correction, 2026-09-16.* `.redb`, not `.cc`: Epiphany selects its backend by extension (`runtime_store_backend.rs:29-34`) and CultCache Studio inspects `cultcache.store.v1` files (P7); a redb file named `.cc` would lie to both.
3. **The receipt is duplicated, bounded, and smaller than Epiphany's.** No authority enum, no companions, no `invariant_owner`, no `store_id`; provenance is a field on the receipt, not a companion document; the digest excludes provenance so exact replay is idempotent across sessions (old D4's rule, which Epiphany's digest does not give). The moving alternative is costed under "FU-3" and not recommended now.
4. **The rule set is stated in full** (the old section pointed at "the old D4", which is now in History only through refusal names). `WrongReferenceKind` is gone: references are looked up by `(kind.type_id(), id)`, so a wrong kind is a missing reference. `RepoNotInCampaign` splits into `RepoNotStewarded` (campaign repos against the mind's stewardship, D3 step 5) and `RepoNotInCampaign` (spec/report repo against the campaign).
5. **`hand_off` is admitted here with its derivations on this mind's side only**; Cut 12 composes the two-mind operation and the import over `admit_prepared`. The old section's `hand_off_derives_stewardship_on_both_sides` moves to Cut 12, where both minds exist.
6. **The batch and outcome types are `pub` and derive `JsonSchema`** so Cut 10's wire and Cut 13's tools can carry them whichever way Q13 is ruled.
7. **The mutation suite is an entries file in Huginn run through Epiphany's harness with a `-Repo` parameter** (Q14), not a copied harness.
8. **Tests: 25, mutations: 20**, replacing the old 16/3. Estimate about +2,100 lines in Huginn and +40/−20 in Epiphany, not +1,400.

### Repo, branch, ordering

- **Epiphany half:** `codex/eureka-pipeline-state`, one commit, pushed before the Huginn half's lock is generated (Cargo fetches the rev from GitHub). Epiphany's remaining stake is schema ownership; this commit is inside that stake, not a new service surface.
- **Huginn half:** `eureka/memory-organ` from `4094e68`, four commits: (1) `Cargo.toml`, `store.rs`, `mind.rs` with the opener and identity rules; (2) `receipt.rs` and the commit primitive; (3) `admission.rs` and `refusal.rs` with the rule table; (4) `tools/eureka-cut8-mutations.psd1`. Soul can verify per commit; if Hands' attempt runs long, the split point is between (2) and (3), and the map records it as 8a/8b.
- Depends on Cuts 6c and 7 (both landed). Blocks Cuts 9, 10, 12.

### Deletes first

Epiphany, `epiphany-pipeline/src/lib.rs` at `dddf9ede`:

| Path | Lines | What dies |
|---|---:|---|
| `:32-39` | 8 | The two `#[cfg(test)]` gates on `use anyhow::Result;` and `use cultcache_rs::{CultCache, CultCacheEnvelope};` and the four-line comment explaining why they were dev-dependencies. |
| `:475-479`, `:489-491`, `:502-506`, `:739-743` | 17 | Four `#[cfg(test)]` attributes and the four doc comments that say "test scaffolding until the organ ... (Cut 8)". Replaced by one-line live doc comments. |
| `:509` | 1 | `cache.register_entry_type::<ForeignDocument>()?;` inside the live registrar. The test stand-in moves to the tests' `schema_cache` (`:977-981`). A live registrar that registers a `cfg(test)` type does not compile un-gated, so this delete is forced, not optional. |
| `Cargo.toml:23-26` | 2 | `anyhow` and `rmp-serde` leave `[dev-dependencies]`. |

Huginn: nothing; the crate is empty. The *replaced* liability is named: the writer lease, git preconditions and merge tool Cut 4 deleted, and the transient-store/session-lease split the old D3 needed.

### Keeps and moves

- **Keeps, Epiphany:** every kind, type id, field, bound, format, key, `pipeline_key`, `pipeline_id` (stays private; Huginn looks references up by `(type_id, key)` and never parses an id, so it needs no reader), `PipelineRefusal`'s four variants including `ForeignStore` (raised by `decode`, which stays here; D2's "moves to the organ" for this one variant is corrected: the raiser stays, so the variant stays), the twenty-six tests unchanged in name, `ForeignDocument` still `cfg(test)`, the wrappers still `pub(crate)`, `schemas/cultnet/` byte-identical (no kind, field or type id moves; `git diff --stat -- schemas/cultnet/` empty).
- **Moves, Epiphany:** `anyhow` and `rmp-serde` from dev to normal dependencies (both already in the workspace lock; zero lock change). `register_entry_type::<ForeignDocument>` from the registrar to `schema_cache`.
- **Keeps, Huginn:** `Cargo.toml` workspace (`:1-11`), `README.md`, `AGENTS.md` (the stub caveat at `AGENTS.md:15-17` and `README.md:33-35` becomes false for `huginn-mind` and is reworded in commit 1 to say which crate is live).
- **Not moved (FU-3):** `epiphany-core`'s receipt. Moving it to CultLib means a `cultcache-rs` change under an in-flight QUIC campaign at `4a2fdaf`, a new CultLib rev to re-pin in Epiphany (campaign two) and Huginn, a receipt generic over an authority type Epiphany's three-variant enum and Huginn's provenance would both instantiate, and a C# reference-parity question for a helper only Rust uses. Moving it into `epiphany-pipeline` breaks the leaf's own charter (`lib.rs:3-7`: no storage). **Recommendation: duplicate, bounded to one file `receipt.rs` of at most 220 lines, with the digest formula written in its doc comment so a later extraction is mechanical.** FU-3 stands with "a third consumer" as the trigger, and its line count is corrected to about 260 in Epiphany.

### Adds

### Epiphany half, `epiphany-pipeline`

| Add | Owner | Live consumer | Protected invariant | Why not an existing owner |
|---|---|---|---|---|
| `pub const PIPELINE_SCHEMA_EPOCH: &str = "epiphany.pipeline.epoch.v1"` | the leaf (ruling 1) | `huginn-mind`'s opener and first write | One epoch string, owned where the schemas are; a breaking bump refuses the old store (Q2, ruling 8) | No constant exists; a Huginn literal is the S5 shape. |
| `pub fn register_pipeline_document_types(&mut CultCache)` (un-gated, thirteen types only) | the leaf | `Mind::open` | Only the leaf names the wrappers; the organ registers what the leaf publishes and nothing else | The wrappers are `pub(crate)`; this is the one door. |
| `pub fn PipelineDocument::prepare(&self, &CultCache) -> anyhow::Result<CultCacheEnvelope>` (un-gated) | the leaf | `Mind::admit` | Every stored payload is `[value]` prepared with `prepare_entry_named` and keyed by `pipeline_key` | Same door. |
| `pub fn PipelineDocument::decode(&CultCacheEnvelope) -> Result<Self, PipelineRefusal>` (un-gated) | the leaf | `Mind::get`, Cut 9's views, Cut 12's import | A decode is type-matched both ways (`ForeignStore` on any other type) | Same door. |
| `pub fn validate_pipeline_write_envelope(&CultCacheEnvelope) -> Result<(), PipelineRefusal>` (un-gated; return type narrowed from `anyhow::Result<()>`) | the leaf | `Mind::admit_prepared` (Cut 8) and the import replay (Cut 12) | Bounds, formats, then key recomputation, on the envelope that will be stored, whoever prepared it | The organ must not re-derive this check. The narrowing removes a `downcast_ref` at `:1072-1076`; every failure path is already a `PipelineRefusal`. |

### Huginn half, `crates/huginn-mind`

Dependencies (`Cargo.toml`): `anyhow`, `chrono = "0.4.44"`, `cultcache-rs` (git `a0813c6…`), `epiphany-pipeline` (git, `rev` = the Epiphany half's commit), `rmp-serde = "1"`, `schemars = "1"`, `serde`, `sha2 = "0.10"`. Dev: `tempfile = "3"`. No `reqwest`, no `cultnet-rs`, no `cultmesh-rs`, no socket. `sha2` is the one package new to Huginn's graph that the leaf does not already bring (it is in Epiphany's lock, not the leaf's 36).

| Add | Owner | Live consumer | Protected invariant | Why not an existing owner |
|---|---|---|---|---|
| `store.rs`: `pub trait MindStore: CacheBackingStore + Clone { fn compare_and_swap_batch(&self, expected: &[CultCacheEnvelope], replacements: Vec<CultCacheEnvelope>) -> anyhow::Result<bool>; }` with `impl MindStore for OwnedRedbMessagePackBackingStore`; `#[cfg(test)] MemoryStore` (a `BTreeMap` behind a `Mutex`, cloneable) and `#[cfg(test)] RefusingStore` (a `MemoryStore` whose CAS returns `Ok(false)` or `Err` on command) | `huginn-mind` | `Mind::open` (owned redb), every admission test (memory), Cut 12's atomicity test (refusing) | The commit primitive is testable without redb, a daemon or Qdrant; CAS is not on `CacheBackingStore`, so the narrow trait is the only way to inject it | `cultcache-rs` exposes CAS as inherent methods on concrete stores. Epiphany's `RuntimeSpineBackingStore` enum solves it with an extension switch the organ does not need. |
| `mind.rs`: `pub struct Mind<S: MindStore> { instance: Slug, store: S, image: CultCache }`; `pub fn open(state_root: &Path, instance: &Slug) -> Result<Mind<OwnedRedbMessagePackBackingStore>, MindRefusal>`; `pub fn open_with(store: S, instance: &Slug) -> Result<Mind<S>, MindRefusal>`; `pub fn instance(&self) -> &Slug`; `pub fn get(&self, kind: PipelineKind, id: &str) -> Result<Option<PipelineDocument>, MindRefusal>`; `pub fn envelope(&self, kind, id) -> Option<&CultCacheEnvelope>`; `pub fn envelopes(&self) -> &[CultCacheEnvelope]` (the image, for Cut 10's snapshot source); `pub fn receipts(&self) -> Result<Vec<HuginnCommitReceipt>, MindRefusal>`; `pub fn is_empty(&self) -> bool`; `pub fn path_for(state_root, instance) -> PathBuf` (`<state_root>/minds/<instance>/mind.redb`) | `huginn-mind` | Cut 10's daemon (`open`, `envelopes`), Cut 9 (`get`, `envelopes`, `receipts`), Cut 12 (`envelope`) | Ruling 14: a store is canonical to one instance, and identity lives in the state (the `instance` document) not the path; ruling 15: one writer (the owned lock); ruling 20's analogue: the opener refuses before it attaches | Nothing in Huginn exists; Epiphany's opener is `pub(crate)` and bound to its Mind types. |
| `mind.rs`: `HuginnMindEpoch` (`DatabaseEntry`, type `huginn.mind_epoch.v1`, slot 0 `schema_epoch: String`, keyed by the epoch string) | `huginn-mind` | the opener; the first write (derived) | The store carries the schema epoch it was written at, so a breaking bump can refuse it (`ForeignEpoch`) | Epiphany's `EpiphanyPipelineIdentity` died in Cut 4 and its type id was Epiphany's. |
| `receipt.rs`: `HuginnCommitReceipt` (`DatabaseEntry`, type `huginn.mind_commit_receipt.v1`, slots: `schema_version`, `receipt_id`, `instance: String`, `provenance: PipelineProvenance`, `strong_reads: Vec<DocumentVersion>`, `writes: Vec<DocumentVersion>`, `committed_at: String`), `DocumentVersion { document_type, document_key, schema_id, payload_msgpack, payload_sha256 }`, `pub(crate) fn receipt_id(instance, strong_reads, writes) -> String` = `"mind-commit-" + sha256(rmp_serde::to_vec_named(&(instance, strong_reads, writes)))`, `validate`, `pub(crate) fn commit(&mut Mind, provenance, strong_reads, writes, now) -> Result<Committed \| AlreadyAdmitted \| Conflict, MindRefusal>` (the one construction site of a receipt; the CAS; the conflict re-read) | `huginn-mind` | `Mind::admit_prepared` only | A batch lands whole with a receipt naming its exact bytes, or not at all; exact replay answers with the stored receipt; a lost CAS is typed `Conflict`; the receipt id is a digest of what was read and written, never of who wrote it | FU-3: Epiphany's is `pub(crate)` and shaped for its scheduler. |
| `admission.rs`: `Faculty { SelfFaculty, Imagination, Hands, Soul, MindSteward, Eyes, Operator }`, `PipelineProvenance { faculty, agent: Short, session: Short, tool: Short }`, `PipelineAdmissionBatch { instance: Slug, provenance, documents: Vec<PipelineDocument> }` (1..=64), `PipelineAdmissionOutcome { Committed { receipt_id, committed_at, writes: Vec<PipelineRef> }, AlreadyAdmitted { receipt_id }, Refused(MindRefusal), Conflict { identities: Vec<PipelineRef> } }`, `impl Mind { pub fn admit(&mut self, batch, now: DateTime<Utc>) -> PipelineAdmissionOutcome; pub fn admit_prepared(&mut self, instance, provenance, envelopes: Vec<CultCacheEnvelope>, now) -> PipelineAdmissionOutcome }`, the rule table below, the derivations | `huginn-mind` | Cut 10's sink calls `admit`; Cut 12's hand-off and import call `admit_prepared`; Cut 13 constructs `PipelineAdmissionBatch` and reads `PipelineAdmissionOutcome` (through the wire, Q13) | Every cross-field and cross-document rule lives here and nowhere else; the leaf never gains one; the daemon and the client never re-derive one | D1/D2: admission is Huginn's. `JsonSchema` on the four public types costs nothing (`schemars` is already in the graph through the leaf) and keeps Q13 open. |
| `refusal.rs`: `pub enum MindRefusal` = `Document(epiphany_pipeline::PipelineRefusal)`, `ForeignInstance { declared, mind }`, `MissingIdentity`, `ForeignEpoch { found, expected }`, `ForeignStore { r#type }`, `MindAlreadyOwned { path }`, `BatchSize { actual }`, `IdentityCollision { kind, id }`, `MissingReference { kind, id }`, `AlreadyResolved { subject }`, `IncompatibleResolution { subject_kind, outcome }`, `CitesResolvedDocument { kind, id }`, `EmptySupersession`, `UnknownSupersessor { id }`, `RevisionWithoutSupersession { kind, revision }`, `DuplicateLabel { field, label }`, `InvalidOptions { question }`, `InvalidChoice { ruling, choice }`, `QuoteWithoutOperator { ruling }`, `RepoNotStewarded { repo }`, `RepoNotInCampaign { repo }`, `CutReportWithoutSpec { report }`, `SpecMismatch { field }`, `RangeOutsideCommits { head }`, `FalsifiedClaimWithoutConfirmedFinding { claim }`, `UnprovenClaimWithConfirmedFinding { claim }`, `PromiseWithoutVerdict { promise }`, `UnknownMutationLabel { label }`, `FindingWithoutRange`, `FindingWithoutEvidence`, `UnknownInvariant { label }`, `NotStewarded { repo }`, `Unavailable { detail }`; `Display`, `Error`, `JsonSchema` | `huginn-mind` | the outcome above; Cut 13's tool outputs | Refusals are data with a field an agent can act on, never a transport error | D2 assigned the service half here. `Document(..)` wraps rather than copies the leaf's four. |

`Faculty` and `PipelineProvenance` are the shapes Cut 4 deleted from Epiphany (`ca275c7b^:pipeline_documents.rs`, `Faculty` unit enum and `PipelineProvenance` value type), re-created here because their only consumer is the receipt. Cut 9's `faculty` query filter and `PipelineDocumentView.faculty` read them from receipts. `faculty` is attribution, not authority (ruling 18); no rule below trusts it.

### The opener, exactly

`Mind::open_with(store, instance)`, fail-closed and in this order, nothing attached until every step passes:

1. `store.pull_all()` → the raw envelopes. A store error is `Unavailable`.
2. Every envelope's type is one of the thirteen pipeline type ids, `huginn.mind_epoch.v1` or `huginn.mind_commit_receipt.v1`; else `ForeignStore { type }`. (A runtime or Mind store passed by mistake dies here, as in Cut 3a.)
3. If any envelope exists: exactly one `HuginnMindEpoch` keyed `PIPELINE_SCHEMA_EPOCH` whose value equals it, else `MissingIdentity` (none) or `ForeignEpoch { found, expected }` (another).
4. If any envelope exists: exactly one `instance` document, else `MissingIdentity`; its `instance` field equals the declared `instance`, else `ForeignInstance { declared, mind }`. **This is ruling 14's "identity lives in the state, not in a path": the path is derived from the slug for convenience and the document is the authority.** A store moved to another instance's directory is refused.
5. Register the fifteen types, `add_generic_backing_store(store.clone())`, `pull_all_backing_stores()`.

`Mind::open(state_root, instance)` computes the path, calls `OwnedRedbMessagePackBackingStore::new(path)` (creates parent directories, takes the exclusive lock; a second owner fails there and is mapped to `MindAlreadyOwned { path }`), then `open_with`. An empty store is a valid open (`is_empty()`), and only the first admission may write into it.

### Admission, exactly

`admit(batch, now)` prepares every document through the leaf (`validate()`, then `prepare()`) and calls `admit_prepared`. `admit_prepared` is the single commit path for Cuts 8, 10 and 12, in this order; the first failing step is the outcome and nothing after it runs:

| # | Step | Refusal |
|---|---|---|
| A1 | `batch.instance == mind.instance()` | `ForeignInstance { declared, mind }` |
| A2 | 1..=64 envelopes | `BatchSize` |
| A3 | Every envelope: `validate_pipeline_write_envelope` (bounds, formats, key) | `Document(..)` |
| A4 | Identities unique within the batch | `IdentityCollision` |
| A5 | Every document carrying an instance field names this mind: `instance.instance`, `stewardship.instance`; `hand_off.from_instance == mind || hand_off.to_instance == mind` | `ForeignInstance` |
| A6 | Empty mind: the batch contains exactly one `instance` document (else `MissingIdentity`) and admission **derives** the `HuginnMindEpoch` write. Non-empty mind: an `instance` document in the batch collides (A8). | `MissingIdentity` |
| A7 | References: every `PipelineRef` and every full-id `Short` field names a document present in image ∪ batch under `(kind.type_id(), id)`. Fields: `question.raised_in`, `ruling.answers`, `cut_spec.rulings`, `cut_spec.questions`, `cut_report.cut_spec`, `cut_report.forks`, `verdict.cut_report`, `verdict.claims[].findings`, `finding.verdict`, `follow_up.source`, `resolution.subject`, `resolution.outcome.{by,to}`, `hand_off.documents`. `Fixed.commit` and `ForeignRef` are syntax only (ruling B, D6). | `MissingReference { kind, id }` |
| A8 | Per-kind rules, table below, including derivations. Derived writes are appended to the batch and pass A3-A8 themselves. | per row |
| A9 | Replay: compute `receipt_id` over `(instance, strong_reads, writes)`; if a receipt with that id exists, its writes (minus `committed_at`) must equal ours, and the outcome is `AlreadyAdmitted { receipt_id }`. **Validation before replay (ruling 20):** a replayed batch the current rules refuse is refused at A3-A8, never answered from the store. | — |
| A10 | Collision: any write whose `(type, key)` exists in the image | `IdentityCollision { kind, id }`. *(As first written this row also named `AlreadyResolved` for a resolution key, "a subject resolves at most once because its resolution key is outcome-invariant". Cut 6d gave resolutions and stewardships a per-subject sequence, so a key collision is only ever a collision; `AlreadyResolved` and `AlreadyStewarded` are A8's in-force rules now. Corrected 2026-09-16.)* |
| A11 | Commit through `receipt::commit`: `strong_reads` = the exact image envelopes of every document A7 resolved in the image (cited bytes pinned into the receipt); `writes` = the batch plus derived writes; the receipt envelope appended; one `compare_and_swap_batch(expected = strong_reads, replacements = writes + receipt)`. `true` → re-pull the image, `Committed`. `false` → re-pull, diff, `Conflict { identities }`. | `Unavailable` on a store error |

Per-kind rules (A8). "In force" means no resolution that is itself in
force names it in image ∪ batch: the derivation is recursive, so a
withdrawn resolution stops counting (Q17 B, landed in the Cut 6d
follow-up). For the two sequenced kinds, resolution and stewardship, A8
checks sequence then in-force: `sequence` must equal the latest for the
subject (or the `(instance, repo)`) plus one, else `ResolutionOutOfSequence`
/ `StewardshipOutOfSequence`; and no resolution (stewardship) may be in
force for that subject (repo), else `AlreadyResolved` / `AlreadyStewarded`.
A withdrawal cannot itself be withdrawn (Q19 A), read as nesting depth: a
resolution whose subject's own subject is a resolution is refused after
the matrix, so a chain is at most base, closure, withdrawal. *(First
written here as an outcome reading, "a resolution with outcome
`Withdrawn`"; the code and the ruling text implement the nesting reading,
which Soul confirmed. Corrected 2026-09-16.)* Under Self's ruling
extending Q19's rationale, a withdrawal is also refused when the base
subject already has an in-force record later than the one the withdrawal
would reinstate (`WouldReinstateOverLater`), so no base ever has two
records in force.

| Kind | Rule | Refusal |
|---|---|---|
| campaign | `repos` non-empty and every repo is stewarded by this mind (an in-force `stewardship` for `(mind, repo)` exists in image ∪ batch) | `InvalidOptions`-style `FieldBound` comes from the leaf; `RepoNotStewarded { repo }` |
| target | `revision == 1`, or revision N with a `resolution(Superseded{by: this target})` of revision N−1 in the batch; invariant labels unique | `RevisionWithoutSupersession`, `DuplicateLabel` |
| question | ≥ 2 options with unique labels; `recommended` is one of them | `InvalidOptions`, `DuplicateLabel` |
| ruling | `operator_quote.is_some()` ⇒ `authority == Operator` (6c); `answers`, if set, names an in-force question and `choice` is one of its options; admission **derives** `resolution { subject: that question, outcome: Answered { by: this ruling }, rationale: ruling.ruling, resolved_on: ruled_on }` unless the batch already carries an identical one | `QuoteWithoutOperator`, `AlreadyResolved`, `InvalidChoice` |
| cut_spec | `repo ∈ campaign.repos`; every cited ruling in force; revision rule as target, same `cut` | `RepoNotInCampaign`, `CitesResolvedDocument`, `RevisionWithoutSupersession` |
| cut_report | cites its spec (A7) and the spec is in force; `repo` and `branch` equal the spec's; `range.head ∈ commits[].sha` | `CutReportWithoutSpec` (missing), `CitesResolvedDocument`, `SpecMismatch { field }`, `RangeOutsideCommits` |
| verdict | cites its report (A7); each `Falsified` claim cites ≥ 1 `Confirmed` finding in image ∪ batch; an `Unproven` claim cites no `Confirmed` finding; **every promise of the cited report is named by exactly one claim's `promise`** (ruling A); every `claims[].mutations` label exists in the report's `mutations[].label` | `FalsifiedClaimWithoutConfirmedFinding`, `UnprovenClaimWithConfirmedFinding`, `PromiseWithoutVerdict { promise }` (zero or two claims), `UnknownMutationLabel` |
| finding | `range` present (type fact; the refusal exists for the import path's raw envelopes), `evidence` ≥ 1, `locations` ≥ 1; every `invariants[]` label exists in the in-force target of the campaign | `FindingWithoutRange`, `FindingWithoutEvidence`, `UnknownInvariant` |
| follow_up | source exists (A7) | — |
| resolution | subject exists (A7); `sequence` is the subject's latest plus one; no resolution of the subject is in force; outcome fits the matrix below; every `by`/`to` referent exists (A7, named `UnknownSupersessor` when the outcome is `Superseded`) and is in force; `Superseded.by` non-empty; up to 8 supersessors, each named (Q10); the subject is not a withdrawal (Q19 A) | `ResolutionOutOfSequence`, `AlreadyResolved`, `IncompatibleResolution`, `EmptySupersession`, `UnknownSupersessor`, `CitesResolvedDocument` |
| instance | only on an empty mind (A6); `instance == mind` (A5) | — |
| stewardship | `instance == mind` (A5); `sequence` is the `(mind, repo)` latest plus one; no stewardship of the repo is in force on this mind | `StewardshipOutOfSequence`, `AlreadyStewarded` |
| hand_off | one side is this mind (A5). **Source side** (`from == mind`): an in-force `stewardship(mind, repo)` exists, every `documents[]` id exists here (A7), and admission **derives** `resolution { subject: that stewardship, sequence: latest + 1, outcome: Withdrawn { reason: <hand_off key> } }`. **Receiving side** (`to == mind`): admission **derives** `stewardship { instance: mind, repo, sequence: latest + 1, assigned_on: handed_on, note: <hand_off key> }`. A hand-off is a transfer (Q18); a return is a second hand-off, and its derived stewardship takes the next sequence rather than colliding. Cut 12 admits the same `hand_off` into both minds atomically and imports the named documents. | `NotStewarded { repo }` |

Resolution matrix (admission's, this crate; the leaf never refuses a row of it):

| Subject | Allowed outcomes |
|---|---|
| target | `Superseded { by: [target] }` |
| question | `Answered { by: ruling }` (that ruling's `answers` must name this question), `Withdrawn` |
| ruling | `Superseded { by: [ruling…] }` |
| cut_spec | `Superseded { by: [cut_spec, same cut] }`, `Withdrawn` |
| finding | `Fixed { commit, by: Option<cut_report> }`, `Deferred { to: follow_up }`, `Recorded`, `Withdrawn` |
| follow_up | `Fixed { commit, by: Option<cut_report> }`, `Superseded { by: [follow_up] }`, `Withdrawn` |
| stewardship | `Superseded { by: [stewardship] }`, `Withdrawn` |
| resolution (Q11 A) | `Withdrawn` only: withdrawing a resolution reopens its subject; nothing else has a meaning yet |
| campaign, cut_report, verdict, instance, hand_off | not resolvable |

Everything `Superseded` takes a list because the shape does (6c); the matrix constrains the referents' kinds, and each kind's row says which. The old D4's "a supersession chain cannot cycle because `by` must be in force" still holds and needs no cycle check.

### The seam Cut 11 needs

`Committed { writes: Vec<PipelineRef> }` names every document the batch landed, derived writes included, and `Mind::envelope(kind, id)` returns its bytes. Cut 11's daemon indexes *after* `admit` returns, from those two, and never inside it. Cut 8 defines no `IndexPort`, no `EmbeddingPort`, no `pending_index` document and no hook, callback or trait object on `Mind`; D5's ports arrive with Cut 11 in `index.rs`. The negative grep below pins that nothing index-shaped is here.

### Per-file changes

Epiphany, `epiphany-pipeline` at `dddf9ede` (re-anchor by content; the file has not moved since, but Hands greps the opener):

| Line | Change |
|---|---|
| `Cargo.toml:17-21` | Add `anyhow = "1"` and `rmp-serde = "1"`; the comment at `:14-16` says the write path is live and why the two crates are normal dependencies. `:23-26` keeps `serde_json` alone. |
| `lib.rs:28-29` | Module doc: "The wrappers are crate-private: outside code registers, prepares and decodes them through `register_pipeline_document_types`, `PipelineDocument::prepare` and `PipelineDocument::decode`, and validates a write through `validate_pipeline_write_envelope`." |
| `lib.rs:32-39` | `use anyhow::Result; use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry};` with no gates and a one-line comment. |
| `lib.rs:43-47` | `PipelineRefusal` doc: `ForeignStore` is raised by `decode` and stays; the organ's refusals wrap this enum. |
| after `lib.rs:644` (`LOCAL_MAX`) | `pub const PIPELINE_SCHEMA_EPOCH: &str = "epiphany.pipeline.epoch.v1";` with a doc comment carrying the README's rule: additive keeps it, a widened `PipelineKind` is additive, a breaking bump refuses the old store. |
| `lib.rs:475-487` | `prepare`: drop `#[cfg(test)]`, `pub`, doc "Prepares the envelope the organ stores: keyed by `pipeline_key`, payload `[value]` through `prepare_entry_named`." |
| `lib.rs:489-499` | `decode`: drop the gate, `pub`. |
| `lib.rs:502-511` | `register_pipeline_document_types`: drop the gate, `pub`, delete `:509`. |
| `lib.rs:515-520` | `ForeignDocument` doc: it stands in for a receipt of *any* organ; Huginn pins the rule against its real receipt type. Still `cfg(test)`. |
| `lib.rs:739-757` | `validate_pipeline_write_envelope`: drop the gate, `pub`, return `Result<(), PipelineRefusal>`, `.into()` at `:754` removed. |
| `lib.rs:977-981` | `schema_cache`: add `cache.register_entry_type::<ForeignDocument>()?;` after the registrar call. |
| `lib.rs:1072-1076` | `keys_are_derived_and_mismatch_refuses`: `assert_eq!(validate_pipeline_write_envelope(&envelope), Err(PipelineRefusal::InvalidIdentity { .. }))` directly; no `downcast_ref`. |
| `lib.rs:1299` | unchanged (`?` on a `PipelineRefusal` inside an `anyhow` test still compiles). |
| tests, after `:1935` | `every_kind_is_at_the_epochs_version`: `PIPELINE_SCHEMA_EPOCH` ends in `.v1` and every `kind.type_id()` ends in `.v1`; the live registrar registers exactly `PipelineKind::ALL.len()` types (`registered_entry_types().len()`), so the stand-in is not among them. |

`schemas/cultnet/`: no change. `notes/eureka-pipeline-state-cut.md`: Self's.

Huginn at `4094e68`, all new files except:

| Line | Change |
|---|---|
| `crates/huginn-mind/Cargo.toml:8` | the dependency list above; `[dev-dependencies] tempfile = "3"`. |
| `Cargo.lock` | regenerated; gains about 91 packages; the CRLF→LF whole-file rewrite Soul predicted on Cut 7 lands here. |
| `README.md:33-35`, `AGENTS.md:15-17` | "The crates are stubs" → `huginn-mind` is live (storage, identity, admission); the other two are stubs. |
| `crates/huginn-mind/src/lib.rs` | `pub mod admission; pub mod mind; pub mod receipt; pub mod refusal; pub mod store;` and re-exports of the public names above. |
| `crates/huginn-mind/src/{store,mind,receipt,admission,refusal}.rs` | as under Adds. Tests live beside their module. |
| `tools/eureka-cut8-mutations.psd1` | entries H1-H20 below. |

### Authority map

- **Owner:** `huginn-mind`, for the mind's identity, every admission rule, every derivation of a write, and the receipt. Inside it: `Mind::open_with` owns "may this store be this instance's mind"; `Mind::admit_prepared` owns "may this batch enter"; `receipt::commit` owns "did it enter, whole, with a receipt".
- **Inputs:** the store's envelopes (through `MindStore`), the batch, the declared instance, the caller's clock (`now: DateTime<Utc>`, passed in; the crate never reads a clock).
- **Outputs:** `PipelineAdmissionOutcome`, `MindRefusal`, receipts, envelopes by key.
- **Derived state:** the epoch record on the first write; the `Answered` resolution for a ruling that answers; the stewardship withdrawal or assignment on a hand-off; the image (`CultCache`) is a cache of the store and is re-pulled after every commit. In-force status is computed at rule time and never stored (Cut 9 owns the read-side derivation).
- **Forbidden writers:** nothing outside `receipt::commit` constructs a `HuginnCommitReceipt` or calls `compare_and_swap_batch` (one test plants a receipt through `MemoryStore` to pin replay; that is the second construction site and it is deliberate, as in Cut 5). Nothing outside `huginn-mind` validates a document, derives a write, decides in-force status, or writes a mind except through `admit`/`admit_prepared`. The leaf gains no cross-field rule. The daemon, the client and the index may not touch the store. No `Mind` method reads the wall clock or the environment.
- **Shared paths:** `admit` (typed) and `admit_prepared` (envelopes) converge before A1; Cut 10's sink, Cut 12's hand-off and import, and Cut 13's tools all end in `admit_prepared`. Four grammar paths in the leaf stay four.
- **Deletion line:** the Epiphany deletes table, before any Huginn dependency is added.

### Verification

**Builds.** `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex` for both repos, path-list baseline recorded before and after. Epiphany: `cargo check -p epiphany-pipeline --lib --tests`, `cargo test -p epiphany-pipeline --lib` (27 tests, 0 warnings), then `cargo check -p epiphany-core --lib --tests` (untouched; a leaf dependency promotion does not reach it). Huginn: `cargo check -p huginn-mind --lib --tests`, `cargo test -p huginn-mind --lib`, `cargo check --workspace` (the two stubs still build), `cargo tree -p huginn-mind -e normal -d` (zero duplicates; one `cultcache-rs`). Host and target are the workstation; `OwnedRedbMessagePackBackingStore` has a unix and a windows arm and only the windows arm is exercised here; Cut 14 builds on Yggdrasil.

**Tests**, `crates/huginn-mind`, each named for the rule it pins.

| Test | Rule |
|---|---|
| `an_empty_store_opens_and_the_first_write_must_carry_the_instance` | A6, ruling 14: a batch without `instance` into an empty mind is `MissingIdentity`; with it, the receipt's writes name the instance document and the epoch record (derived). |
| `the_instance_document_is_the_identity_not_the_path` | Opener step 4: a store written for `yggdrasil` opened as `thought-cage` (same path) is `ForeignInstance`; a store copied to `minds/thought-cage/` and opened as `yggdrasil` opens. |
| `admission_refuses_a_foreign_instance_whatever_the_transport` | A1, A5: `batch.instance` foreign; a `stewardship` naming another instance; a `hand_off` naming neither side. All `ForeignInstance`, store bytes unchanged. |
| `the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching` | Steps 2-4, in order; each leaves bytes unchanged; a `MemoryStore` counts `pull_all` calls to prove nothing attached. |
| `a_mind_has_one_owner_at_a_time` | Ruling 15: a second `Mind::open` of the same path while the first is alive is `MindAlreadyOwned`; after drop it opens. Real redb in a `tempdir`. |
| `two_minds_in_one_state_root_stay_separate` | Path derivation and store isolation: documents admitted into one are absent from the other; `path_for` differs. |
| `keys_are_recomputed_and_a_forged_key_refuses` | A3 through the shared path: `admit_prepared` with an envelope whose key is edited is `Document(InvalidIdentity)`. |
| `references_must_exist_in_image_or_batch` | A7: a ruling answering an absent question, a follow-up sourced from an absent finding, a verdict citing an absent report, a resolution of an absent subject; each `MissingReference` naming kind and id; a well-formed id of the wrong kind at the same key is also `MissingReference` (no `WrongReferenceKind`). |
| `batch_is_all_or_nothing` | A11 + `RefusingStore`: a refused third document, and separately a CAS that returns `false`, leave the store byte-identical and no receipt written. |
| `exact_replay_returns_already_admitted_across_provenance` | A9: the same documents with a different `provenance` and a later `now` return `AlreadyAdmitted` with the first receipt id; one receipt in the store. |
| `a_refused_batch_is_not_answered_from_a_stored_receipt` | Ruling 20's second half: admit a cut_spec citing R1; supersede R1; replay the first batch byte-for-byte → `CitesResolvedDocument`, not `AlreadyAdmitted`. |
| `a_receipt_names_the_exact_bytes_it_read_and_wrote` | The receipt's `strong_reads` are the cited envelopes' bytes and `writes` the stored bytes, each with a matching `payload_sha256`; `receipt_id` recomputes from `(instance, strong_reads, writes)` and **not** from provenance (two receipts with different provenance and equal content have equal ids). |
| `a_document_is_written_once_and_superseded_by_resolution` | A10 + target invariant: re-putting `ruling:R8` with new text is `IdentityCollision`; superseding it by resolution lands and the old ruling is still readable by key. |
| `subject_resolves_at_most_once` | A10 on a resolution key: `AlreadyResolved { subject }`, whatever the second outcome. |
| `the_resolution_matrix_is_admissions` | Every row: one accepted outcome and one refused per subject kind, `instance` and `hand_off` refused with any outcome, a resolution of a resolution accepted with `Withdrawn` and refused with `Superseded`. |
| `supersession_names_each_supersessor_and_none_is_empty` | 6c inherited + Q10: `Superseded { by: [] }` → `EmptySupersession`; two named supersessors both existing → lands; one absent → `UnknownSupersessor`; one resolved → `CitesResolvedDocument`. |
| `a_ruling_answering_a_question_derives_the_answered_resolution_atomically` | A8 ruling row: the receipt's writes hold ruling and resolution; `choice` not among options → `InvalidChoice`; answering a resolved question → `AlreadyResolved`. |
| `an_operator_quote_requires_operator_authority` | 6c: `QuoteWithoutOperator`. |
| `a_revision_requires_its_predecessors_supersession_in_the_batch` | target and cut_spec rows: `RevisionWithoutSupersession`; with the resolution in the batch, lands. |
| `a_cut_report_cites_an_in_force_spec_and_agrees_with_it` | `CutReportWithoutSpec`, `CitesResolvedDocument`, `SpecMismatch { field: "branch" }`, `RangeOutsideCommits`. |
| `verdict_vocabulary_binds_claims_to_findings_promises_and_mutations` | `FalsifiedClaimWithoutConfirmedFinding`, `UnprovenClaimWithConfirmedFinding`, `PromiseWithoutVerdict` (zero claims and two claims name P1), `UnknownMutationLabel`. |
| `a_finding_names_evidence_locations_and_known_invariants` | `FindingWithoutEvidence`, `FindingWithoutRange` (through `admit_prepared` with a hand-built envelope lacking `range`, since the type makes it unreachable from `admit`), `UnknownInvariant`. |
| `a_campaign_names_only_repos_this_mind_stewards` | `RepoNotStewarded`; with the stewardship in the same batch, lands; a cut_spec whose repo is outside `campaign.repos` → `RepoNotInCampaign`. |
| `a_hand_off_derives_this_minds_side_only` | Source side: withdrawal of the stewardship derived, `NotStewarded` without one, `MissingReference` for an unnamed document; receiving side: stewardship derived; the same `hand_off` admitted into both minds (two `MemoryStore`s) lands in both with the same key. |
| `decode_refuses_the_organs_own_receipt` | S5 closed here: a `HuginnCommitReceipt` envelope through `PipelineDocument::decode` is `ForeignStore { type: "huginn.mind_commit_receipt.v1" }`, against the real type. |

Epiphany, one new test named under Per-file changes; the twenty-six existing unchanged in name and assertion.

**Mutations.** `F:\Projects\Huginn\tools\eureka-cut8-mutations.psd1`, entries H1-H20, run through Epiphany's `tools/eureka-mutations.ps1` with the `-Repo` parameter Q14 adds:

```
powershell -File F:\Projects\Epiphany\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
    -Entries tools/eureka-cut8-mutations.psd1 `
    -Target crates/huginn-mind/src/mind.rs,crates/huginn-mind/src/receipt.rs,crates/huginn-mind/src/admission.rs `
    -Test 'cargo test -p huginn-mind --lib'
```

Anchors are content the entry names; Hands writes them against the code it lands, exactly once each, and M0 is built in. Every entry below is a ruling this cut implements, mutated to restore the old permissiveness, killed by one named test.

| # | Ruling | Mutation, exactly | Killed by |
|---|---|---|---|
| H1 | 14: admission refuses another instance's identity | A1: `if batch.instance != self.instance` → `if false` | `admission_refuses_a_foreign_instance_whatever_the_transport` |
| H2 | 14: identity in the state | Opener step 4: compare the stored instance against the declared one → compare against itself | `the_instance_document_is_the_identity_not_the_path` |
| H3 | 14: a mind is never un-owned | A6: drop the `MissingIdentity` return on an empty mind | `an_empty_store_opens_and_the_first_write_must_carry_the_instance` |
| H4 | 14: the epoch record is derived on the first write | A6: drop the derived `HuginnMindEpoch` write | the same test (the receipt lacks the epoch write); collaterally the opener test on re-open |
| H5 | 15: one writer | `Mind::open`: `OwnedRedbMessagePackBackingStore::new` → `RedbMessagePackBackingStore::new` wrapped (the transient store; requires a `cfg(test)`-free `impl MindStore` for it, which Hands adds only in the mutation, so the entry is `MustNotCompile = $false` and the killer is the second open succeeding) | `a_mind_has_one_owner_at_a_time` |
| H6 | 20: fail-closed opener | Opener: move the attach (`add_generic_backing_store`) above step 2 | `the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching` (the `pull_all` count) |
| H7 | 20: validation before replay | A9 moved above A7 | `a_refused_batch_is_not_answered_from_a_stored_receipt` |
| H8 | Q2/epoch: foreign epoch refused | Opener step 3: `found != PIPELINE_SCHEMA_EPOCH` → `false` | the opener test |
| H9 | Opener: foreign type refused | Step 2: skip the type check | the opener test |
| H10 | Receipt: idempotent replay | A9: skip the lookup (always commit) | `exact_replay_returns_already_admitted_across_provenance` (`IdentityCollision` instead) |
| H11 | Receipt: the digest is content, not author | `receipt_id`: add `provenance` to the digested tuple | `a_receipt_names_the_exact_bytes_it_read_and_wrote` |
| H12 | Receipt: strong reads pin cited bytes | A11: `expected = &[]` | `a_receipt_names_the_exact_bytes_it_read_and_wrote` (empty `strong_reads`) and `batch_is_all_or_nothing`'s CAS branch |
| H13 | All-or-nothing | A11: on `false`, return `Committed` | `batch_is_all_or_nothing` |
| H14 | Supersession, not overwrite | A10: skip the collision check (the store's CAS still refuses, so the outcome becomes `Conflict`) | `a_document_is_written_once_and_superseded_by_resolution` (expects `IdentityCollision`) |
| H15 | Resolution matrix in admission | `matrix(subject_kind, outcome)` → `true` | `the_resolution_matrix_is_admissions` |
| H16 | 6c: `EmptySupersession` | drop the `is_empty` check | `supersession_names_each_supersessor_and_none_is_empty` |
| H17 | Q10 + 6c: `UnknownSupersessor` | resolution referents skipped in A7 | the same test |
| H18 | 6c: `QuoteWithoutOperator` | drop the check | `an_operator_quote_requires_operator_authority` |
| H19 | Ruling A: every promise measured once | `PromiseWithoutVerdict`: `count != 1` → `count == 0` | `verdict_vocabulary_binds_claims_to_findings_promises_and_mutations` (the two-claims forgery) |
| H20 | 6c: `UnknownMutationLabel` | drop the check | the same test |

Stated limits: `ForeignStore` through `decode` is the leaf's rule and is pinned there; H5's mutant needs a helper impl the entry supplies in `New`, which is a two-edit entry on `store.rs` and `mind.rs` (the harness supports `Edits`). `MindAlreadyOwned` is the store's lock, not a Huginn check; H5 pins that Huginn chose the owning store, not that fs2 works.

**Negative greps.**

- `rg -n "reqwest|UdpSocket|qdrant|ollama|cultnet|cultmesh|IndexPort|EmbeddingPort|pending_index" F:\Projects\Huginn\crates\huginn-mind` empty (Cut 11 owns the ports; Cut 10 the socket).
- `rg -n "prepare_entry\(" crates/huginn-mind/src` empty: only `prepare_entry_named`, and only inside `receipt.rs` (the receipt) — pipeline documents are prepared by the leaf.
- `rg -n "compare_and_swap_batch" crates/huginn-mind/src`: the trait, its impl(s), and exactly one call in `receipt.rs`.
- `rg -n "HuginnCommitReceipt \{" crates/huginn-mind/src`: two sites, `receipt::commit` and the replay-planting test.
- `rg -n "Utc::now|SystemTime::now|std::env::var" crates/huginn-mind/src` empty.
- `rg -n "epiphany_core|epiphany-core|ghostlight" F:\Projects\Huginn --glob '!.voidbot/**'` empty; `cargo tree -p huginn-mind -e normal | rg -c "epiphany-core"` is 0.
- `rg -n "cfg\(test\)" F:\Projects\Epiphany\epiphany-pipeline\src\lib.rs` shows only `ForeignDocument`, `derived_schema` and `mod tests`.
- `rg -n "impl Bounded|fn validate" crates/huginn-mind/src` shows no impl over a leaf type (the leaf's `Bounded` is `pub(crate)`; if Hands needs it, that is a finding, not a workaround).
- `git diff --stat dddf9ede -- schemas/cultnet/` empty. `git diff --stat 4094e68 -- .voidbot` empty.

**Operator checks:** none blocking. Q13 and Q14 are asked below; both have a default Hands can build under.

### Subtraction estimate

Epiphany: −28 lines (gates, comments, one registration, two dev-dependency lines), +about 40 (constant, doc lines, one test, two dependency lines); net about +12 outside tests, +15 tests. Zero schema, kind, field, type id, epoch or lock change. Liability retired: the last `cfg(test)`-shaped "until the organ" waivers, and the drift hazard of an epoch string nobody owned in code.

Huginn: +about 2,100 lines (store 120, mind 260, receipt 220, refusal 90, admission 700, tests 650, entries file 120), +1 live crate, +8 direct dependencies, about +91 lock packages, +2 document types (`huginn.mind_epoch.v1`, `huginn.mind_commit_receipt.v1`), zero binaries, zero targets. Of the receipt file, about 200 lines duplicate `epiphany-core`'s 260; the rest is the service boundary D1 chose. Nothing in Huginn is removed because nothing is there.

### Build budget

- **Packages that compile:** Epiphany `epiphany-pipeline` (lib + tests); `epiphany-core` check only, no rebuild expected since the leaf is not its dependency. Huginn `huginn-mind` (lib + tests) and its 90 transitive packages, the two stubs.
- **Profiles/targets/platforms:** debug only, workstation host = target, no features, no codegen paths, no release profile.
- **Footprint:** `C:\Users\Meta\.cargo-target-codex` measured this pass at **9,872 paths, 8.2 GiB (`debug/` only)**, drive C: 261 GiB free. `libcultcache_rs`, `libredb` and `libepiphany_pipeline` rlibs are already present from Epiphany builds; Huginn's feature unification may or may not reuse them. **Expected delta: +400 to +900 paths, +0.4 to +0.9 GiB**, all under `debug/`. Hands records the before/after path list as every cut has, and reports a miss.
- **Retention:** the shared dir is the operator's; nothing is deleted by this cut.

### Operator questions

- **Q13. Where do the batch and outcome types live for the client?** `eureka-state` (Cut 13) must construct `PipelineAdmissionBatch` and read `PipelineAdmissionOutcome`/`MindRefusal`, and the map says it does not depend on `huginn-mind` ("would invite a second validator", Cut 13). Cut 10 puts the wire types in the `huginn-daemon` *binary* crate, which a client cannot import either. Options: **A.** `eureka-state` depends on `huginn-mind` for types, with a negative grep that it never calls `open`, `admit` or `admit_prepared`; the graph cost is one package beyond what the leaf already brings (`redb` comes with the leaf regardless). **B.** A fourth crate `huginn-wire` holding only the request/response/outcome types; doctrine's "a named external consumer with a hard dependency boundary" argument (D1) applies weakly, since the boundary is a discipline a grep can pin. **C.** Put them in `epiphany-pipeline`; refused here because D6 already rules Huginn's transport contracts are Huginn's. **Recommended: A.** Cut 8 makes the types `pub` and `JsonSchema` so A or B both work; the choice is Cut 10/13's, but it should be ruled before Cut 10 writes `wire.rs` into a binary crate. **Ruled A, 2026-09-16** ("I'll take your recommendations"). Cut 13's "no dependency on `huginn-mind`" becomes "depends on it for types only, with a negative grep that it never calls `open`, `admit` or `admit_prepared`"; Cut 10's `wire.rs` moves out of the binary crate accordingly when Cut 10 is refreshed.
- **Q14. How does Huginn run the mutation harness?** **A.** Add `-Repo` (default: the harness's own parent) to `tools/eureka-mutations.ps1` in Epiphany, about four lines, and keep one harness; Huginn holds only its entries file and its verification names Epiphany's path. **B.** Copy the harness into Huginn: a second copy of a 19 KB script the operator will notice, one bug away from two behaviours. **C.** Move the harness to the Eureka skill repo (`GameCult/Eureka`), the owner of cross-repo Eureka tooling, and point both repos at `~/.claude/skills/eureka/tools/`: coherent, three repos touched, and Cut 15 ("Skill wiring") is its natural home. **Recommended: A now, C in Cut 15.** *Taken as a default by Self, 2026-09-16: not a product fork.*
- **Q15. `mind.redb`, not `mind.cc`.** D3 wrote `mind.cc`; `F:\Projects\CLAUDE.md` says "all state should be CultCache `.cc` files". A redb-backed CultCache store is CultCache state in a different container, and the extension is what Epiphany's backend selector and CultCache Studio key on. **Recommended: `mind.redb`**, and the doctrine sentence reads "CultCache state", which this is. If the operator wants `.cc` literally, the cost is Epiphany's selector convention and Studio misreading the file, both real. **Ruled `mind.redb`, 2026-09-16.** The operator asked how redb got in: it entered Epiphany's runtime spine as the keyed backing store on 2026-08-08 (`5d6a7316`) and CultLib with the return of the Rust CultCache engine on 2026-09-03 (`b672fb7`); it is CultCache's own embedded keyed backend at the shared pin, not a dependency this campaign added.
- **Q16 (carried, not new). Q6/ruling 18 stands:** identity is declared. This spec builds no credential path; `ForeignInstance` is collision and attribution control (D8). *Not a question: ruling 18 is in force and nothing here reopens it. Left as written so the reader sees it was checked.*

### Findings not assignable to a cut

- **FU-3's number is stale:** the Epiphany receipt path is about 260 lines, not 120, and Huginn's copy is about 200 because it drops authority variants, companions, `invariant_owner` and `store_id`. Self corrects the follow-up text.
- **`open_runtime_spine_cache` (`runtime_spine.rs:646-653`) declares `SingleFileMessagePackBackingStore` while `commit_authorized_mind_mutation` (`reasoning_context.rs:1612`) passes a `RuntimeSpineBackingStore`;** one of the two is not what it reads as, or a conversion hides between them. Not read further because it is Epiphany's Mind path and Soul is in that tree; it belongs to campaign two or a Mind Steward note, not to Cut 8.
- **The map's Cut 10 puts `HuginnMindRequest`/`HuginnMindResponse` in the daemon binary crate and Cut 13 forbids a `huginn-mind` dependency** — the client then has no crate to get the wire types from. Q13 above; the fix is Cut 10's spec.
- **`README.md:33-35` and `AGENTS.md:15-17` in Huginn go stale the moment commit 1 lands**; the per-file table rewords them, but the "describe the live system" tension Soul raised on Cut 7 recurs at every Huginn cut until the workspace is whole.
- **Huginn's `.gitignore` still carries `node_modules/`** (Cut 7 correction 27); one line, not this cut's.
- **The old D4 rule text survives only in git history** (`304832ad^:notes/eureka-pipeline-state-cut.md:546-632`); the live map has the refusal names and no rules. This spec restates them in full; Self should make the map's Cut 8 section the owner so the next reader does not need `git show`.

### Pinned HEADs

Epiphany `ca7e230c` (lib.rs at `dddf9ede`), Huginn `4094e68`, CultLib `main` `4a2fdaf` with the Rust runtimes identical to `a0813c6`, Eureka skill checkout untouched. Probe artifacts: the lockfile resolution ran in the session scratchpad (`pin-probe/`, removed after this spec was written) and compiled nothing. The shared target dir read 9,872 paths before this pass and 9,874 after; this pass ran no cargo build, so the two paths belong to a concurrent build (Soul is in the Epiphany tree). Both repo trees are clean at the pinned HEADs.

## Cut 9. `huginn-mind`: queries and derivations

Imagination, 2026-09-16, refreshed against the Body after the Cut 6d Huginn
follow-up and its fix batch. Replaces the old Cut 9 section whole; the
paragraph "Carried in from the Q17 ruling and the Cut 6d follow-up" is kept
as requirements R-A to R-C below.

### Pins

| Repo | Branch | HEAD | State |
|---|---|---|---|
| Epiphany | `codex/eureka-pipeline-state` | `94df3a8f` | tree clean; map only since `d5a36c2a`. |
| Epiphany leaf | `epiphany-pipeline/src/lib.rs` | `d5a36c2a` (2,366 lines) | the rev Huginn pins in `crates/huginn-mind/Cargo.toml`. |
| Huginn | `eureka/memory-organ` | `7b67730` | **an uncommitted Hands batch is in the tree** (`crates/huginn-mind/src/admission.rs`, `tools/eureka-cut8-mutations.psd1`: a subject-selection fix in `derive`, a dead arm deleted, four tests). Every anchor in this spec is a function, type or test **name**, never a line; Hands re-anchors by content after that batch lands, and this cut lands after it. |
| CultLib | `a0813c6` | `cultcache-rs` checkout `~/.cargo/git/checkouts/cultlib-7ab3069e1ba2db32/a0813c6` | read for `CultCacheEnvelope` only. |

Body facts this spec rests on, each by source read of the files named:

- `admission.rs` holds `Docs { image: Vec<Held>, batch: Vec<Staged> }` and
  every derivation: `from_image`, `push`, `in_batch`, `in_image`, `find`,
  `of_kind`, `resolutions`, `stewardships`, `latest_resolution`,
  `derived_resolution_sequence`, `derived_stewardship_sequence`,
  `latest_stewardship`, `in_force`, `in_force_unless`, `stewardships_of`,
  `later_in_force`, `stewardship_of`, and the free fn `later_than`. Nothing
  else in the crate decides in force.
- `in_force_unless(kind, id, own)` is the recursive derivation of Cut 6d D4:
  no resolution `R` naming `(kind, id)`, other than `own`, such that
  `in_force(Resolution, key(R))`. It returns `bool` and does not say *which*
  resolution closes the document; the views need that resolution.
- `Mind` exposes `envelopes()` (the image, sorted `(type, key)`),
  `envelope(kind, id)`, `get(kind, id)` (one typed decode), `receipts()`
  (every `HuginnCommitReceipt`, decoded from the image), `instance()`,
  `is_empty()`; `raw_envelope` and `cache` are `pub(crate)`.
- A receipt names its `writes: Vec<DocumentVersion>` by `(document_type,
  document_key)` and its `strong_reads` separately; `committed_at` is the
  `now` admission was passed, RFC3339 seconds UTC (`receipt::candidate`); a
  strong read is re-inserted unchanged in the swap but recorded under
  `strong_reads`, never `writes` (`receipt::commit`). A10 refuses a write
  whose identity the image holds, so **exactly one receipt names any stored
  document as a write**.
- `PipelineProvenance { faculty: Faculty, agent, session, tool }` and
  `Faculty` derive `Serialize`, `Deserialize`, `JsonSchema` (`receipt.rs`).
- At `d5a36c2a` the leaf's `PipelineDocument` (adjacently tagged `{ kind,
  value }`), `PipelineRef`, `PipelineKind`, `ResolutionOutcome`, every
  value type and `PipelineRefusal` derive `Serialize`, `Deserialize` and
  `JsonSchema`. So a view carrying a `PipelineDocument` derives `JsonSchema`
  (Q13 A holds with no leaf change).
- `CultCacheEnvelope { key, type, payload, stored_at: String, schema_id }`.
  `rg stored_at crates/huginn-mind/src` is empty today.
- Baseline (probe): detached worktree at `7b67730` under the scratchpad,
  `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`, `cargo test -p
  huginn-mind --lib`: **41 passed, 0 failed, 5.5 s wall warm**; target dir
  10,579 paths before and after; worktree removed; main tree untouched.

### What the cut does

The read side of a mind, in `huginn-mind`, testable over a `&Mind<MemoryStore>`
with no store file, daemon or index:

- **Views.** `PipelineDocumentView { id, document, admission, status }`: the
  document, the admission facts joined from the one receipt that wrote it
  (`receipt_id`, `admitted_at`, `provenance` whole), and its status derived
  at read time (`InForce`, or `Resolved` by a named in-force resolution).
- **Queries.** `Mind::view(&PipelineRef)`, `Mind::query(&PipelineQuery)`
  (by campaign root, repo, cut, kinds, in-force, faculty, admission window;
  capped at 200, ordered by `(admitted_at, id)`, with the match count so a
  truncation is visible), `Mind::open_items(&campaign)`, and
  `Mind::history(&HistoryScope)` — every resolution a subject ever had, or
  every assignment of a repo on this mind, in sequence order, withdrawn ones
  included with their status and reasons (R-B).
- **One owner for in force.** `Docs` and every derivation move whole from
  `admission.rs` into `src/docs.rs`. The views read through the same
  `Docs`; the one helper they need that admission lacks
  (`closing_resolution_unless`, the resolution that closes a document)
  becomes the body of `in_force_unless`, so admission calls it too. No
  derivation reads `stored_at` (R-C).
- **`semantic` is accepted and refused typed** (`Unavailable`) until Cut 11.

Requirements carried in, restated so Hands reads them here:

- **R-A (Q17 B, Cut 6d follow-up).** Status is the recursive in-force
  derivation admission uses; a test pinning the non-recursive form pins
  the defect. The reopen case (n1, its withdrawal, n2) is in the status test.
- **R-B (Q17's two obligations).** A subject's resolution history is a
  first-class view with withdrawn records and their reasons; the index
  (Cut 11) gets a document-plus-admission source, which is `Mind::view` per
  landed `PipelineRef`, so withdrawals are indexable documents with their
  `reason` in the payload.
- **R-C (Soul, Cut 8 Huginn half).** `stored_at` is the store's stamp;
  `admitted_at` is the receipt's `committed_at`. Pinned by a runtime test
  that scrambles `stored_at` and by a negative grep.
- **Soul's standing findings the views must not hide:** history lands one
  record per batch (Cut 12's import constraint), so two records of one
  scope carry two receipts; the history test asserts distinct `receipt_id`s.
  Replay refuses once a scope has advanced (ruling 20); the views do not
  touch replay and add no path around it.

### What changed against the old Cut 9 section

1. **`Docs` moves; nothing is copied.** The old section had "Deletes first:
   none" and a `src/query.rs` with its own derivations. That would be a
   second in-force. The shared module is the cut's first commit.
2. **Five functions collapse to four, differently cut.** `get` becomes
   `view` (a document with its facts and status); `rulings_in_force` and
   `stewardship` are `query` presets (`kinds: [Ruling], in_force: true,
   campaign` and `kinds: [Stewardship], in_force: true`) and are deleted
   from this crate's surface; `history` is new (R-B); `open_items` stays.
3. **`PipelineQuery` sheds three fields.** `instance` (the mind is the
   instance; D6's request carries it and the daemon routes on it),
   `outcome` (`history` carries outcomes; a document's closing outcome is in
   its `status`), `text_contains` (Q21 below; Cut 11 owns which fields are
   text). `status` becomes `in_force: Option<bool>`.
4. **The view is four fields, not seven.** `receipt_id`, `admitted_at` and
   `faculty` fold into `admission: AdmissionFacts` carrying the whole
   provenance; `resolution: Option<(id, PipelineResolution)>` becomes
   `status: PipelineStatus`.
5. **A query result says how many matched.** `PipelineQueryPage { items,
   matched }`; the cap never truncates silently.
6. **A document with no receipt is refused, not shown.** The store is the
   organ's alone; a row admission did not write is an integrity fault
   (`Unavailable`), the same answer `Docs::from_image` gives a row that does
   not decode.
7. **Tests 6 → 10, mutations 3 → 23**, in `tools/eureka-cut9-mutations.psd1`;
   three cut-8 entries re-anchor to `docs.rs`.

### Decisions, with the reasons

#### D1. The shared derivation module is a move of `Docs`, not a new abstraction

`Docs` already is the thing: "the image and the batch, the two places a rule
may look". The views look at the image alone, which is `Docs` with an empty
batch, built by the same `Docs::from_image(mind.envelopes())`. Moving the
struct and its `impl` into `src/docs.rs` (`pub(crate)`, fields `pub(crate)`
so H49's mutant in `check` still compiles) changes no derivation and no
call site: `admission.rs` keeps `stage`, `refuse_foreign_instance`,
`references`, `resolve`, `derive`, `check`, `revision_rule`,
`unique_labels`, `outcome_name`, `matrix`, `resolution_rule`,
`refuse_collisions`, `kind_of_id`, and every `docs.` call reads as before.
`later_than` moves because only `later_in_force` calls it. `kind_of_type`
moves with `from_image`.

Rejected: a trait over "a document set" implemented by the image and by
image ∪ batch. It would be one implementor with a flag; `Docs` with an
empty batch is that already.

#### D2. The one new derivation: which resolution closes a document

`in_force_unless` answers "is any in-force resolution naming this document
other than `own`?" as a `bool`. The views need the resolution itself for
`PipelineStatus::Resolved`. So:

```
fn closing_resolution_unless(&self, kind, id, own) -> Option<(&str, &PipelineResolution)>
    = self.resolutions().find(|(key, r)| r.subject.kind == kind && r.subject.id.0 == id
                                       && !own(r) && self.in_force(Resolution, key))
fn closing_resolution(&self, kind, id) -> Option<(&str, &PipelineResolution)>
    = self.closing_resolution_unless(kind, id, |_| false)
fn in_force_unless(&self, kind, id, own) -> bool
    = self.closing_resolution_unless(kind, id, own).is_none()
```

`find` is exact, not a tie-break: under `AlreadyResolved` (A8) and the Q19
cap at most one resolution of a subject is in force. Admission's callers of
`in_force`/`in_force_unless` change nothing and now run through the same
`find` the views do; H41's mutant lands inside `closing_resolution_unless`
and kills admission's reopen test and the views' status test alike, which is
the proof that there is one owner (V1 below).

Two small extractions for the history views, so that `history` shares
admission's scope selection rather than restating it:

```
fn resolutions_of(&self, subject: &PipelineRef) -> impl Iterator<Item = (&str, &PipelineResolution)>
    = self.resolutions().filter(|(_, r)| r.subject == *subject)
fn assignments_of(&self, mind: &Slug, repo: &OrgRepo) -> impl Iterator<Item = (&str, &PipelineStewardship)>
    = self.stewardships().filter(|(_, s)| s.instance == *mind && s.repo == *repo)
```

`latest_resolution` becomes `resolutions_of(subject).filter(own != …).map(sequence).max()`;
`latest_stewardship` and `stewardships_of` (the in-force ones; name kept)
filter `assignments_of(mind, repo)`. Same results, one scope selector each.
H47 and H60 anchor lines inside these two and re-anchor (Per-file changes).

The history is selected by the `subject` field, as `latest_resolution`
already does, not by the `…<kind>.<local>.n` key prefix. The leaf's test
`a_subjects_resolutions_share_a_prefix_no_other_key_has` pins that the two
select the same set; the prefix is the storage-level affordance (Studio, a
raw scan), the field is the organ's, and the views parse no key part for it.

#### D3. Admission facts come from receipts, from `writes` only

```
pub struct AdmissionFacts { pub receipt_id: String, pub admitted_at: String, pub provenance: PipelineProvenance }
```

Built once per read as `AdmissionIndex` (`pub(crate)`, in `query.rs`): a
`BTreeMap<(String, String), AdmissionFacts>` over `mind.receipts()?`,
inserting every `receipt.writes[].identity()` with `admitted_at =
receipt.committed_at`. `strong_reads` are never read: a document cited by a
later batch keeps its first receipt. Two receipts naming one write, or a
pipeline document in the image that no receipt names, is
`Unavailable { detail: "document <type>/<key> has no commit receipt" | "… is written by two receipts" }`,
raised from `view` and `query` alike, never a silent skip (V4). The epoch
record and the receipts themselves appear in `writes` and are simply never
looked up.

`admitted_at` is the caller's `now` as admission stored it (`committed_at`,
RFC3339 `Z`, seconds), so its string order is its time order; the crate
still reads no clock. `stored_at` is not consulted anywhere (R-C; V2).

Rejected: `admission: Option<AdmissionFacts>` with `None` for an
unreceipted row. That is the "show it and let the reader guess" shape; the
store has one writer and the receipt is its proof.

#### D4. The view is a struct of four fields

A `(document, facts, status)` tuple serialises as a positional JSON array
and gives Cut 13's tool schema three unnamed slots, and it cannot carry the
id: a `PipelineDocument` does not carry its key, the client must not derive
keys (Cut 13's negative grep), and every follow-up call (`history`,
`view`) takes a `PipelineRef`. So:

```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineDocumentView {
    pub id: PipelineRef,
    pub document: PipelineDocument,
    pub admission: AdmissionFacts,
    pub status: PipelineStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PipelineStatus {
    InForce,
    Resolved { resolution: PipelineRef, record: PipelineResolution },
}
```

`Resolved.record` is the whole closing resolution (outcome, rationale,
`resolved_on`) because rehydration wants why, not only that. Kinds the
matrix makes unresolvable (campaign, cut_report, verdict, instance,
hand_off) are always `InForce`, by the same derivation and no special case.
A withdrawn resolution's own view is `Resolved { resolution: <its
withdrawal>, record: { outcome: Withdrawn { reason }, .. } }`, which is how
R-B's "with their reasons" reaches an agent.

#### D5. The query type

```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineQuery {
    pub campaign: Option<Slug>,        // the key's root: a campaign slug, or an instance for stewardships and hand-offs
    pub repo: Option<OrgRepo>,
    pub cut: Option<Label>,
    pub kinds: Vec<PipelineKind>,      // empty = every kind
    pub in_force: Option<bool>,
    pub faculty: Option<Faculty>,      // attribution, ruling 18: it filters, it grants nothing
    pub admitted_after: Option<Short>, // exclusive, string-compared against `admitted_at`
    pub admitted_before: Option<Short>,// exclusive
    pub limit: Option<u32>,            // None = QUERY_LIMIT_MAX; clamped to 1..=QUERY_LIMIT_MAX
    pub semantic: Option<SemanticQuery>,
}
pub struct SemanticQuery { pub text: Line, pub top_k: u32 }
pub struct PipelineQueryPage { pub items: Vec<PipelineDocumentView>, pub matched: u32 }
pub const QUERY_LIMIT_MAX: usize = 200;
```

Filter semantics, each one rule:

| Filter | Matches |
|---|---|
| `campaign` | the key's root segment equals it. The grammar is `<root>:<kind>:<local>`, three segments for every kind (`pipeline_id`; leaf test `every_key_has_exactly_three_segments`), and `kind_of_id` in admission already reads the kind segment the same way. A resolution's root is its subject's root, so a campaign's resolutions match with it. |
| `repo` | `campaign.repos` contains it; `cut_spec.repo`, `cut_report.repo`, `follow_up.repo`, `stewardship.repo`, `hand_off.repo` equal it; a resolution matches when its **base** matches. target, question, ruling, verdict, finding, instance carry no repo and never match. |
| `cut` | the local begins with `cut-<label>.` (cut_spec `cut-<label>.r<N>`, cut_report `.h<N>`, verdict `.s<N>`, finding `.s<N>.<label>`: the leaf's `parent_cut` guarantees the prefix), or a resolution whose base matches. No walking of references; the key already carries the chain. |
| `kinds` | `document.kind()` is in the list; empty list matches all. |
| `in_force` | `status` is `InForce` (true) or `Resolved` (false). |
| `faculty` | `admission.provenance.faculty` equals it. |
| `admitted_after` / `admitted_before` | `admitted_at > after`, `admitted_at < before`, plain string comparison; a caller passes RFC3339 UTC (a date alone, `2026-09-16`, compares correctly against `2026-09-16T…`). |
| `semantic` | refused before any filter runs: `Err(Unavailable { detail: "semantic query: the index is not wired (Cut 11)" })`. Typed, never empty. |

**Base of a resolution:** follow `subject` while the subject is a resolution
(depth ≤ 2 by Q19 A); the base is the first non-resolution document, found
through `Docs::find`. A7 guarantees it is in the image.

Ordering: `(admitted_at, id)` ascending, `id` the key string; ties within one
batch (same `committed_at`) fall to key order, which is root-first and so
differs from the image's `(type, key)` order (the seed batch orders
`eureka-state:campaign:self`, `eureka-state:target:r1`,
`yggdrasil:instance:self`, `yggdrasil:stewardship:…` by id and
campaign, instance, stewardship, target by type). `matched` is the count
before the cut. Stated limit: no cursor; a caller pages by
`admitted_after` at second granularity and sees `matched > items.len()`
when a page is short. Exact paging is a follow-up if a proof campaign ever
shows a query over 200 matches (Findings).

`instance` is not a query field: `query` takes `&self` on one mind. The
daemon selects the mind from the request's `instance` (D6) before it calls
anything here.

#### D6. `open_items`

```
pub struct PipelineOpenItems {
    pub questions: Vec<PipelineDocumentView>,
    pub findings: Vec<PipelineDocumentView>,
    pub follow_ups: Vec<PipelineDocumentView>,
    pub specs_without_report: Vec<PipelineDocumentView>,
    pub reports_without_verdict: Vec<PipelineDocumentView>,
}
```

`Mind::open_items(&self, campaign: &Slug)`: root equals the campaign for
every list; `questions`, `findings`, `follow_ups` are those kinds in force;
`specs_without_report` are in-force cut specs no cut report's `cut_spec`
names (a superseded `r1` is not open; `r2` with no report is); `reports_without_verdict`
are cut reports no verdict's `cut_report` names (reports are not
resolvable, so in-force is not a condition). Each list in query order,
uncapped: the open set of one campaign is bounded by the campaign and
truncating it would hide work.

#### D7. `history`

```
pub enum HistoryScope { Subject(PipelineRef), Repo(OrgRepo) }
```

`Mind::history(&self, scope)`: `Subject` → `resolutions_of(subject)`;
`Repo` → `assignments_of(self.instance(), repo)`. Views, ordered by the
record's `sequence` ascending (the sequence is the owner of history order;
`admitted_at` coincides but is not the rule), uncapped. A resolution's own
withdrawals are not in its subject's history (their subject is the
resolution); `history(Subject(<resolution id>))` lists them. Each record
carries its own receipt (one per batch).

#### D8. Where the code sits, and dependency injection

`src/query.rs`: the public types above, `AdmissionIndex`, a private
`Reader<'a> { docs: Docs, facts: AdmissionIndex, mind: &'a Mind<S> }` built
once per call, and `impl<S: MindStore> Mind<S> { view, query, open_items,
history }`, following `admission.rs`'s `impl Mind` shape. Every function
takes `&self`; a test opens `Mind::open_with(MemoryStore::new(), …)` and
admits through the fixtures. No store file, no daemon, no clock, no new
dependency; `schemars` and `serde` are already direct dependencies.

`Mind::get(kind, id)` stays: the typed one-document read the opener's
tests and Cut 12's import use. `view` is the read with facts and status,
and is what Cut 10's `Get`, Cut 13's `get` and Cut 11's index source call.

Cost, stated: each read call decodes the image once (`Docs::from_image`)
and every receipt once, as each admission already does. Holding a decoded
image on `Mind` (refreshed in `refresh`, refused by the opener when a row
does not decode) would remove both and is recorded under Findings, not
done here.

### Deletes and moves first

| Path (by name) | Lines | What |
|---|---:|---|
| `admission.rs`: `struct Staged`, `struct Held`, `struct Docs`, `fn kind_of_type`, `impl Docs` (every method), `fn later_than` | about 170 | **Move** to `src/docs.rs`. Byte-identical bodies except the three edits in D2 (`closing_resolution_unless` extracted from `in_force_unless`; `resolutions_of`/`assignments_of` extracted from `latest_resolution`, `latest_stewardship`, `stewardships_of`). Fields of `Docs`, `Held`, `Staged` become `pub(crate)`. |
| `admission.rs` module doc, the sentence beginning `"In force" is recursive` | 3 | Moves to `docs.rs`'s module doc; admission's says where in force lives. |
| the old Cut 9 section's `rulings_in_force` and `stewardship(instance)` | — | Not built. Presets of `query` (D5); Cut 10's `RulingsInForce`/`Stewardship` operations and Cut 13's two tools become presets when those cuts are refreshed (Findings). |
| the old section's `PipelineQuery.{instance, status, outcome, text_contains}` | — | Not built (D5, Q21). |

Nothing in the leaf. Nothing in `Cargo.toml`, `Cargo.lock`, the two stubs.

### Keeps

Every admission rule, refusal variant, test name and cut-8 entry (three
re-anchored). `Mind::get`, `envelopes`, `envelope`, `receipts`. The
`#[serde(remote)]` mirror in `refusal.rs` is untouched here though dead
(Findings; Cut 10's). The leaf pin `d5a36c2a`.

### Adds

| Add | Owner | Live consumer | Protected invariant | Why an existing owner cannot serve |
|---|---|---|---|---|
| `src/docs.rs` (`pub(crate)`): the moved `Docs` plus `closing_resolution_unless`, `closing_resolution`, `resolutions_of`, `assignments_of` | `huginn-mind` | `admission.rs` (every rule) and `query.rs` (every view) | **One derivation of in force, of a subject's history and of a repo's assignments, shared by admission and the views**; status is computed at read time and never stored | It is the existing owner, moved so a second module can reach it without `admission.rs` exporting its internals. |
| `PipelineDocumentView`, `AdmissionFacts`, `PipelineStatus` (`pub`, `JsonSchema`) | `huginn-mind` | Cut 10's `Get`/`Query` responses; Cut 11's index source (`view` per landed ref); Cut 13's `get`/`query`/`open_items` outputs (Q13 A) | A client never re-derives status or joins receipts; the id travels with the document | Nothing carries id + document + facts + status; the leaf owns no receipt and no status. |
| `PipelineQuery`, `SemanticQuery`, `PipelineQueryPage`, `QUERY_LIMIT_MAX` | `huginn-mind` | Cut 10's `Query`; Cut 13's `query` | The cap and the stable order; a truncation is visible; `semantic` is typed-refused until Cut 11 | — |
| `PipelineOpenItems`, `HistoryScope` | `huginn-mind` | Cut 13's `open_items`; a new Cut 13 `history` tool (Findings, Cut 13's refresh) | Open work and a scope's history are derived, not maintained | — |
| `AdmissionIndex` (`pub(crate)`) | `query.rs` | the four read functions | Facts come from `writes` only; an unreceipted row refuses | The receipt is `receipt.rs`'s; this is its read side. |
| `impl<S: MindStore> Mind<S> { view, query, open_items, history }` | `huginn-mind` | as above | D1's injection: testable over `MemoryStore` | — |
| fixtures: `admit_at(mind, documents, now)`, `admit_as(mind, faculty, documents)`, `question_n(label)` → `question(&format!("Q{n}"), &["A","B"], "A")` | tests | the ordering, faculty and cap tests | — | `admit` fixes clock and faculty. |
| ten tests, `tools/eureka-cut9-mutations.psd1` (V1-V23) | the crate's suite | Epiphany's harness with `-Repo` | every rule above has a runtime mutant | — |

No dependency, target, binary, document type, schema, kind, format or
epoch. `refusal.rs` gains no variant: `Unavailable { detail }` carries the
two integrity cases and the semantic refusal, as it carries every other
"the organ cannot answer" case.

### Per-file changes, by name

**`crates/huginn-mind/src/docs.rs`** (new). Module doc: "The image and the
batch, the two places a rule or a view may look, and every derivation over
them. *In force* is recursive: a document is in force when no resolution that
is itself in force names it, so a withdrawn resolution stops closing its
subject. Computed over image and batch at rule time, over the image at read
time, never stored, and never from `stored_at`." Then, moved: `Staged`,
`Held`, `Docs` (fields `pub(crate)`), `kind_of_type`, `impl Docs` with the
D2 edits, `later_than`. `in_force_unless`'s doc comment stays on it;
`closing_resolution_unless` gets: "The in-force resolution naming the
document, other than `own`, if any; exact rather than a tie-break because
A8 and the Q19 cap admit at most one."

**`crates/huginn-mind/src/admission.rs`**: `use crate::docs::{Docs, Staged};`
(and `Held` only if a remaining fn names it; today none does). The moved
block deleted. Module doc: the "In force is recursive" sentence replaced by
"In force, the sequences and the scopes are `docs.rs`'s, shared with the
views (Cut 9)." Every `docs.` call unchanged. The batch in flight touches
`derive`; nothing here overlaps it except by import.

**`crates/huginn-mind/src/query.rs`** (new): D3-D8. `Reader::new(mind)`
calls `Docs::from_image(mind.envelopes())?` and `AdmissionIndex::build(mind)?`;
`Reader::view_of(&Held) -> Result<PipelineDocumentView, MindRefusal>`
(facts lookup, `closing_resolution` → status); `Reader::base(&Held) ->
&Held` for resolutions; `Reader::matches(&PipelineQuery, &Held, &view)`;
`fn ordered(views) -> Vec<…>` sorting by `(admission.admitted_at, id.id.0)`.
`query`: `semantic` check first; collect matching views; `matched = len`;
sort; truncate to the clamped limit.

**`crates/huginn-mind/src/lib.rs`**: `mod docs;` (private), `pub mod query;`,
re-exports `pub use query::{AdmissionFacts, HistoryScope, PipelineDocumentView,
PipelineOpenItems, PipelineQuery, PipelineQueryPage, PipelineStatus,
SemanticQuery, QUERY_LIMIT_MAX};`. Module doc: after the three decisions,
one sentence: "The read side derives status, joins the receipts and answers
typed queries through the same `docs` the rules use; nothing is stored for
it."

**`crates/huginn-mind/src/fixtures.rs`**: the three helpers above; `admit`
becomes `admit_at(mind, documents, now())`.

**`tools/eureka-cut8-mutations.psd1`**: header `-Target` gains
`crates/huginn-mind/src/docs.rs`. H41: `File` → `docs.rs`; `Old` is the
`&& !own(resolution)` / `&& self.in_force(PipelineKind::Resolution, key)`
pair as it lands in `closing_resolution_unless`. H47: `File` → `docs.rs`;
`Old`/`New` re-anchored on `stewardships_of`'s filter as it lands over
`assignments_of` (the mutant still drops the `in_force_unless` conjunct).
H60: `File` → `docs.rs`; `Old` re-anchored on `latest_resolution`'s body
over `resolutions_of` (the mutant still reads `self.image` only). H49 is
unchanged (its `New` reads `docs.image`, which `pub(crate)` keeps legal).
Every other entry anchors code that does not move (Hands greps each `Old`
once after the move; the harness refuses a zero-match anchor anyway).

**`tools/eureka-cut9-mutations.psd1`** (new): V1-V23 below.

**`README.md`, `AGENTS.md`**: the live-crate sentence gains "queries and
derived status"; no present-tense claim about Cuts 10-13.

### Authority map

- **Owner:** `huginn-mind`. Inside it, `docs::Docs` owns every derivation
  (in force, the closing resolution, the sequences, the scopes);
  `query::AdmissionIndex` owns the join of admission facts;
  `Mind::{view, query, open_items, history}` own the read surface;
  `Mind::admit_prepared` still owns admission and calls `Docs` as before.
- **Inputs:** `mind.envelopes()` (decoded through the leaf's `decode`),
  `mind.receipts()`, the query argument, the mind's instance. **Not
  inputs:** `stored_at`, a clock, the environment, a store handle, the
  index.
- **Outputs:** `PipelineDocumentView`s, `PipelineQueryPage`,
  `PipelineOpenItems`, `MindRefusal::Unavailable` for the two integrity
  faults and the semantic refusal.
- **Derived state:** status, the base of a resolution, the admission index,
  the order, the match count — all computed per call, none stored. No
  admission writes an `in_force`, `status` or `admitted_at` field; the
  receipt's `committed_at` is the only stored time the views read.
- **Forbidden writers:** `query.rs` may not decide in force, a sequence or
  a scope by any rule of its own (it calls `Docs`); may not read
  `stored_at`; may not construct a receipt, call the store, or write; may
  not parse a sequence out of a key; may not derive a key. `admission.rs`
  may not keep a private copy of any `Docs` method. Cut 10's daemon, Cut
  11's index and Cut 13's client may not compute status; the index may not
  index status (D5) though the view carries it.
- **Shared paths:** `Docs::from_image` and every derivation, called by
  admission over image ∪ batch and by the views over the image;
  `closing_resolution_unless` under both `in_force_unless` and
  `PipelineStatus`; `Mind::view` under `get` (Cut 10/13) and the index
  (Cut 11).
- **Deletion line:** the move out of `admission.rs` lands in the first
  commit, before `query.rs` exists; `cargo test -p huginn-mind --lib` is
  41 green between the two commits.

### Verification

**Commits.** (1) `docs.rs` move with D2's three edits, three cut-8 entries
re-anchored, 41 tests green, H1-H61 killed. (2) `query.rs`, fixtures, the
ten tests, `lib.rs`, README/AGENTS. (3) `eureka-cut9-mutations.psd1`, all
entries killed. Soul verifies per commit.

**Builds.** `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`; path-list
baseline before and after; **no `cargo clean` of any scope** (the fix batch's
deviation). `cargo check -p huginn-mind --lib --tests`; `cargo test -p
huginn-mind --lib` (41 + 10 = **51 tests**, 0 warnings); `cargo check
--workspace`; `cargo tree -p huginn-mind -e normal -d` unchanged. Host =
target = workstation; no platform code touched.

**Tests**, `crates/huginn-mind/src/query.rs`, each named for the rule it pins.

| Test | Pins |
|---|---|
| `status_is_the_derivation_admission_uses` | R-A. Q1 open → `InForce`; a ruling answers it → `Resolved { resolution: …question.Q1.n1, record.outcome: Answered }`; withdraw n1 → Q1 `InForce` again and n1's own view `Resolved { resolution: …question.Q1.n1.n1, record.outcome: Withdrawn { reason } }`; resolve n2 → Q1 `Resolved { …n2 }`. The ruling stays `InForce` after its resolution is withdrawn (Soul's observation). A campaign and a verdict are `InForce` with no special case. |
| `a_subjects_history_lists_every_resolution_with_its_status_and_receipt` | R-B, D7, V9-V10. Ten reopen cycles on Q1 (n1..n10 with withdrawals of n1..n9): `history(Subject(Q1))` is exactly n1..n10 in sequence order (n10 after n9, not after n1), n1..n9 each `Resolved` by its withdrawal with `Withdrawn { reason }`, n10 `InForce`; no withdrawal in the list; `history(Subject(<n1>))` is `[n1.n1]`; every record's `receipt_id` distinct (one per batch). |
| `a_repos_stewardship_history_on_a_mind_lists_every_assignment` | D7, V11. Hand off `REPO` away and back, and `OTHER_REPO` once: `history(Repo(REPO))` is `[n1 Resolved { Withdrawn { reason: <hand-off key> } }, n2 InForce]` with `assigned_on` the two `handed_on`s; `OTHER_REPO` absent; the other mind is not consulted. |
| `views_join_admission_facts_from_the_receipt_that_wrote_them` | D3, V3-V4. `receipt_id` equals `Committed.receipt_id`; `admitted_at` equals `committed_at` and the `now` passed; `provenance` equals the batch's; a derived write (the `Answered` resolution) carries the ruling's receipt; a document cited as a strong read by a later batch keeps its first receipt; a planted pipeline document with no receipt (`MemoryStore::plant`, then `open_with`) makes `view` of it and any `query` `Unavailable` naming it. |
| `views_read_admitted_at_from_the_receipt_not_stored_at` | R-C, V2. Seed and admit at two `now`s; copy every row into a second `MemoryStore` with `stored_at` overwritten by a constant, and again with the stamps reversed; `open_with` each; `query` and `history` outputs equal the original's. |
| `query_filters_each_select_by_one_field` | D5, V16-V23. One assertion per filter: `campaign` (a second campaign on the same repo; a resolution matches by its root; a stewardship matches under the instance root), `repo` (`campaign.repos` contains; `cut_spec.repo`; a question never), `cut` (cut-9 spec, report, verdict, finding and the spec's supersession match; cut-10's do not; a ruling never), `kinds`, `in_force` both ways, `faculty` (admitted as `Soul` vs `Hands`), `admitted_after`/`admitted_before` at the exact boundary (exclusive). |
| `query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count` | D5, V5-V7. 201 questions over four batches at rising `now`s, Q2's batch before Q1's; `limit: Some(500)` → 200 items, `matched: 201`, first item Q2's batch; `None` → 200; `Some(0)` → 1; `Some(5)` → 5. The seed batch (one `now`) orders by id: `campaign:self`, `target:r1`, `instance:self`, `stewardship:…`, not by type. |
| `open_items_are_derived_from_in_force_and_citation` | D6, V12-V15. Open: an unanswered question, a `Plausible` finding, a follow-up, spec `cut-9.r2` (r1 superseded, no report), report `cut-10.h1` (no verdict). Not open: the answered question, the `Fixed` finding, `r1`, a spec with a report, a report with a verdict, a second campaign's question. |
| `semantic_query_refuses_typed_until_wired` | D5, V8. `semantic: Some(..)` with every other filter set → `Err(Unavailable)`, never `Ok(empty)`; the same query without `semantic` → `Ok`. |
| `view_of_an_absent_id_is_none_and_of_a_present_one_is_its_document` | D4/D8. `view` of an absent ref → `Ok(None)`; of a present one → the decoded document equal to `Mind::get`'s and the key equal to `id.id`. |

**Negative greps** (`crates/huginn-mind/src`):

- `rg -n "stored_at"` empty.
- `rg -n "fn in_force|fn closing_resolution|fn latest_resolution|fn latest_stewardship|fn stewardships_of|fn later_in_force|fn resolutions_of|fn assignments_of"` matches in `docs.rs` only, once each.
- `rg -n "struct Docs|struct Held|struct Staged|fn later_than|fn kind_of_type"` in `docs.rs` only.
- `rg -n "Utc::now|SystemTime::now|std::env::var"` empty.
- `rg -n "pipeline_key|split\(':'\)" query.rs`: no `pipeline_key`; `split(':')` at most in the one root/local reader (the same shape `kind_of_id` uses).
- `rg -n "compare_and_swap|prepare_entry|HuginnCommitReceipt \{" query.rs` empty.
- `rg -n "reqwest|qdrant|ollama|IndexPort|EmbeddingPort|pending_index" crates/huginn-mind` empty (Cut 11's).
- `rg -n "rulings_in_force|fn stewardship\(|text_contains|outcome:" query.rs` empty of any function or field by those names.
- `git diff --stat 7b67730 -- Cargo.toml Cargo.lock crates/huginn-mind/Cargo.toml` empty (after the batch in flight lands, against its commit).

**Suites that must still kill every entry:** `eureka-cut8-mutations.psd1`
(H1-H61 with H41, H47, H60 re-anchored; `-Target` gains `docs.rs`) and the
new `eureka-cut9-mutations.psd1`, both through `tools/eureka-mutations.ps1`
with `-Repo F:\Projects\Huginn`, M0 green. Cut 9's run:

```
powershell -File F:\Projects\Epiphany\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
    -Entries tools/eureka-cut9-mutations.psd1 `
    -Target crates/huginn-mind/src/docs.rs,crates/huginn-mind/src/query.rs `
    -Test 'cargo test -p huginn-mind --lib'
```

#### Mutations, `tools/eureka-cut9-mutations.psd1`

Each rule has a **revert** (the old permissiveness: the rule absent) and,
where one exists, a **loosening** (the rule weakened, not absent). Anchors
are content Hands lands, matched exactly once; `Killed by` names the test
the entry runs; collateral kills are noted where they are the point.

| # | Rule | Revert | Loosening | Killed by |
|---|---|---|---|---|
| V1 | One owner for in force (D2) | `closing_resolution_unless` body → `None` (nothing ever closes) | H41's form: drop `&& self.in_force(PipelineKind::Resolution, key)` (a withdrawn resolution still closes) | `status_is_the_derivation_admission_uses`; collaterally `a_withdrawn_resolution_reopens_its_subject_and_stays_readable` and `a_document_is_written_once_and_superseded_by_resolution` in the full run — the same anchor as H41, listed twice on purpose: one mutant, two suites, one owner |
| V2 | `admitted_at` is the receipt's, not the store's (R-C) | `admitted_at: receipt.committed_at.clone()` → `mind.raw_envelope(&write.document_type, &write.document_key).map_or_else(\|\| receipt.committed_at.clone(), \|e\| e.stored_at.clone())` | none: one source field | `views_read_admitted_at_from_the_receipt_not_stored_at` |
| V3 | Facts from `writes` only (D3) | — | `receipt.writes.iter()` → `receipt.writes.iter().chain(&receipt.strong_reads)` (a later citation overwrites the first receipt) | `views_join_admission_facts_from_the_receipt_that_wrote_them` |
| V4 | An unreceipted row refuses, not hides (D3) | `Edits`: in `query`'s loop `let facts = …?;` → `let Ok(facts) = … else { continue };`, in `view` → `else { return Ok(None) };` | the two-receipts check dropped (`insert` without the `is_some()` refusal) | the same test |
| V5 | The cap (D5) | `.clamp(1, QUERY_LIMIT_MAX)` → `.max(1)` | `QUERY_LIMIT_MAX: usize = 200` → `201` | `query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count` |
| V6 | Stable order `(admitted_at, id)` | sort by `id` only | sort by `admitted_at` only (stable over image order, so ties fall to `(type, key)`) | the same test (Q2 before Q1; the seed batch's id order) |
| V7 | `matched` is the count before the cut | `matched` computed after `truncate` | — | the same test |
| V8 | `semantic` refuses typed (D5) | the `semantic` arm returns `Ok(PipelineQueryPage { items: vec![], matched: 0 })` | the check moved after the filters (still refuses; equivalent — recorded as such, not an entry) | `semantic_query_refuses_typed_until_wired` |
| V9 | History is in sequence order (D7) | the sort removed (image order: n1, n10, n2, …) | none: every other order the data offers (`admitted_at`, receipt order) coincides with sequence in a well-formed mind | `a_subjects_history_lists_every_resolution_with_its_status_and_receipt` |
| V10 | `Subject` history selects by subject | `resolutions_of` → `resolutions()` (every resolution) | subject compared by `id` only, `kind` ignored | the same test (withdrawals excluded; `history(<n1>)` exact) |
| V11 | `Repo` history selects `(mind, repo)` | `assignments_of` → `stewardships()` | `repo` compared, `instance` ignored | `a_repos_stewardship_history_on_a_mind_lists_every_assignment` |
| V12 | Open questions/findings/follow-ups are in force (D6) | the `InForce` condition dropped | — | `open_items_are_derived_from_in_force_and_citation` |
| V13 | Specs without report: in force only | in-force dropped (superseded r1 counted) | — | the same test |
| V14 | Reports without verdict | the citation check dropped (every report open) | citation compared by `cut` prefix instead of the exact id | the same test |
| V15 | Open items are one campaign's | the root filter dropped | — | the same test |
| V16 | `cut` filter is the label | `starts_with(&format!("cut-{label}."))` → `starts_with("cut-")` | the trailing `.` dropped (`cut-9` matches `cut-90`) | `query_filters_each_select_by_one_field` |
| V17 | Filters reach a resolution through its base | `base()` returns the resolution itself | base followed one step, not to the non-resolution | the same test (the spec's supersession matches `cut`) |
| V18 | `campaign` is the key root | the filter dropped | compared against `campaign` fields only (stewardship under the instance root, and resolutions, never match) | the same test |
| V19 | `kinds` | dropped | — | the same test |
| V20 | `in_force` | dropped | inverted | the same test |
| V21 | `faculty` (attribution filters, grants nothing) | dropped | — | the same test |
| V22 | `admitted_after`/`admitted_before` exclusive | dropped | `>` → `>=` | the same test (the boundary case) |
| V23 | `repo` matches `campaign.repos` and the five repo fields | dropped | `campaign.repos` omitted | the same test |

Stated limits: the recursion's termination (`key(R)` strictly longer than
`id`) has no runtime mutant, as in Cut 6d; the `JsonSchema` derives are
pinned by compilation of Cut 13, not here; V8's "moved check" is
equivalent and is recorded, not run.

**Operator checks before landing:** Q21 below has a recommended default
Hands builds under; nothing blocks.

### Subtraction estimate

Source outside tests: `admission.rs` −about 175 (the move, the doc
sentence); `docs.rs` +about 205 (the moved 170 plus D2's four helpers and
docs); `query.rs` +about 330 (types 70, index 45, reader and filters 150,
the four functions 65); `lib.rs` +4; fixtures +20. Net **+about 385**
outside tests. Tests +about 520 (ten tests). Entries +about 300 (V1-V23),
cut-8 re-anchors ±15. Total about +1,200 with tests and entries against the
old section's +600 (which had six tests and three mutants and a second
derivation).

Liability retired before it shipped: the second in-force the old section
implied; two functions (`rulings_in_force`, `stewardship`) and three query
fields whose meaning was either a preset or a second owner. Nothing in the
crate is removed net because the read side did not exist; the move is the
cut's subtraction and it is a real one: after it, `admission.rs` holds no
derivation.

Kinds, dependencies, lock, targets, formats, schemas, epoch: zero.

### Build budget

`huginn-mind` lib + tests only; the two stubs re-checked by `cargo check
--workspace` with no change. Debug, workstation host = target, no features,
no codegen. The leaf rlib and the 90 transitive packages are warm (the probe
rebuilt only the crate's test binary: 5.5 s). Expected footprint delta: +0
to +40 paths, all under `debug/` for the crate's own fingerprint and test
binary; baseline 10,579 paths this pass. Retention: the shared target dir is
the operator's; **nothing is cleaned**, in any scope.

### Operator questions

- **Q21. `text_contains` until Cut 11?** The landed field set had a
  substring filter over a document's text. **A. Drop it (recommended).**
  Cut 11 defines once which fields of each kind are text (its `IndexPoint`
  source); a substring filter here would be a second definition of "the
  text of a document", or a filter over serialised JSON, which is the
  symptom-shaped kind. Until Cut 11 lands, an agent reads views; `history`
  and `open_items` carry the precedent an agent rehydrates from (ruling 2).
  **B. Keep it**, built over a `text_of(&PipelineDocument)` projection in
  `huginn-mind` that Cut 11's index then reuses. Coherent, but it pulls
  Cut 11's projection forward into this cut for a filter nothing yet
  consumes. If B, the projection is one owner and `text_contains` is a
  filter over it; Cut 11's spec must then say it reuses `text_of`.
  **Taken as a default by Self, 2026-09-16: A.** Not a product fork; a
  second definition of a document's text is what Cut 11 would have to
  reuse or delete.

Decisions Self can overturn without an operator (not questions):
`instance` and `outcome` leave `PipelineQuery` (D5); an unreceipted row
refuses (D3); the view carries the whole provenance and the whole closing
record (D4); `open_items` and `history` are uncapped (D6, D7).

### Findings not assignable to this cut

- **`refusal.rs`'s `#[serde(remote)]` mirror `DocumentRefusal` is dead at
  the pin**: `PipelineRefusal` derives `Serialize`, `Deserialize`,
  `JsonSchema` at `d5a36c2a`. Cut 8's map already assigns its removal to
  Cut 10; still true, still not done.
- **`admission.rs`'s doc on `PipelineAdmissionBatch`** ("the leaf's
  `PipelineDocument` derives neither `Serialize` nor `JsonSchema` at the
  pinned rev, so this type cannot either") **is false at the pin** —
  `PipelineDocument` derives all three (leaf, `pipeline_kinds!`). The batch
  can derive them now; discrepancy 34's premise is gone. Cut 10's, with the
  wire.
- **D4, D6 and D7 in the map are superseded** by this spec: D4's
  seven-field view and five functions; D6's `RulingsInForce | Stewardship`
  operations (presets of `Query`); D7's `rulings_in_force` and `stewardship`
  tools (presets) and a missing `history` tool (R-B's affordance must reach
  the client or the obligation is unmet at the agent). Self's, at this
  cut's landing; Cut 10's and Cut 13's refreshes carry them.
- **Every read decodes the image and the receipts once per call**, as every
  admission already does. A decoded image held on `Mind` and refreshed in
  `refresh` would let the opener refuse a non-decoding store up front and
  remove `Docs::from_image`'s `Unavailable` path from both sides. Not this
  cut's; worth doing when a mind is large enough to measure.
- **No exact paging.** A query over 200 matches pages by `admitted_after`
  at second granularity; `matched` makes the truncation visible. A cursor
  `(admitted_at, id)` is the exact form if a proof campaign needs it.
- **The batch in flight** touches `derive` and the cut-8 entries; this cut
  moves `Docs` out from under `derive`'s call sites. Land order: that batch,
  then this cut's commit (1). If that batch adds a `Docs` method, it moves
  with the rest and Hands says so in the report.
- **`Faculty::SelfFaculty`** is the wire spelling Cut 13's tool schema will
  show. Cosmetic; noted for Cut 13's refresh, not changed here.

### Pinned HEADs

- Epiphany `codex/eureka-pipeline-state` at `94df3a8f`, tree clean; leaf at
  `d5a36c2a`, the rev Huginn pins.
- Huginn `eureka/memory-organ` at `7b67730` with an uncommitted Hands batch
  in `admission.rs` and `eureka-cut8-mutations.psd1`; this spec anchors by
  name because of it.
- CultLib `a0813c6` (the cargo checkout), read for `CultCacheEnvelope`.
- Probe artifacts: `scratchpad/cut9-wt` (detached at `7b67730`, one `cargo
  test -p huginn-mind --lib`, 41 passed, removed); `scratchpad/cut9-baseline.log`;
  `scratchpad/leaf-d5a36c2a.rs` (a `git show` copy, read only). No file in
  either repo was written; no `notes/` file was touched.

## Cut 10. `huginn-daemon`: the CultNet surface

Imagination, 2026-09-16, refreshed against the Body after Cut 9 landed and
Q13 was ruled A. Replaces the old Cut 10 section whole. The old section put
the wire types in a binary crate (overturned by Q13 A), copied Odin's
document-server harness (which cannot reply to a request), listed operations
Cut 9 turned into presets, and predated `Mind::{view, query, open_items,
history}`. Every anchor here is a function, type, constant or test **name**,
never a line: a Hands batch is landing in `crates/huginn-mind/src/query.rs`,
`docs.rs` and `tools/eureka-cut9-mutations.psd1` while this is written (the
main tree carried those three modified and a harness sidecar at spec time),
and this cut touches none of those three files.

### Pins

| Repo | Branch | HEAD | State |
|---|---|---|---|
| Huginn | `eureka/memory-organ` | `8fc39b1` | the last commit; the Hands batch above is uncommitted in the tree. This cut lands after it. |
| Epiphany | `codex/eureka-pipeline-state` | `fb18395f` | tree clean; `epiphany-pipeline/src/lib.rs` last moved at `d5a36c2a`, the rev `crates/huginn-mind/Cargo.toml` pins. No Epiphany change in this cut. |
| CultLib | `main` | `47aa7b6` | `git diff --stat a0813c6 47aa7b6 -- packages/cultcache-rs packages/cultnet-rs packages/cultmesh-rs` is **empty**; every source read below is against the checkout Huginn pins, `a0813c6`. |
| Odin | `main` | `5a015d9` | read for the house daemon shape (`crates/odin-daemon/src/main.rs`) and Eve document ids (`crates/odin-core/src/documents.rs`). |
| Idunn | `main` | `5b3f646` | read for the house request/response shape over a hub (`src/host_actuator.rs`, `HostActuatorHub`). |
| gamecult-ops | `main` | `36a466f` | `systemd/epiphany.service`, `runbooks/odin-yggdrasil.md`, `idunn/yggdrasil/bindings/odin.toml.in`, `inventory.md` (17872 still unallocated). |

Body facts this spec rests on, each by source read of the named file or by
the probe at the end:

- **`CultMeshRudpDocumentServer` cannot reply to a request.** Its sink is
  `CultMeshRudpRawDocumentSink::accept_raw_document(receipt) -> Result<()>`;
  a `DocumentPutRaw` is admitted or rejected (`ApplicationRejected`), and the
  only bytes that go back are the transport ACK. Reads are `SnapshotRequest`
  → `SnapshotResponseRaw` selected by `schema_ids`/`record_keys` only
  (`cultmesh-rs/src/rudp_document_server.rs`, `deliver_application_message`).
  So D6's "replies go back as `DocumentPutRaw`" has no mechanism, and a
  `PipelineQuery` cannot ride a snapshot filter. That server is Odin's shape
  because Odin is a catalog; the organ is a service.
- **`CultNetRudpServerHub` is the multi-session request/response substrate.**
  One socket, one `CultNetRudpSession` per remote peer, events
  `Connected | Frame | Pong | Disconnected` from `receive_event_once`, and
  `send_schema_message(&session, &CultNetMessage)` addressed to one session
  by `(remote_addr, session_generation)` (`cultnet-rs/src/rudp.rs`,
  `CultNetRudpServerHub`). Idunn's `HostActuatorHub::service` is the live
  house pattern (`Idunn/src/host_actuator.rs`). The hub drops any packet
  whose `connection_id` is not its own (`receive_packet_once`).
- **`cultnet.operation_request.v0` / `cultnet.operation_response.v0` are the
  typed request/response envelope**, with C# reference parity
  (`CultLib/src/GameCult.Networking/CultNetSchemaMessages.cs`,
  `CultNetOperationRequestMessage`/`ResponseMessage`; `CultNetOperationServer.cs`
  with `Accepted`/`Rejected` replies and `FailureSchemaId =
  "gamecult.cultnet.operation_failure.v1"` carrying `{code, message}`), published
  JSON at `CultLib/contracts/cultnet/cultnet.operation-{request,response}.schema.json`,
  and named by Eve as its plugin ABI carrier (`Eve/docs/plugin-architecture.md`,
  "Executable Plugin ABI"). Fields: `message_id, service_id, operation,
  payload_schema, payload_encoding, payload, source_runtime_id?,
  target_runtime_id?`; the response adds `status`, `diagnostics`. CultNet
  validates `payload_encoding == "messagepack-base64"` and every string
  non-empty (`contracts.rs`, `validate_message`). The Rust
  `CultNetOperationServer` (`operation_service.rs`) is a single-peer wrapper
  over one `CultNetRudpSocketTransportConnection` and nothing in Rust uses
  it; the C# one is multi-handler. Not used here.
- **Schema publication over CultNet is `SchemaCatalogRequest` →
  `CultNetSchemaRegistry::create_catalog_response`**, registrations carry
  `schema_json` (required, hashed canonically), `kind:
  WireMessage | DocumentPayload | SharedContract` (`schema_discovery.rs`).
  This is the one place JSON crosses the wire, and it is schema publication.
- **A client is `CultMesh::create_rudp_client_for_endpoint(runtime_id,
  connection_id, "rudp://host:port", CultMeshRudpSocketOptions)`** then
  `connect`, `send_schema_message`, `receive_schema_message_once`,
  `poll_resends` (`cultnet-rs/src/cultmesh.rs`, `rudp.rs`). `CultMesh::connect_rudp_client_for_endpoint`
  does the connect wait. Cut 13 needs nothing else from the transport.
- **`huginn-mind` at `8fc39b1`:** `Mind::open(state_root, &Slug) ->
  Result<Mind<OwnedRedbMessagePackBackingStore>, MindRefusal>` with
  `MindAlreadyOwned` for a held lock (`mind.rs`); `Mind::admit(batch, now)`,
  `admit_prepared` and step A1 `if instance != self.instance()` inline in
  `admit_steps` (`admission.rs`); `Mind::{view, query, open_items, history}`
  (`query.rs`); `PipelineAdmissionBatch` derives only `Clone, Debug,
  PartialEq, Eq` under a doc comment claiming the leaf's `PipelineDocument`
  cannot serialise, which is false at `d5a36c2a` (Cut 9's finding);
  `refusal.rs` still carries the `#[serde(remote = "PipelineRefusal")]`
  mirror `DocumentRefusal` and the two `with` attributes on
  `MindRefusal::Document`, dead since the leaf derives all three traits;
  `store::test_stores::{MemoryStore, RefusingStore}` are `#[cfg(test)]
  pub(crate)` and unreachable from another crate; `lib.rs` re-exports the
  read and admission types but not `epiphany_pipeline`; the crate reads no
  clock and no environment. `Mind<OwnedRedbMessagePackBackingStore>` is
  `Send` (probe).
- **Odin's daemon** (`odin-daemon/src/main.rs`): `parse_options` over exact
  `--state-root --idunn-projection --idunn-anchor`, `GAMECULT_IDUNN_CANDIDATE_BIND`
  asserted loopback, `signal_hook::flag::register` for SIGTERM/SIGINT with
  the PID-namespace comment, a bootstrap loop `while !try_activate()` gated
  on Idunn's process write lease and activation records, a self-presence
  heartbeat published through `publish_cultnet_message_to_rudp_catalog`, and
  `std::os::fd::{FromRawFd, RawFd}` for systemd-passed signer descriptors —
  **Linux-only**. `signal-hook` 0.3 registers SIGTERM/SIGINT on Windows
  (probe).
- **`epiphany.service`** spells its endpoints `--qdrant-url`,
  `--ollama-base-url`, `--ollama-model` and its Idunn health as
  `--idunn-rudp-health 10.77.0.1:17870` (gamecult-ops `systemd/epiphany.service`);
  those are Cut 11's and Cut 14's flags and are not added here.
- **Eve's operator surface is a provider advertisement plus surface documents
  through Odin**: `gamecult.eve.provider_advertisement.v1` (required:
  `providerId, serviceId, verseId, title, kind, freshness, schemas, witnesses,
  surfaces, commands`; `Eve/schemas/…provider_advertisement.v1.schema.json`)
  and `gamecult.eve.surface_state.v1` (`odin-core/src/documents.rs`,
  `EveSurfaceStateRecord { provider_id, title, version, updated_at, surface:
  Value }`). Nothing in Rust builds one outside Sleipnir; both carry JSON
  `Value` bodies. No Eve DSL type exists in `cultnet-rs` or `cultmesh-rs`.
- **Probe** (detached worktree at `8fc39b1` under the scratchpad,
  `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex`, PowerShell, `cargo
  test -p huginn-daemon --lib` once, worktree removed, main tree untouched
  by it): with the dependency set under Build budget, a `CultNetRudpServerHub`
  on `127.0.0.1:0` accepted two `CultMesh::create_rudp_client_for_endpoint`
  clients, received one `OperationRequest` from each carrying a base64
  `rmp_serde::to_vec_named(PipelineQuery)`, answered each on its own session
  with an `OperationResponse` carrying a `PipelineQueryPage`, and each client
  decoded the reply correlated to its own `message_id`. `schemars::schema_for!
  (PipelineQueryPage)` is 42,899 bytes of JSON. Build 30.4 s warm; lock 93 →
  139 entries (41 new package names, listed under Build budget); target dir
  10,619 → 11,030 paths, 8.74 → 9.08 GiB. Probe files kept at
  `scratchpad/cut10-probe-lib.rs` and `cut10-probe-Cargo.toml`.

### What the cut does

The organ's body: one process that opens one instance's mind, serves
admission and the read side over CultNet RUDP to whoever can reach its
socket (the LAN and, through Cut 14's route, WireGuard), refuses to start
when it cannot open the mind, and publishes its own wire schemas on request.

- **The wire, in `huginn-mind::wire`** (Q13 A): `HuginnMindRequest` with one
  variant per `Mind` method plus `Whoami`; `HuginnMindResponse` with the
  method's return type per variant plus `Refused(MindRefusal)` for the read
  side; `MindStatus`, the answer to `Whoami` and the typed state a dashboard
  would project. All derive `Serialize, Deserialize, JsonSchema`; the payloads
  are Cut 8's and Cut 9's types unchanged.
- **The envelope, in `huginn-daemon::envelope`:** a `HuginnMindRequest` rides
  `cultnet.operation_request.v0` (`service_id = "huginn.mind"`, `operation` =
  the variant's name, `payload_schema = "huginn.mind_request.v1"`, `payload` =
  base64 of named MessagePack); the response rides
  `cultnet.operation_response.v0` the same way with `status` derived from the
  response. An envelope the daemon cannot decode is answered with
  `gamecult.cultnet.operation_failure.v1 { code, message }`, status
  `rejected`, and touches no mind.
- **The daemon, in `huginn-daemon::daemon`:** `Daemon<S: MindStore> { mind:
  Mind<S>, index: I }` and `handle(&mut self, request, now) ->
  HuginnMindResponse`, a pure dispatch with no socket, no clock and no rule:
  `Admit` → `mind.admit`; every other instance-bearing request →
  `mind.require_instance(&declared)?` then the method; after a `Committed`
  outcome, `index.committed(&mind, &writes)`, whose error is logged and never
  changes the outcome.
- **The loop, in `huginn-daemon::serve`:** `run(daemon, hub, registry,
  stopping)` over `CultNetRudpServerHub`: frames on the `schema` channel are
  decoded, answered on their session, and every other message is answered
  with `CultNetMessage::Error`; `SchemaCatalogRequest` is answered from the
  registry holding the two wire schemas. Hostile datagrams and a departed
  session are logged and served past, never fatal.
- **`main`:** `--state-root`, `--instance`, `--bind`; open the mind first,
  bind second, serve until SIGTERM/SIGINT. A mind that will not open ends the
  process with the refusal on stderr and no socket ever bound.
- **One instance check, one owner:** step A1 becomes `Mind::require_instance`,
  called by `admit_steps` as before and by the daemon for reads. The daemon
  compares nothing itself.

Not in this cut, by decision: an Eve provider advertisement or surface
(Q23), the Idunn activation handshake and presence heartbeat (Cut 14),
Qdrant and Ollama (Cut 11), a hand-off or import operation (Cut 12 decides
whether it needs one; Findings), and any `.cc` state of the daemon's own.

### What changed against the old Cut 10 section

1. **The wire types move out of the binary** into `huginn-mind::wire` (Q13 A)
   and the daemon gains a lib target so Cut 13 can run it in-process.
2. **`CultNetRudpServerHub` + operation envelopes replace
   `CultMeshRudpDocumentServer` + `DocumentPutRaw`/snapshot**, because the
   document server has no reply path and a snapshot has no query. `SinkHandle`
   and `SnapshotHandle` are not built.
3. **Operations are `Mind`'s methods:** `whoami, admit, view, query,
   open_items, history`. D6's `Get` is `view`; `RulingsInForce` and
   `Stewardship` are `query` presets (Cut 9); `HandOff` and `Import` are Cut
   12's in-process operations and have no wire form here.
4. **No Odin bootstrap wait, heartbeat, `fs2`, `cultmesh-rs` or
   `pending_index`.** The Idunn handshake is Linux-only and Cut 14's; the
   index retry slot is Cut 11's; the daemon holds no lease file of its own
   (the owned store's lock is the single-writer mechanism, Cut 8).
5. **The smoke spawns no process.** `serve::run` is driven in-process over a
   loopback hub; Cut 14's runbook verification exercises the real process.
6. **Two deletions in `huginn-mind`** the map assigned here: the
   `DocumentRefusal` mirror, and the false doc on `PipelineAdmissionBatch`
   (which now derives the three traits).
7. **Tests 5 → 10, mutations 2 → 16** in `tools/eureka-cut10-mutations.psd1`.

### Decisions, with the reasons

### D1. Request/response over CultNet RUDP through the hub; not a CultMesh publication

Admission is a command with a typed outcome and a query is a filter with a
page; both are request/response. The three CultNet shapes that could carry
them, by source read:

| Shape | Reply per request | Multi-session | Query filters | Verdict |
|---|---|---|---|---|
| `CultMeshRudpDocumentServer` (D6's) | none from the sink; ACK or rejection | yes | `schema_ids`, `record_keys` only | cannot answer `admit` or `query` |
| `CultNetOperationServer` (Rust) | yes, typed envelope | no: one peer, a new Connect resets the session | any | one workstation session at a time |
| `CultNetRudpServerHub` + `OperationRequest`/`Response` | yes, per session | yes | any | **this** |

A CultMesh publication path was considered for admission (put a request
document, snapshot the response document later): two round trips and a
response store the daemon would have to own and expire, which is a cache
pretending to be truth. Rejected. Reads through a snapshot source (D6's
"plain CultNet snapshot client can read a mind") are also not built: a
snapshot cannot express `PipelineQuery`, and the raw image is the store's
shape, not the view's (status and admission facts are derived, Cut 9).

What this buys against doctrine: the organ speaks CultNet (RUDP over UDP,
ruling 19) with CultLib's own typed envelope that the C# reference and Eve's
ABI already speak; JSON crosses the wire only as schema publication (D7).
What it does not buy: a CultMesh state document a dashboard can pull without
speaking the operation envelope. That is Q23.

### D2. The wire types, exactly

`crates/huginn-mind/src/wire.rs`, `pub mod wire`, re-exported from `lib.rs`:

```rust
pub const MIND_SERVICE_ID: &str = "huginn.mind";
pub const MIND_REQUEST_SCHEMA: &str = "huginn.mind_request.v1";
pub const MIND_RESPONSE_SCHEMA: &str = "huginn.mind_response.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HuginnMindRequest {
    Whoami,
    Admit(PipelineAdmissionBatch),                       // carries `instance`
    View { instance: Slug, id: PipelineRef },
    Query { instance: Slug, query: PipelineQuery },
    OpenItems { instance: Slug, campaign: Slug },
    History { instance: Slug, scope: HistoryScope },
}
impl HuginnMindRequest {
    /// The envelope's `operation`: `whoami | admit | view | query | open_items | history`.
    pub fn operation(&self) -> &'static str;
    /// The instance the request declares; `Whoami` declares none.
    pub fn instance(&self) -> Option<&Slug>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HuginnMindResponse {
    Whoami(MindStatus),
    Admit(PipelineAdmissionOutcome),
    View(Option<PipelineDocumentView>),
    Query(PipelineQueryPage),
    OpenItems(PipelineOpenItems),
    History(Vec<PipelineDocumentView>),
    Refused(MindRefusal),                                // the read side's refusal
}
impl HuginnMindResponse {
    /// The envelope's `status`, the C# reference's vocabulary: `rejected`
    /// for `Refused(_)` and `Admit(Refused(_))`, `accepted` otherwise
    /// (`Conflict` and `AlreadyAdmitted` are answers, not refusals).
    pub fn status(&self) -> &'static str;
}

/// What a mind says about itself: the typed state a dashboard projects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MindStatus {
    pub instance: Slug,
    pub schema_epoch: String,     // PIPELINE_SCHEMA_EPOCH
    pub documents: u32,           // pipeline documents in the image
    pub receipts: u32,
}
impl<S: MindStore> Mind<S> { pub fn status(&self) -> MindStatus; }   // in mind.rs
```

`Admit` carries the batch whole because the batch already carries
`instance` and `provenance`; a second `instance` beside it would be two
declarations. The read requests carry `instance` because ruling 14 says a
write carrying another instance's identity is refused *whatever the
transport*, and a read of the wrong mind is the same collision on the read
side (D3). `Whoami` carries none: it is how a client learns which mind it
reached.

`PipelineAdmissionBatch` gains `Serialize, Deserialize, JsonSchema`; its
doc comment loses the false sentence. `MindRefusal::Document` loses its two
`with` attributes; `DocumentRefusal` is deleted.

Serialisation: `rmp_serde::to_vec_named` and `rmp_serde::from_slice`, so
field names on the wire are the names the JSON schemas publish (the C#
reference keys by name). `PipelineDocument` is adjacently tagged
`{kind, value}` and rides as a map; the leaf's `[value]` array form is the
*store* shape, not the wire's, and the daemon never sees an envelope.

Schemas: `schemas/cultnet/huginn.mind_request.v1.schema.json` and
`huginn.mind_response.v1.schema.json` in Huginn, derived by
`schemars::schema_for!` and pinned byte-for-byte by a test (Epiphany's
`pipeline_published_schemas_match_derivation` pattern). They are Huginn's
contracts, published from Huginn (D6's schema-publication paragraph stands).
Expect about 100 KB each; the pipeline value types are inlined by
`schemars` under `$defs` and Epiphany's thirteen files are not copied.

### D3. One instance check, owned by the mind

`admit_steps`'s A1 becomes:

```rust
impl<S: MindStore> Mind<S> {
    /// Ruling 14 across every transport: the declared instance is this
    /// mind's, else `ForeignInstance { declared, mind }`. A1 for admission;
    /// the daemon asks it for every read that names an instance.
    pub fn require_instance(&self, declared: &Slug) -> Result<(), MindRefusal>;
}
```

`admit_steps` calls it where the inline comparison was; behaviour identical,
H1 re-anchors to the new body (Per-file changes). The daemon's `handle`
calls it once per instance-bearing read before dispatch and never compares
an instance itself. Rejected: comparing in the daemon (a second validator of
A1, the thing the brief forbids) and dropping the instance from reads (a
client configured for `thought-cage` would silently read `yggdrasil`).

### D4. The daemon is a pure dispatch over a hub, without a transport trait

```rust
// daemon.rs
pub trait IndexSink<S: MindStore> {
    /// Called after `Committed`, with the landed refs; Cut 11 reads
    /// `mind.view(ref)` per ref and indexes. An error never reaches the caller.
    fn committed(&mut self, mind: &Mind<S>, writes: &[PipelineRef]) -> anyhow::Result<()>;
}
pub struct NoIndex;                                   // this cut's implementation
pub struct Daemon<S: MindStore, I: IndexSink<S>> { mind: Mind<S>, index: I }
impl Daemon<OwnedRedbMessagePackBackingStore, NoIndex> {
    pub fn open(state_root: &Path, instance: &Slug) -> Result<Self, MindRefusal>;   // Mind::open
}
impl<S: MindStore, I: IndexSink<S>> Daemon<S, I> {
    pub fn with_index<J: IndexSink<S>>(self, index: J) -> Daemon<S, J>;
    pub fn mind(&self) -> &Mind<S>;
    pub fn runtime_id(&self) -> String;                // "huginn-<instance>"
    pub fn handle(&mut self, request: HuginnMindRequest, now: DateTime<Utc>) -> HuginnMindResponse;
}
```

`handle`, exactly: `Whoami` → `Whoami(mind.status())`. `Admit(batch)` →
`mind.admit(batch, now)`; if `Committed { writes, .. }`, `index.committed(mind,
&writes)` and on `Err` one `eprintln!` naming the refs; the outcome is
returned as computed before the sink ran. Every other variant →
`mind.require_instance(declared)` then the method, `Err(refusal)` →
`Refused(refusal)`. No `Utc::now()` here: `now` is the caller's, so a test
pins it.

No `Transport` trait. The loop below is under forty lines; a trait with one
live and one test implementation over it is the one-implementation
abstraction ruling 20 killed in Cut 5. Typed hand-off between organs is
proven without a socket at `handle` (Tests 1-4) and at the envelope codec
(Tests 5-6); the loop is proven once over a loopback hub (Test 9). The
hub is CultLib's port, not ours.

The daemon's tests open real minds through `Mind::open` in a `tempfile`
directory (redb, milliseconds) rather than promoting `MemoryStore` out of
`cfg(test)`: no feature flag, no `pub` test surface, and the lock is
exercised. If Cut 13's smoke wants a memory mind, one
`#[cfg(any(test, feature = "test-stores"))]` line is the promotion; noted
under Findings, not done.

### D5. The envelope codec, exactly

`envelope.rs`, pure functions:

```rust
pub const FAILURE_SCHEMA: &str = "gamecult.cultnet.operation_failure.v1";   // the C# reference's FailureSchemaId
#[derive(Serialize, Deserialize, JsonSchema, ..)]
pub struct OperationFailure { pub code: String, pub message: String }       // keys as the C# type: `code`, `message`

pub fn encode_request(message_id: &str, request: &HuginnMindRequest, source_runtime_id: Option<String>) -> Result<CultNetMessage>;
pub fn decode_request(message: &CultNetMessage) -> Result<(String /*message_id*/, HuginnMindRequest), OperationFailure>;
pub fn encode_response(message_id: &str, operation: &str, response: &HuginnMindResponse, source_runtime_id: &str) -> Result<CultNetMessage>;
pub fn encode_failure(message_id: &str, operation: &str, failure: &OperationFailure, source_runtime_id: &str) -> CultNetMessage;
pub fn decode_response(message: &CultNetMessage) -> Result<(String, Result<HuginnMindResponse, OperationFailure>)>;   // Cut 13's side, tested here for symmetry
```

`decode_request` refuses, each with its own `code`, in this order:
`not-an-operation-request` (any other `CultNetMessage`), `wrong-service`
(`service_id != MIND_SERVICE_ID`), `wrong-payload-schema`, `payload-not-base64`,
`payload-not-a-request` (`rmp_serde::from_slice` fails), `operation-mismatch`
(`operation != request.operation()`: the envelope's string is checked against
the enum, never trusted). `payload_encoding` is CultNet's own check
(`validate_message`) and is not re-checked. `source_runtime_id` and
`target_runtime_id` on a request are read by nothing (ruling 18: declared,
not verified, and the declaration is in the payload). `message_id` is the
client's correlation key and is echoed; the daemon keeps no table of them —
the receipt is the organ's idempotency (A9), and a replayed `admit` answers
`AlreadyAdmitted` from the mind, not from a message cache.

`encode_response`: `status = response.status()`, `payload_schema =
MIND_RESPONSE_SCHEMA`, `diagnostics = []`, `source_runtime_id =
Some(runtime_id)`. `encode_failure`: `status = "rejected"`, `payload_schema =
FAILURE_SCHEMA`, `diagnostics = [failure.code.clone()]`.

Base64 is `base64::engine::general_purpose::STANDARD` (the crate cultnet-rs
already depends on). Connection id is `cultnet_rs::CULTNET_OPERATION_CONNECTION_ID`
(`0x4355_4c54`), CultLib's constant for operation services; the hub drops
every other connection id, and Cut 13's client must use the same.

### D6. The loop, exactly

`serve.rs`:

```rust
pub struct ServeOptions { pub session_timeout: Duration /* 30 s */, pub idle_sleep: Duration /* 2 ms */ }
pub fn bind(addr: SocketAddr, runtime_id: &str) -> Result<CultNetRudpServerHub>;   // non-blocking socket; max_fragment_bytes 1200, max_pending_reliable_packets 1024, max_peers 256
pub fn schema_registry() -> Result<CultNetSchemaRegistry>;                          // the two wire schemas, kind WireMessage, wire contract CultNetSchemaV0
pub fn answer<S, I>(daemon: &mut Daemon<S, I>, registry: &CultNetSchemaRegistry, message: CultNetMessage, now: DateTime<Utc>) -> CultNetMessage;
pub fn run<S, I>(daemon: &mut Daemon<S, I>, hub: &mut CultNetRudpServerHub, registry: &CultNetSchemaRegistry, stopping: &AtomicBool, options: &ServeOptions) -> Result<()>;
```

`answer`: `OperationRequest` → `decode_request` → `handle(request,
now)` → `encode_response`, or `encode_failure` on a decode failure;
`SchemaCatalogRequest` → `registry.create_catalog_response`; anything else
→ `CultNetMessage::Error { error: "huginn.mind answers cultnet.operation_request.v0 and cultnet.schema_catalog_request.v0" }`.

`run`: until `stopping`, `hub.remove_timed_out_sessions`, `hub.poll_resends`,
then drain `receive_event_once`: a `Frame` on channel `schema` is decoded
(`decode_cultnet_message_from_slice`, `CultNetSchemaV0`) and its `answer`
sent with `hub.send_schema_message(&session, ..)`; `Connected`, `Pong`,
`Disconnected` are ignored; a frame on another channel is ignored. `Err`
from `receive_event_once` (a hostile datagram fails `decode_rudp_packet`,
or the peer limit) and from `send_schema_message` (a session that left) are
printed and served past. `Utc::now()` is read once per frame here, the
crate's only clock read. Idle loops sleep `idle_sleep`, Odin's shape.

Stated limits: a socket error and a hostile datagram are indistinguishable
at `receive_event_once` and both are logged, so a dead socket spins with
logging rather than exiting (Odin has the same shape; Cut 14's health is
the observer). The reliable window is `1024 × 1200` bytes per session, about
1.2 MB in flight; a `Query` page of 200 maximal views may exceed it and
`send_many` refuses with an error the loop logs, so the client times out
instead of receiving a typed answer. Soul probes the number; if it is real,
the fix is `PipelineQuery.limit` guidance in Cut 13 or a larger window, not
a second page format.

### D7. `main`, exactly

`main.rs`: `parse_options` over exactly `--state-root <abs path>`,
`--instance <slug>`, `--bind <ip:port>`, each required, each once, Odin's
parser shape; then `Daemon::open` (**before** any socket), on `Err(refusal)`
print `refusal` (its `Display`) to stderr and exit 1; then `bind`,
`schema_registry`, `signal_hook::flag::register` for SIGTERM and SIGINT with
Odin's PID-namespace comment (Cut 14 runs this under Idunn), `run`. Nothing
reads the environment: `GAMECULT_IDUNN_CANDIDATE_BIND` and the rest are Cut
14's, which decides how Idunn supplies `--bind`. The mind is dropped at
process exit and the store's lock with it.

The `--instance` slug is parsed as `Slug` (`epiphany_pipeline`'s newtype,
`From<&str>`); the daemon reaches the leaf only through
`huginn_mind::epiphany_pipeline`, a `pub use epiphany_pipeline;` added to
`huginn-mind`'s `lib.rs` so one crate pins one rev and the daemon and Cut
13's client never declare the git dependency twice.

### D8. What this cut owns of the operator surface, and what it does not

Owned here: `MindStatus` (the typed state a dashboard projects), the two
wire schemas on the catalog, and the fact that any CultNet client can call
`whoami`. Not owned here: an Eve provider advertisement or a
`surface_state` composition through Odin. Reasons: the advertisement shape
is a JSON `Value` body with `verseId`, `freshness`, `witnesses` fields that
only mean something once the daemon is a Verse citizen on Yggdrasil beside
Odin (Cut 14), nothing in Rust builds one outside Sleipnir, and the presence
heartbeat it would ride on is Cut 14's. Building it here would be a
dashboard-shaped surface with no dashboard, which `F:\Projects\CLAUDE.md`
names as the thing not to invent. Q23 asks where it lands.

### Deletes first

| Path (by name) | Lines | What |
|---|---:|---|
| `crates/huginn-mind/src/refusal.rs`: `enum DocumentRefusal` with its `#[serde(remote = "PipelineRefusal")]`, `#[allow(dead_code)]` and doc comment; the `#[serde(with = "DocumentRefusal")]` and `#[schemars(with = "DocumentRefusal")]` attributes on `MindRefusal::Document` | about 20 | Dead since the leaf derives all three traits at `d5a36c2a`. `MindRefusal::Document(PipelineRefusal)` derives directly. |
| `crates/huginn-mind/src/admission.rs`: the doc sentence on `PipelineAdmissionBatch` beginning `The leaf's `PipelineDocument` derives neither` | 3 | False at the pin. Replaced by "Rides the wire whole (`wire::HuginnMindRequest::Admit`)." |
| `crates/huginn-mind/src/admission.rs`: the inline A1 comparison in `admit_steps` | 3 | Moves into `Mind::require_instance` (D3). |
| `crates/huginn-daemon/src/main.rs`: `fn main() {}` | 1 | The stub. |
| `README.md`, `AGENTS.md`: every sentence saying `huginn-daemon` is a stub, "nothing here publishes yet", "nothing here publishes or connects yet" | about 8 | Describe the live system. |
| The old Cut 10 section's `src/wire.rs` in the binary, `SinkHandle`/`SnapshotHandle`, `cultmesh-rs`, `fs2`, `signal-hook` bootstrap wait and heartbeat, `pending_index` retry slot, `RulingsInForce`/`Stewardship`/`HandOff`/`Import` operations, the snapshot read path, and the spawn-the-binary smoke | — | Not built (What changed, 1-5). |
| The map's D6 and D7 | — | Superseded by D1-D7 here for the transport and by Cut 13's refresh for the tools; the port (`rudp://10.77.0.1:17872`, private range `27880-27887`) and the schema-publication paragraph stand. D8 stands unchanged. Self's edit at landing. |

Nothing in the leaf, `Cargo.toml` workspace, `Cargo.lock` beyond the
daemon's additions, `query.rs`, `docs.rs`, or the cut-9 entries.

### Keeps

Every admission rule and refusal variant; `Mind::get`, `envelopes`,
`envelope`, `receipts`; every test name in `huginn-mind`; H1-H67 with H1
re-anchored; V1-V23 untouched; the leaf pin `d5a36c2a`; `MemoryStore` and
`RefusingStore` `cfg(test)`; D8's trust boundary (no signer, no anchor, no
credential path: ruling 18).

### Adds

| Add | Owner | Live consumer | Protected invariant | Why an existing owner cannot serve |
|---|---|---|---|---|
| `huginn-mind::wire`: `HuginnMindRequest`, `HuginnMindResponse`, `MindStatus`, the three constants, `operation()`, `instance()`, `status()` | `huginn-mind` | `huginn-daemon` (`envelope`, `daemon`), Cut 13's client (Q13 A) | One vocabulary of operations, equal to `Mind`'s methods; refusals are data in the response, never a transport error | Q13 A: the client imports types from `huginn-mind`; a binary crate cannot be imported. |
| `Mind::require_instance`, `Mind::status` (`mind.rs`) | `huginn-mind` | `admit_steps`; `Daemon::handle` | Ruling 14 has one check and it is the mind's; `whoami` is derived from the mind, not from configuration | A1 was inline; the daemon must not restate it. |
| `pub use epiphany_pipeline;` (`lib.rs`) | `huginn-mind` | the daemon's `Slug`, `PipelineRef`; Cut 13 | One git rev of the leaf in the workspace | Two `[dependencies]` entries on one git URL are two places to drift. |
| `schemas/cultnet/huginn.mind_{request,response}.v1.schema.json` + test | Huginn | the catalog response; Cut 13's schema test | The published schema equals the derivation, byte for byte | Huginn's contracts are published from Huginn (D6). |
| `huginn-daemon` `[lib]` `huginn_daemon`: `daemon.rs` (`IndexSink`, `NoIndex`, `Daemon`, `handle`), `envelope.rs` (D5), `serve.rs` (D6) | `huginn-daemon` | `main.rs`; Cut 13's smoke in-process; Cut 11 (`IndexSink`); Cut 14 (`run` under Idunn) | The daemon owns the socket, the process and the envelope, and no rule; the index never decides an outcome; an undecodable envelope touches no mind | Nothing in Huginn serves a socket; Odin's harness is a catalog, not a service. |
| `main.rs` (D7) | `huginn-daemon` | the operator, Cut 14's unit | Refuse loudly before listening (ruling 15) | — |
| `tools/eureka-cut10-mutations.psd1`, D1-D16 | the crate's suite | Epiphany's harness with `-Repo` | every ruling the daemon implements has a revert and a loosening | — |
| Dependencies of `huginn-daemon`: `anyhow`, `base64 = "0.22"`, `chrono = "0.4.44"`, `cultnet-rs` (git `a0813c6…`), `huginn-mind` (path), `rmp-serde = "1"`, `schemars = "1"`, `serde`, `serde_json = "1"`, `signal-hook = "0.3"`; dev `tempfile = "3"` | `huginn-daemon` | as named | — | `cultmesh-rs`, `fs2`, `cultcache-rs` are not needed: the hub and the client live in `cultnet-rs`, the daemon names no store type and holds no lease file. |

No new document type, kind, format, epoch, `.cc` file, HTTP, JSON on the
wire outside the schema catalog, Qdrant, Ollama, signer or anchor.

### Per-file changes, by name

**`crates/huginn-mind/src/refusal.rs`**: the deletes above; module doc's
"the wire (Cut 10) serialises it" stays true.

**`crates/huginn-mind/src/admission.rs`**: `PipelineAdmissionBatch` derives
`Serialize, Deserialize, JsonSchema`; doc replaced. In `admit_steps`, the A1
block becomes `self.require_instance(instance)?;` under the same `// A1`
comment.

**`crates/huginn-mind/src/mind.rs`**: `require_instance` and `status` on
`impl<S: MindStore> Mind<S>`; `status` counts `image` entries whose type is
a `PipelineKind` type id (`is_known_type` minus the two organ types) and
receipts by `HuginnCommitReceipt::TYPE`; `MindStatus` lives in `wire.rs`
and is imported.

**`crates/huginn-mind/src/wire.rs`** (new): D2, with a module doc: "The
organ's request and response vocabulary: one operation per `Mind` method
and `whoami`. Payloads are the admission and read types unchanged. The
daemon carries these in CultNet's operation envelope; the client constructs
them. Nothing here validates a document, derives a status or names a
transport."

**`crates/huginn-mind/src/lib.rs`**: `pub mod wire;`, `pub use wire::{...}`,
`pub use epiphany_pipeline;`; module doc gains one sentence: "The wire
vocabulary is here too, so the daemon and the client share one set of
types."

**`crates/huginn-mind/src/fixtures.rs`**: unchanged (the batch in flight may
touch it; this cut needs nothing from it).

**`crates/huginn-daemon/Cargo.toml`**: `[lib] name = "huginn_daemon" path =
"src/lib.rs"`, `[[bin]] name = "huginn-daemon" path = "src/main.rs"`, the
dependencies above, a header comment naming the crate's charter (socket,
process, envelope; no rule).

**`crates/huginn-daemon/src/lib.rs`**: `pub mod daemon; pub mod envelope;
pub mod serve;` and re-exports.

**`crates/huginn-daemon/src/{daemon,envelope,serve,main}.rs`**: D4-D7.
Tests beside their module.

**`schemas/cultnet/`** (new directory in Huginn): the two files; a
`README.md` of five lines saying whose they are and how they are checked.

**`tools/eureka-cut8-mutations.psd1`**: H1's `File` becomes
`crates/huginn-mind/src/mind.rs` and its `Old` the comparison inside
`require_instance` as Hands lands it (the mutant still `if false`); the
header's `-Target` already lists `mind.rs`.

**`tools/eureka-cut10-mutations.psd1`** (new): D1-D16 below.

**`README.md`, `AGENTS.md`**: `huginn-daemon` is live (the CultNet surface:
admission and reads over RUDP, schema catalog, no index yet); `eureka-state`
is the remaining stub; the `cargo` block gains `cargo run -p huginn-daemon --
--state-root <abs> --instance <slug> --bind 127.0.0.1:17872`.

**`Cargo.lock`**: regenerated; +41 package names (Build budget).

### Authority map

- **Owner:** `huginn-daemon` owns the socket, the sessions, the process
  lifetime and the envelope; `huginn-mind` owns every rule, the instance
  check, the receipt, the status and the wire vocabulary. Inside the daemon:
  `Daemon::open` owns "may this process serve this mind" (by delegating to
  `Mind::open`); `Daemon::handle` owns "which `Mind` method answers this
  request"; `envelope` owns "is this a request at all"; `serve::run` owns
  "which session gets which reply".
- **Inputs:** `--state-root`, `--instance`, `--bind`; datagrams on one socket;
  the wall clock, once per frame, in `serve`.
- **Outputs:** one `OperationResponse` (or failure, or `Error`) per frame on
  the frame's session; the schema catalog; the process exit code; stderr.
- **Derived state:** the hub's session table (CultLib's, expired by timeout);
  `MindStatus` per `Whoami` call; the schema registry built at start. The
  daemon persists nothing of its own.
- **Forbidden writers:** the daemon may not call `compare_and_swap_batch`,
  `admit_prepared`, `prepare_entry`, or construct a `HuginnCommitReceipt`; may
  not validate a document, derive a status or compare an instance (it calls
  `require_instance`); may not read `stored_at`; may not open a second mind
  or reopen its mind; may not spool, cache or retry a request; may not read
  `source_runtime_id`, `connect_payload` or `remote_addr` for any decision;
  may not sign, verify or enrol an identity; may not read the environment.
  `IndexSink` may not change an outcome. `huginn-mind` may not name a socket,
  a message, or `cultnet_rs`.
- **Shared paths:** `Mind::require_instance` under `admit_steps` and under
  every read the daemon dispatches; `Mind::admit` under the wire (this cut),
  Cut 12's hand-off (in-process) and Cut 13's tool (through the wire);
  `envelope::{encode,decode}_*` under the daemon and Cut 13's client.
- **Deletion line:** the `refusal.rs` mirror and the false doc land in
  commit (1) before `wire.rs` exists; `cargo test -p huginn-mind --lib` is
  green between commits.

### Verification

**Commits.** (1) `huginn-mind`: the two deletes, `require_instance` with
H1 re-anchored, `status`, `PipelineAdmissionBatch` derives, the leaf
re-export; suite green, H1-H67 and V1-V23 killed. (2) `huginn-mind::wire`
and the two schema files with their test. (3) `huginn-daemon` lib: `daemon`,
`envelope`, `serve`, tests. (4) `main.rs`, README/AGENTS. (5)
`eureka-cut10-mutations.psd1`, every entry killed. Soul verifies per
commit; if Hands runs long the split is between (2) and (3), recorded as
10a/10b.

**Builds.** `CARGO_TARGET_DIR=C:\Users\Meta\.cargo-target-codex` through
PowerShell (the Bash tool collapses the path; Cut 6d's scar); path-list
baseline before and after; **no `cargo clean` of any scope**. `cargo check
-p huginn-mind --lib --tests`; `cargo test -p huginn-mind --lib` (51 + the
batch in flight's count, 0 warnings); `cargo check -p huginn-daemon --lib
--bin huginn-daemon --tests`; `cargo test -p huginn-daemon --lib`; `cargo
check --workspace` (the `eureka-state` stub still builds); `cargo tree -p
huginn-daemon -e normal -d` shows **no duplicate of any GameCult crate**
(one `cultcache-rs`, one `epiphany-pipeline`, one `cultnet-rs`); the only
duplicates are the `windows-sys` majors the lock already carried plus 0.52
from `socket2` (probe).
Host = target = workstation; the Linux build is Cut 14's.

**Tests**, `crates/huginn-mind` (new, beside the existing) and
`crates/huginn-daemon`, each named for the rule it pins. Daemon tests open
`Daemon::open(tempdir, "yggdrasil")` and seed with one `instance` document
built inline (`PipelineInstance { instance, display_name, created_at, host }`
through `huginn_mind::epiphany_pipeline`), `PipelineProvenance { faculty:
Faculty::Hands, .. }`, and a fixed `now`.

| # | Test | Pins |
|---|---|---|
| 1 | `mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share` (huginn-mind) | D3: `require_instance("thought-cage")` on a `yggdrasil` mind is `ForeignInstance { declared: "thought-cage", mind: "yggdrasil" }`; `admit` with `batch.instance = "thought-cage"` returns the same value; the existing `admission_refuses_a_foreign_instance_whatever_the_transport` still passes. |
| 2 | `wire::tests::the_wire_vocabulary_is_the_minds_methods_and_status_is_derived_from_the_response` (huginn-mind) | D2: `operation()` yields the six names; `instance()` is `None` only for `Whoami`; `status()` is `rejected` exactly for `Refused(_)` and `Admit(Refused(_))`; every variant round-trips through `to_vec_named`/`from_slice`. |
| 3 | `wire::tests::published_wire_schemas_match_derivation` (huginn-mind) | D2: the two files equal `serde_json::to_string_pretty(schema_for!(..))` byte for byte. |
| 4 | `daemon::tests::a_batch_round_trips_typed_through_handle_without_a_socket` | D4: `Admit([instance])` → `Admit(Committed { writes: [instance:self] })`; `Whoami` → `MindStatus { instance: "yggdrasil", documents: 1, receipts: 1 }`; `Query { kinds: [Instance] }` → one view whose `admission.provenance` is the batch's; `View(id)` → `Some`; `OpenItems` → all empty; `History(Repo(..))` → empty. No socket, no clock read (`now` is passed). |
| 5 | `daemon::tests::a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing` | Rulings 14 and 18 across the transport: `Query { instance: "thought-cage" }` → `Refused(ForeignInstance)`; `Admit(batch.instance = "thought-cage")` → `Admit(Refused(ForeignInstance))`; `Whoami.documents` unchanged after both. |
| 6 | `daemon::tests::the_index_is_handed_every_landed_write_after_commit_and_never_decides_the_outcome` | D4, Cut 11's seam: a `RecordingIndex` receives exactly `Committed.writes` (derived writes included: a ruling that answers a question hands two refs) and can `mind.view` each at call time; a `FailingIndex` leaves the outcome `Committed` and the documents readable. |
| 7 | `envelope::tests::a_refusal_is_a_typed_response_not_a_transport_failure` | D5: `encode_response(Refused(..))` is an `OperationResponse` with `status: "rejected"`, `payload_schema: MIND_RESPONSE_SCHEMA`, decoding back to the same `HuginnMindResponse`; `Committed` → `accepted`; `AlreadyAdmitted` and `Conflict` → `accepted`. |
| 8 | `envelope::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind` | D5: six malformed messages (each code above) through `serve::answer` yield `OperationResponse { status: "rejected", payload_schema: FAILURE_SCHEMA, message_id: <echoed> }` with the named `code`, and `Whoami.documents` is 0 after all six; a non-operation message yields `CultNetMessage::Error`; a `SchemaCatalogRequest { include_schema_json: true }` yields both schemas whose `content_hash` equals the registry's. |
| 9 | `serve::tests::two_clients_get_their_own_replies_over_loopback` | D6: `bind("127.0.0.1:0")`, `run` on a thread with a stop flag; two `CultMesh::connect_rudp_client_for_endpoint` clients with `CULTNET_OPERATION_CONNECTION_ID`; one sends `Admit`, the other `Query`, each in its own thread; each receives the reply whose `message_id` is its own and decodes it typed; a third client on another connection id receives nothing within the timeout; stop flag → `run` returns and the thread joins. |
| 10 | `daemon::tests::the_daemon_refuses_loudly_when_it_cannot_open_the_mind_and_binds_nothing` | Ruling 15: a first `Daemon::open` holds the lock; a second on the same path is `Err(MindAlreadyOwned)`; a store planted for `yggdrasil` opened as `thought-cage` is `Err(ForeignInstance)`; `Daemon::open`'s signature takes no address (the bind cannot precede it), and `main`'s order is pinned by D12 below. |

**Negative greps.**

- `rg -n "compare_and_swap|admit_prepared|prepare_entry|HuginnCommitReceipt \{|stored_at|in_force|closing_resolution|pipeline_key|validate\(" crates/huginn-daemon/src` empty.
- `rg -n "!= self.instance\(\)|== self.instance\(\)|\.instance\(\) [!=]=|instance != |instance == " crates/huginn-daemon/src` empty (the daemon compares no instance).
- `rg -n "require_instance" crates/huginn-mind/src`: the definition, one call in `admit_steps`, tests; `crates/huginn-daemon/src`: one call in `handle`.
- `rg -n "Utc::now|SystemTime::now" crates/huginn-daemon/src`: exactly one, in `serve.rs`; `crates/huginn-mind/src` empty.
- `rg -n "std::env|env::var|GAMECULT_IDUNN" crates/huginn-daemon/src` empty (Cut 14's).
- `rg -n "ServiceIdentitySigner|enroll_service_identity|verify_service_identity|TrustAnchor|source_runtime_id|connect_payload" crates/huginn-daemon/src`: `source_runtime_id` only where the response sets it; nothing else (ruling 18; D8 stands).
- `rg -n "reqwest|qdrant|ollama|http://|serde_json::to_value|CultMeshRudpDocumentServer|DocumentPutRaw|SnapshotRequest" crates/huginn-daemon/src` empty except `SnapshotRequest`, which must not appear either (Cut 14 adds Idunn's presence answer if it needs one; Findings).
- `rg -n "serde_json" crates/huginn-daemon/src`: only in the schema registry build and its test.
- `rg -n "cultnet_rs|cultmesh_rs|UdpSocket" crates/huginn-mind/src` empty.
- `rg -n "serde\(remote|DocumentRefusal" crates/huginn-mind/src` empty.
- `rg -n "epiphany-pipeline" crates/huginn-daemon/Cargo.toml` empty (the leaf is reached through `huginn_mind::epiphany_pipeline`).
- `rg -n "stub" README.md AGENTS.md`: only about `eureka-state`.
- `git diff --stat <batch commit> -- crates/huginn-mind/src/query.rs crates/huginn-mind/src/docs.rs tools/eureka-cut9-mutations.psd1` empty.

**Mutations, `tools/eureka-cut10-mutations.psd1`.** Run:

```
$env:CARGO_TARGET_DIR = 'C:\Users\Meta\.cargo-target-codex'
powershell -File F:\Projects\Epiphany\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
    -Entries tools/eureka-cut10-mutations.psd1 `
    -Target crates/huginn-mind/src/mind.rs,crates/huginn-mind/src/wire.rs,crates/huginn-daemon/src/daemon.rs,crates/huginn-daemon/src/envelope.rs,crates/huginn-daemon/src/serve.rs `
    -Test 'cargo test -p huginn-daemon --lib'
```

Entries whose test lives in `huginn-mind` carry their own `Command = 'cargo
test -p huginn-mind --lib'`. Anchors are content Hands lands, matched once.
Each ruling has a **revert** (the rule absent) and, where one exists, a
**loosening**.

| # | Rule | Revert | Loosening | Killed by |
|---|---|---|---|---|
| D1 | 14: one instance check, the mind's (D3) | `require_instance` body → `Ok(())` (H1's shape, now shared) | compare `declared.0.len()` to `self.instance.0.len()` | test 1 and test 5 (one anchor, two suites, one owner) |
| D2 | 14/18 on the wire: the daemon passes the declared instance through | `handle`: `Admit(mut batch)` → `batch.instance = self.mind.instance().clone()` before `admit` | reads: `require_instance` called only for `Query`, not `View`/`OpenItems`/`History` | test 5 |
| D3 | refusals are data, never a transport failure | `encode_response`: `Refused(_)` → `encode_failure(.., OperationFailure { code: "refused", .. })` | `status()` returns `accepted` for `Refused(_)` | test 7 |
| D4 | an undecodable envelope touches no mind | `decode_request`: on `payload-not-a-request` return `Ok((id, HuginnMindRequest::Whoami))` | `wrong-service` check dropped | test 8 |
| D5 | the operation string is checked, not trusted | `operation-mismatch` check dropped | compare case-insensitively | test 8 (an `Admit` payload under `operation: "query"`) |
| D6 | the index never decides an outcome | `handle`: `index.committed` `Err` → return `Admit(Refused(Unavailable { .. }))` | `index.committed` called before `admit` (the sink then sees refs it cannot `view`) | test 6 |
| D7 | the index sees every landed write | `index.committed(mind, &[])` | pass `writes[..1]` | test 6 (the derived write) |
| D8 | each reply goes to its own session | `run`: send the reply to the first session in `hub.sessions()` | send to every session | test 9 |
| D9 | only the operation connection id is served | `bind`: `CULTNET_OPERATION_CONNECTION_ID` → `CULTMESH_RUDP_DOCUMENT_CATALOG_CONNECTION_ID` | — | test 9 (the third client) |
| D10 | `whoami` is derived from the mind | `status()`: `documents` → `self.image.len()` (counts the epoch record and receipts) | `receipts` → `0` | test 4 |
| D11 | 15: a mind that will not open is refused before anything listens | `Daemon::open`: `Mind::open` `Err` → open an empty `Mind` at a sibling path `<instance>-fallback` | `MindAlreadyOwned` mapped to `Ok` after a retry with the lock file deleted | test 10 |
| D12 | 15: bind after open | `main.rs`: `startup(options) -> Result<(Daemon, Hub, Registry)>` is the one place both happen; the mutant moves `bind` above `Daemon::open`. Test 10 holds the mind's lock, calls `startup` with a free port, asserts `Err`, then binds that port itself: under the mutant the hub holds it and the test's bind fails | — | test 10 |
| D13 | the wire vocabulary equals `Mind`'s methods | `operation()`: `History` → `"query"` | — | test 2 |
| D14 | the published schema equals the derivation | the request schema file has one whitespace change | — | test 3 |
| D15 | `AlreadyAdmitted` and `Conflict` are answers | `status()`: `Admit(AlreadyAdmitted { .. })` → `rejected` | — | test 7 |
| D16 | a non-operation message is answered, not dropped | `answer`: the `_ =>` arm returns `Error { error: "" }` (CultNet's `validate_message` refuses the empty string, so the send fails and the client hangs) | — | test 8 (the `Error` text is non-empty and names both accepted messages) |

Stated limits: the loop's "log and serve past" on a hostile datagram has no
mutant because the hub's error path is CultLib's (Findings); the reliable
window limit is probed, not mutated; the SIGTERM path is not testable in
the harness and is pinned by reading `main`.

**Operator checks before landing:** Q22 and Q23 below. Q22 has a recommended
default Hands builds under; Q23 changes nothing in this cut whichever way it
goes.

### Subtraction estimate

`huginn-mind`: −about 25 (the mirror, the false doc, the inline A1), +about
150 (`wire.rs` 90, `require_instance` and `status` 30, exports 5, doc 5),
+about 90 tests, +2 schema files (about 200 KB of derived JSON, not counted
as source). `huginn-daemon`: +about 520 outside tests (`daemon.rs` 110,
`envelope.rs` 170, `serve.rs` 140, `main.rs` 90, `lib.rs` 10), +about 480
tests, +1 lib target; the bin target exists as a stub and becomes real.
Entries file about 260. Total about +1,500 with tests and entries against
the old section's +900; the difference is the envelope codec and the
malformed-input surface the old section did not have, and ten tests for
five.

Liability retired before it shipped: `SinkHandle`/`SnapshotHandle`, a
`cultmesh-rs` and `fs2` dependency, a pending-index slot, a second copy of
Odin's Idunn bootstrap on a platform where it does not compile, a
process-spawning smoke, and four wire operations that were presets or Cut
12's. Retired in the tree: the dead serde mirror and a false doc.

Kinds, formats, epoch, leaf pin, `.cc` files: zero.

### Build budget

- **Packages that compile:** `huginn-mind` (lib + tests), `huginn-daemon`
  (lib + bin + tests), `cultnet-rs` and its 40 new transitive packages
  (probe: `aead, aes, aes-gcm, base64, base64ct, cipher, const-oid, ctr,
  curve25519-dalek(+derive), der, ed25519, ed25519-dalek, fiat-crypto,
  ghash, hmac, inout, opaque-debug, pkcs8, polyval, ppv-lite86, rand,
  rand_chacha, rand_core, rmpv, rustc_version, semver, signal-hook(+registry),
  signature, socket2, spki, subtle, universal-hash, wasi, wasip2,
  wit-bindgen, zerocopy(+derive), zeroize`; lock 93 → 139 entries). Most of
  their rlibs were warm from Epiphany builds: the probe built in 30 s.
- **Targets:** one lib target new; the bin target exists. Debug only,
  workstation host = target, no features, no codegen, no release profile.
- **Footprint:** baseline this pass 10,619 paths, 8.74 GiB under
  `C:\Users\Meta\.cargo-target-codex\debug`; the probe left it at 11,030
  paths, 9.08 GiB, and the cut's own build lands on top of that. **Expected
  delta for the cut: +100 to +300 paths, +0.1 to +0.3 GiB** beyond the
  probe's, all under `debug/`. Drive C: has over 250 GiB free. Hands records
  the path list before and after and reports a miss.
- **Retention:** the shared dir is the operator's; nothing is cleaned.

### Operator questions

- **Q22. Is the organ's surface request/response over CultNet RUDP, a
  CultMesh state publication, or both?** **A. Request/response through
  `CultNetRudpServerHub` with `cultnet.operation_request/response.v0`
  (recommended; D1).** Admission and typed queries are request/response by
  nature; CultLib's envelope has C# parity and Eve names it as the ABI
  carrier; the schema catalog is the one JSON surface. **B. CultMesh
  publication only:** admission as a document put and reads as snapshots.
  Refused above: the document server cannot reply from its sink, a snapshot
  cannot carry a query, and a response store would be a cache pretending to
  be truth. **C. Both:** A for the operations, plus the daemon publishing
  `MindStatus` (and later an Eve surface) as a CultMesh document through
  Odin's catalog for dashboards to pull. C is A plus Q23's answer, not a
  third transport; it costs Odin registration, which is Cut 14's body. If C
  is wanted now, `MindStatus` is already the document, and the publication
  is one `publish_cultnet_message_to_rudp_catalog` call on a heartbeat that
  Cut 14 owns. **Ruled A, 2026-09-16** ("I take your recommendations").
- **Q23. Where does the operator interface land?** Doctrine says an earned
  daemon publishes its capabilities as Eve DSL through CultMesh, lowered by
  others. **A. Not this cut (recommended).** This cut owns `MindStatus` and
  the wire schemas; a `gamecult.eve.provider_advertisement.v1` and a
  `surface_state` composition need a Verse identity, freshness and a
  heartbeat that exist only once Cut 14 puts the daemon beside Odin on
  Yggdrasil. Land it as a named cut after 14 ("Verse presence": the
  advertisement, the status surface, the Idunn health it rides with), before
  the proof campaign so Cut 16 can read the mind from a dashboard. Cuts 15
  and 16 are skill wiring and the proof; neither is the right owner. **B.
  Fold it into Cut 14**, since Odin registration is part of being deployed
  on Yggdrasil; the cost is a larger Cut 14 that already carries the Idunn
  handshake, the backup and the route. **C. Build the advertisement here**
  against a workstation-local Odin: a surface with no dashboard, and a
  `verseId` chosen before the Verse exists; not recommended. **Ruled A,
  2026-09-16**: a named "Verse presence" cut after Cut 14, before the proof.

Decisions Self can overturn without an operator: no transport trait (D4);
real redb minds in the daemon's tests rather than a promoted `MemoryStore`
(D4); the failure-schema mirror carried in the daemon (D5); the option
spelling `--state-root --instance --bind` (D7); the base64 cost of the
operation envelope (about a third over raw bytes, accepted as CultNet's
contract).

### Findings not assignable to this cut

- **`CultNetRudpServerHub::receive_event_once` propagates a hostile
  datagram as `Err`** (`receive_packet_once`: `decode_rudp_packet(&wire)?`)
  and the peer limit likewise, so every hub service must treat `Err` as a
  discard or die on the first stray packet; Idunn's `HostActuatorHub::service`
  propagates it. CultLib's, under the QUIC campaign's owner; a stray datagram
  on a mesh port is not hypothetical.
- **The Rust `cultnet-rs` has no `gamecult.cultnet.operation_failure.v1`
  type or published schema**, though the C# reference defines
  `CultNetOperationServer.FailureSchemaId` and `CultNetOperationFailure
  { code, message }`, and `contracts/cultnet/` publishes only the request
  and response schemas. This cut carries a two-field mirror with the C#
  keys; parity belongs in CultLib.
- **The Rust `CultNetOperationServer` is single-peer and unused in Rust**;
  the C# one is a multi-handler dispatcher. Either it grows to match or it
  is deleted; not this campaign's.
- **The Idunn activation handshake is a second message family.** Odin
  answers Idunn's route challenge as a `SnapshotRequest` for its own
  presence record (`exact_self_presence_query`) through the document
  server's snapshot source, and publishes presence with
  `publish_cultnet_message_to_rudp_catalog`. Under D1 the hub can answer a
  `SnapshotRequest` frame with a `SnapshotResponseRaw` (it is one more
  `CultNetMessage` on the `schema` channel), so Cut 14 adds an arm to
  `serve::answer` rather than a second server; but Cut 14's spec must say
  exactly which message Idunn's route driver sends and how a hub-based
  service satisfies `route_required = true`. Odin's `main.rs` uses
  `std::os::fd` and builds on Linux only; the Idunn arm will be `cfg(unix)`
  or Cut 14 must move the signer-descriptor reading behind a port.
- **`gamecult-ops/runbooks/odin-yggdrasil.md` describes the pre-v2 body**
  (Compose container, `/srv/odin/current`, `odin.service`) while the
  binding is `gamecult.idunn.operator_binding.v2` under `idunn-odin`; the
  runbook Cut 14 models on it is modelling a stale one.
- **Cut 12 has no wire operation here.** If the two minds of a hand-off live
  in two daemons, Cut 12 needs either a wire `HandOff`/`Import` pair (one
  more variant each in `wire.rs`) or an offline tool that opens both stores;
  if both minds are stores on one host, `hand_off(&mut from, &mut to, ..)`
  is in-process and no wire form exists. Cut 12's refresh decides; the wire
  enum widens additively either way.
- **Cut 13's `tool_schemas_equal_the_published_schemas`** compares against
  Epiphany's `schemas/cultnet/` (the pipeline documents); it should compare
  the tool schemas against Huginn's `schemas/cultnet/huginn.mind_*.v1` (the
  operations). And Cut 13 gains a `history` tool (Q17 B's obligation, Cut
  9's finding) and loses `rulings_in_force`/`stewardship` as tools in favour
  of presets. Cut 13's refresh.
- **`MemoryStore` is `cfg(test)`-private to `huginn-mind`.** The daemon's
  tests do not need it; if Cut 13's in-process smoke wants a mind without a
  file, promote `test_stores` behind `#[cfg(any(test, feature =
  "test-stores"))]` (one line) in that cut.
- **A `Query` page of 200 maximal views may exceed the hub's reliable
  window** (1024 packets × 1200 bytes); `send_many` then errors and the
  client times out. Soul measures it against the leaf's bounds; if real,
  Cut 13 caps `limit` by default or the hub options grow. Not a second page
  format.
- **The wire schemas are large** (`PipelineQueryPage` alone 42.9 KB; the
  response schema will inline every pipeline value type). Fine on the
  catalog (16 MiB payload cap) and as files; noted so nobody reads the size
  as a defect.
- **`Faculty::SelfFaculty`** is the wire spelling clients will see (carried
  from Cut 9).
- **`README.md`/`AGENTS.md`** go stale again at Cut 11 and Cut 13; the
  "describe the live system" tension recurs until the workspace is whole.

### Pinned HEADs

- Huginn `eureka/memory-organ` at `8fc39b1`; at spec time the main tree
  carried the concurrent Hands batch uncommitted in
  `crates/huginn-mind/src/query.rs`, `docs.rs`, `tools/eureka-cut9-mutations.psd1`
  and a `docs.rs.eureka-mutation-original` harness sidecar. This spec
  anchors by name and touches none of those files.
- Epiphany `codex/eureka-pipeline-state` at `fb18395f`; leaf at `d5a36c2a`.
- CultLib `main` `47aa7b6`, Rust runtimes identical to `a0813c6`.
- Odin `main` `5a015d9`; Idunn `main` `5b3f646`; gamecult-ops `main`
  `36a466f`.
- Probe artifacts: `scratchpad/cut10-wt` (detached at `8fc39b1`, one `cargo
  test -p huginn-daemon --lib`, 1 passed in 0.08 s after a 30.4 s build,
  removed and pruned); `scratchpad/cut10-probe-lib.rs` and
  `cut10-probe-Cargo.toml` kept as evidence. The shared target dir read
  10,619 paths / 8.74 GiB before and 11,030 / 9.08 GiB after; nothing was
  cleaned. No file in any repo was written and no `notes/` file was touched.

## Cut 11. Qdrant collections and Ollama embeddings

- **Repo/branch:** Huginn, same branch. Depends on Cut 10 and Q5.
- **Deletes first:** none.

**Adds.**

- `crates/huginn-mind/src/index.rs`: the `EmbeddingPort` and `IndexPort` traits
  and `IndexPoint`/`IndexFilter`/`IndexHit` from D5, plus the `pending_index`
  document and the retry rule.
- `crates/huginn-daemon/src/qdrant.rs` and `src/ollama.rs`: `reqwest::blocking`
  adapters over the endpoints R3 establishes. `reqwest` is added here, in the
  daemon only, never in `huginn-mind`.
- Daemon flags `--qdrant-url`, `--ollama-base-url`, `--ollama-model`,
  `--embedding-dimensions`, following `epiphany.service`'s existing spelling
  (R16) so the unit reads like its neighbour.

**Collection compatibility.** Port the deleted `CollectionCompatibility` idea
(`856648de^:semantic_backend.rs:47-57`): the collection stores its
`managed_by`, `corpus_kind`, `projection_version`, `embedding_model` and
`vector_size`, and the daemon refuses to write into a collection whose
compatibility record disagrees. That is what stops a model change silently
mixing 1,024-dim and other-dim vectors.

**Authority map.**

- **Owner:** the typed mind owns truth; Qdrant owns nothing.
- **Inputs:** admitted documents' bounded text fields.
- **Outputs:** Qdrant points; semantic hits resolved back through `get`.
- **Derived state:** **the entire collection.** It is rebuildable by
  `--reindex`.
- **Forbidden writers:** nothing writes a Qdrant point except the daemon's
  post-admission path; nothing reads a pipeline document *from* Qdrant as truth;
  in-force status is never indexed.
- **Shared paths:** the `semantic` branch of `query` (Cut 9).
- **Deletion line:** n/a.

**Verification.**

| Test | Pins |
|---|---|
| `admission_succeeds_when_the_index_is_unreachable_and_records_pending` | The index never rejects a write |
| `pending_index_is_retried_and_cleared` | — |
| `reindex_rebuilds_the_collection_from_the_store` | The projection is disposable |
| `semantic_hits_resolve_through_the_typed_store` | Qdrant is not truth |
| `incompatible_collection_is_refused` | The compatibility record |
| `receipts_identity_and_provenance_are_never_indexed` | — |

The first four run against **mock ports**; `huginn-mind` has no network
dependency, which is the point of the trait boundary.

- **Live adapter check (operator or Hands, on Yggdrasil):** one end-to-end
  admit-then-semantic-query against the real Qdrant and the real Ollama,
  confirming 1,024 dimensions.
- **Mutations:** index before committing; index the in-force status; drop the
  compatibility check.
- **Negative grep:** `rg -n "reqwest|qdrant|ollama" crates/huginn-mind/src`
  empty.

**Subtraction ledger:** +about 800 lines. +`reqwest` (daemon only), +1 Qdrant
collection.

## Cut 12. Stewardship hand-off and mind import

- **Repo/branch:** Huginn, same branch. Depends on Cut 11.
- **Deletes first:** none.
- **Adds:** `crates/huginn-mind/src/handoff.rs`.
  - `hand_off(&mut Mind from, &mut Mind to, HandOffRequest)` admits the
    `hand_off` document **into both minds** in one logical operation; Cut 8's
    derivations do the rest: a `Withdrawn` resolution of the source side's
    stewardship and a new `stewardship` at the next sequence on the target
    side. *(First written as "the superseding `stewardship` on the source
    side"; Cut 8 landed a withdrawal, and Cut 6d gave stewardships a
    sequence so a return is an ordinary second hand-off. A hand-off is a
    transfer, not a lease (Q18); this cut models no return and gains one
    test, `a_repo_handed_back_takes_the_next_sequence`, that two hand-offs
    A→B→A leave A stewarding at n2 and B withdrawn.)*
  - **Import replays in sequence order, one record per batch where a
    subject's history is concerned.** Soul on the Cut 6d follow-up: each
    record in a batch sees the later one as the subject's latest, so
    `[n1, n2]` in one batch is `OutOfSequence { 3, 1 }` and reversed is
    `AlreadyResolved`. Spec-consistent, and a constraint on this cut's
    import loop. Replay is idempotent per batch (`AlreadyAdmitted`),
    including batches with derived writes, after the follow-up's fix batch.
  - `import(&mut Mind, foreign: &Path, HandOffRef)` replays the named
    documents' exact envelopes through `admit` against the target mind. Envelope
    bytes are carried, not reserialised, so additive fields written by a newer
    binary survive.

**Not a merge tool.** The deleted D5 merge tool existed because two clones could
diverge. Two minds cannot diverge: each has one owner. Import is a *transfer* of
named documents under a recorded hand-off, and it refuses anything not named in
the hand-off.

**Authority map.**

- **Owner:** `huginn-mind`'s hand-off module, but it writes only through
  `admit`, so the per-kind rules still apply to imported documents.
- **Inputs:** the source mind, the target mind, a `HandOffRequest`.
- **Outputs:** a `hand_off` in both minds, two `stewardship` writes, two
  receipts.
- **Derived state:** the stewardship assignments.
- **Forbidden writers:** import may not bypass `admit`, may not reserialise an
  envelope, and may not import a document the hand-off does not name.
- **Shared paths:** `admit`.
- **Deletion line:** n/a. This replaces the never-built merge tool, which Cut 4
  deleted from the design.

**Verification.**

| Test | Pins |
|---|---|
| `hand_off_writes_both_minds_or_neither` | Atomicity across two stores |
| `import_replays_envelope_bytes_exactly` | No reserialisation |
| `import_refuses_a_document_the_hand_off_does_not_name` | — |
| `imported_documents_still_pass_every_admission_rule` | No bypass |
| `hand_off_is_idempotent` | Replay returns `AlreadyAdmitted` |
| `hand_off_between_the_same_instance_refuses` | — |

- **Mutations:** import without re-validating; reserialise on import; write the
  target before the source and fail the source.
- **Builds:** `cargo check -p huginn-mind --lib --tests`.

**Subtraction ledger:** +about 500 lines.

## Cut 13. `eureka-state`

- **Repo/branch:** Huginn, same branch. Depends on Cut 10 (the wire) and Q6.
- **Deletes first:** none.
- **Adds: `crates/eureka-state`.** Dependencies `anyhow`, `cultnet-rs`,
  `epiphany-pipeline`, `rmcp = { version = "2.2.0", default-features = false, features = ["server","macros","transport-io"] }`,
  `schemars`, `serde`, `tokio = { features = ["macros","rt-multi-thread","io-std"] }`.
  It does **not** depend on `huginn-mind`: it is a client, and linking the rule
  engine into the client would invite a second validator.
  - `src/lib.rs`: `EurekaStateServer` with a `ToolRouter` and the seven tools.
  - `src/main.rs`: `EurekaStateServer::from_env()?.serve(rmcp::transport::stdio()).await?.waiting().await`.

**Authority map.**

- **Owner:** none over state. The server is a transport shim.
- **Inputs:** JSON-RPC requests; two environment variables.
- **Outputs:** typed JSON.
- **Derived state:** none. **No lease cache, no store handle, no local file.**
- **Forbidden writers:** the package may not open a CultCache store, may not
  validate a document, may not derive status, and may not write anything to
  disk. This is the structural guarantee behind "it never writes a second copy".
- **Shared paths:** the CultNet surface from Cut 10.
- **Deletion line:** n/a.

**Verification.**

| Test | Pins |
|---|---|
| `every_tool_round_trips_against_a_live_daemon` | Pipeline smoke on an ephemeral port |
| `an_unreachable_organ_returns_typed_unavailable_from_every_tool` | The target's honesty invariant |
| `tool_schemas_equal_the_published_schemas` | Compare `tools/list` with `schemas/cultnet/` |
| `whoami_reports_unreachable_without_failing` | Rehydration can check first |
| `a_refusal_is_an_outcome_not_a_jsonrpc_error` | P4's `structuredContent` path |

- **Negative greps:** `rg -n "BackingStore|CultCache::|std::fs::write" crates/eureka-state/src`
  empty.
- **JSON-RPC exchange:** pipe `initialize`, `notifications/initialized`,
  `tools/list` and one `query` into the built binary, as P4 did.
- **Build and install (Hands):**
  `cargo install --locked --path crates/eureka-state --root C:\Users\Meta\.eureka`
  with the shared `CARGO_TARGET_DIR`. Unprobed: whether `cargo install` honours
  `CARGO_TARGET_DIR`; Hands confirms from the build log and reports the path.
- **Registration: the operator's, not Hands'.** Hands reports the binary path
  and the confirmed `claude mcp add` line from D7; the operator runs it.

**Subtraction ledger:** +about 600 lines. +1 binary, +`rmcp`, +`tokio`.

## Cut 14. Deployment

- **Repos:** Huginn `eureka/memory-organ` (the recipe) and gamecult-ops `main`
  (the binding, unit inputs, backup and tunnel). Depends on Cut 11 and Q7.
- **Deletes first:** none.

**Adds, Huginn: `deployment/idunn/recipe.toml`**, `gamecult.idunn.target_declaration.v1`,
modelled on Odin's (R17):

- `[[steps]]` test then build, runner `rust-build`,
  `cargo test --locked -p huginn-mind` and
  `cargo build --locked --release -p huginn-daemon --bin huginn-daemon`.
- `[[artifacts]]` `huginn-daemon` from `target/release/huginn-daemon`.
- `[service]` with `--state-root`, `--idunn-projection`, `--idunn-anchor`,
  `--qdrant-url`, `--ollama-base-url`, `--ollama-model`; `transport = "rudp"`;
  `route_required = true`; required environment
  `GAMECULT_IDUNN_CANDIDATE_BIND`, `GAMECULT_IDUNN_PROCESS_WRITE_LEASE`,
  `GAMECULT_IDUNN_RUNTIME_BUNDLE`.
- `[service.health] contract = "huginn.runtime-health.v1"`.
- `[state] schema_generation = "huginn-v1"` with one slot: `minds`, relative
  path `minds`, kind `cultcache-directory`, writer `process-bound-single-writer`,
  recovery `preserve`, startup `create-or-open-after-write-lease`.
  **`recovery = "preserve"` is the load-bearing line: a mind is never
  reconstructible from a release.**
- `[[provides]]` capability `huginn.instance-mind`, schema
  `huginn.mind_response.v1`.
- `[[dependencies]]` — this is where Qdrant and Ollama are declared, following
  Ghostlight's shape (`Ghostlight/deployment/idunn/recipe.toml:168-187`): a
  `shared-infrastructure` dependency on the Qdrant endpoint and one on the
  embedding endpoint, both `startup = "before-promotion"`, so a candidate that
  cannot embed is never promoted.

**Adds, gamecult-ops: `idunn/yggdrasil/bindings/huginn.toml.in`**,
`gamecult.idunn.operator_binding.v2`, modelled on `odin.toml.in`:

- `[repository]` origin `https://github.com/GameCult/Huginn.git`, `admitted_ref`
  the campaign branch until it merges, then `refs/heads/main`;
  `minimum_revision = "PROVISIONED_HUGINN_MINIMUM_REVISION"`;
  `recipe_path = "deployment/idunn/recipe.toml"`.
- `[runners.rust-build]` identical to Odin's pinned rust image and caps.
- `[workload]` `state_group = "huginn-v1-state"`, `unit_prefix = "idunn-huginn"`,
  `release_root = "/srv/gamecult/idunn-releases/huginn"`,
  `state_root = "/var/lib/gamecult/huginn-v1"`, `network = "host-private"`,
  `hardening = "strict"`.
- `[route]` `stable_endpoint = "rudp://10.77.0.1:17872"`, private range
  `27880-27887` (R21).
- `[brakes]` **both**, separately, per `F:\Projects\CLAUDE.md` and R18:
  `deployment_store = "/var/lib/gamecult/idunn-brakes/huginn-deployment-brake.cc"`
  and `lifecycle_store = ".../huginn-lifecycle-brake.cc"`. Changing the
  artifact is deployment; restarting the admitted body is continuity. One brake
  must not gate both.
- `[rollout] strategy = "candidate-then-promote"`, `retain_releases = 2`.
  `drain_seconds` is raised above Odin's 10 to let an in-flight admission finish;
  Hands proposes a value and reports it.
- `[placement]` `desired_replicas = 1`, `nodes = ["yggdrasil"]`. **One replica is
  an invariant, not a capacity choice:** two replicas would be two writers to one
  mind.

**Backup owner — the target requires one, and this is it.** The daily authority
backup is an explicit path list (R19); a path not named is not backed up.
**One edit, not two:** `scripts/backup-gamecult-authority-yggdrasil.sh:81-105`
gains `var/lib/gamecult/huginn-v1` to the `tar` path list.
`systemd/gamecult-authority-backup.service:15` needs **no** change — its
`ReadOnlyPaths` already grants the whole `/var/lib/gamecult` tree, so the new
state root is readable the moment it exists. Hands must not add a redundant
line there.
The script freezes named writers before tarring (`:72-74`, currently
`epiphany-swarm.service` and `epiphany.service`); the `idunn-huginn` unit is
added to that loop so a mind is never captured mid-commit.
**Named owner: `gamecult-authority-backup.service`, daily at 03:20 UTC.**

**Workstation reach.** `scripts/start-yggdrasil-tunnel.ps1:13-24` gains one
entry: `@{ Name = "huginn-organ"; LocalPort = 17872; RemoteHost = "127.0.0.1"; RemotePort = 17872 }`,
and `runbooks/yggdrasil-ssh-tunnel.md:86-88`'s port table gains the row.
**Caveat, and it is a real one:** the existing forwards are TCP local forwards,
and the organ speaks RUDP over **UDP**. `runbooks/yggdrasil-ssh-tunnel.md:32-39`
already records this exact limitation and recommends WireGuard rather than SSH
forwards when raw UDP must cross. **See Q7:** the workstation almost certainly
reaches the organ over the existing WireGuard mesh (`10.77.0.1`), not the SSH
tunnel. The tunnel row is only added if Q7 chooses a TCP transport.

**Adds, gamecult-ops: `runbooks/huginn-yggdrasil.md`**, following
`runbooks/odin-yggdrasil.md`'s five headings: Authority map, Release body,
Initial admission, Verification, plus a Recovery section naming the backup and
the `--reindex` rebuild.

**Authority map.**

- **Owner:** Idunn owns deployment and continuity actuation. Huginn owns its own
  state and health.
- **Inputs:** the admitted revision, the binding, the brakes.
- **Outputs:** a running `idunn-huginn` unit, published presence health.
- **Derived state:** the release root; the Qdrant collection.
- **Forbidden writers:** no operator script deploys Huginn directly; nothing but
  Idunn writes the release root; nothing but the daemon writes the state root.
- **Shared paths:** the Qdrant container (Q7), the embedding endpoint, the
  backup timer.
- **Deletion line:** n/a.

**Verification.**

- **Operator, and only the operator:** admit the binding, run `idunn up`,
  confirm the unit reaches Ready, confirm health publishes, confirm the route
  answers on `10.77.0.1:17872`, then run one `eureka-state whoami` from the
  workstation.
- **Restart-preserves-state check:** admit a document, restart the unit through
  Idunn, query it back. This is the check that `recovery = "preserve"` is real.
- **Backup check:** run the backup unit once and confirm
  `var/lib/gamecult/huginn-v1` is in the archive listing.
- **Brake check:** set the deployment brake and confirm a redeploy is refused
  while a restart still succeeds. This falsifies the one-brake conflation.
- **Hands does not deploy.** Deployment is an Idunn actuator gated on root and
  `IDUNN_ACTUATOR=1` (`scripts/deploy-epiphany-yggdrasil.sh:4-8`).

**Subtraction ledger:** +about 250 lines of ops configuration. +1 systemd
workload, +1 route, +2 brake stores, +1 backup path, +1 runbook.

## Cut 15. Skill wiring

- **Repo:** `GameCult/Eureka` `main` at `6ca7882`, checked out at
  `~/.claude/skills/eureka`. It is now a git repo, so Cut 15's edits land as
  commits there. Depends on Cut 13 being registered.

**`SKILL.md` — exact edits.**

- **`:8-18`, the substrate paragraph.** Replace. Eureka keeps rulings, specs,
  reports, verdicts, findings and follow-ups in an instance's mind, owned by the
  Huginn memory organ and reached through the `eureka-state` MCP tools. Delete
  the claim that Epiphany findings are "queried semantically" — Epiphany has no
  semantic query since `856648de`. Delete the sentence pointing semantic search
  at voidbot; the organ owns its own index (ruling 16).
- **`:57-70`, §0.** Add: at campaign start Self calls `whoami`, then admits
  `campaign` and `target`. Delete nothing about the target document: campaign
  prose stays in the repo (target, Invariants).
- **`:72-125`, §1.** "Imagination produces the cut map" becomes: Imagination
  admits `cut_spec` revisions and `question`s with `faculty: Imagination`.
  "Commit the map" (`:122-123`) becomes: nothing to commit — the mind is not in
  git. **This is the line that must change, or agents will keep trying to commit
  a store that no longer exists.**
- **`:126-145`, §2.** Rulings are admitted with `answers`, `choice` and
  `operator_quote`. Supersession is an explicit `resolution { Superseded }`,
  never an edit.
- **`:146-168`, §3.** The Hands brief carries the `cut_spec` id and the in-force
  ruling ids. Hands admits its `cut_report`.
- **`:169-188`, §4.** Soul admits one `verdict` plus its `finding`s, each with
  `range` and `evidence`.
- **`:189-212`, §5.** Triage outcomes are `resolution` documents.
- **`:213-228`, §6.** The status header becomes `open_items` plus `query`. Self
  still reconciles the subtraction ledger, from `cut_report.structural_delta`.
- **`:230-252`, Self's discipline.** "Keep the maps committed and current"
  becomes: keep the target committed; never restate typed state in prose. Delete
  the one-runner rule and the `WriterLeaseHeld` line — there is no lease. Add:
  when the organ is unreachable, stop and say so; never keep a second copy.

**`references/briefs.md` — exact edits.**

- **`:16-58`, Imagination.** Read first: `rulings_in_force`, `open_items`, and
  the prior `cut_spec` and `cut_report`. Output: admitted documents plus a short
  report of ids.
- **`:60-113`, Hands.** "The spec is `<section>` of `<map>`" (`:63`) becomes
  "`get <cut_spec id>`". The report becomes `admit cut_report`; the prose report
  shrinks to the receipt id and blockers. Keep every git and mutation scar at
  `:74-95` — those are about code, not state.
- **`:115-148`, Soul.** Scope is the `cut_report` id and its `range`. Report is
  `admit verdict` plus findings.
- **`:150-166`, Mind Steward.** Candidates come from `query` over resolutions
  admitted since the last boundary. The steward never writes the mind.
- **New section, "Rehydrate."** A fresh agent calls, in order: `whoami`,
  `rulings_in_force`, `open_items`, `query { kinds: [cut_report], campaign }` —
  and nothing else.

**`references/cut-map.md` — rewrite, do not delete.** It becomes "Typed campaign
state": the document set, the keys, the resolution matrix, and query recipes,
pointing at the Epiphany schemas as owner. The markdown cut map is retired for
new campaigns, and the reason stands unchanged from the old Cut 5: a committed
rendering needs a renderer and a regeneration discipline, which is a second
owner that goes stale exactly the way this file did. The target document stays
prose. Past cut maps, including this one, remain history.

**`references/changelog.md`** gains a dated entry with this cut map's evidence.

**Verification (Soul, by reading).**

- `rg -n "cut map|status header|repo_root|WriterLeaseHeld|lease" ~/.claude/skills/eureka`
  hits only history and changelog lines.
- Every kind in D2, including `instance`, `stewardship` and `hand_off`, is named
  by at least one brief.
- No brief asks for prose relay of a typed artifact.

**Subtraction ledger:** prose shrinks; `references/cut-map.md` is replaced rather
than extended.

## Cut 16. Proof campaign

- **Task:** the two CultLib follow-ups on `CultRecordRefFormatter` recorded in
  `F:\Projects\Aetheria\docs\cultcache-migration-cut.md:77-83`. Collapse the dead
  empty check at `src/GameCult.Caching.MessagePack/CultRecordRefFormatter.cs:15-16`,
  where `CultRecordKey` already equates null and `""`; and make the contract's
  reason for accepting nil also name code from before `452f928`.
- **Why this task, unchanged:** real and small, one genuine operator question
  (whether nil stays accepted on read indefinitely or gets a sunset, which
  changes the collapsed reader), Soul-falsifiable at the wire layer by decoding a
  1.0.58 nil store, and CultLib is public with `main` as its default branch.
- **Repo/branch:** CultLib `eureka/cultrecordref-followups` from `main`
  `a0813c6`. Depends on Cuts 14 and 15.

**Run.** Note what is *absent* compared with the old Cut 6: no attribute lines
to commit, no store to commit, no branch binding.

1. Self calls `whoami` and confirms the organ is reachable.
2. Self admits `campaign` and `target`. The campaign's repo must be one the
   instance stewards, so Self admits a `stewardship` for `GameCult/CultLib`
   first if absent.
3. Imagination admits `cut_spec cut-1.r1` and `question Q1-1`.
4. The operator rules; Self admits the `ruling` answering Q1-1.
5. Hands admits `cut_report cut-1.h1`.
6. Soul admits `verdict cut-1.s1` and its findings.
7. Self admits `resolution`s and `follow_up`s.

**Pass criteria. All must hold.**

1. Every artifact exists in the instance's mind with a receipt. No `*-cut.md`
   exists for this campaign, and the Hands and Soul briefs contain ids, not spec
   prose.
2. **Nothing about this campaign is committed to CultLib except the code change.**
   `git status` on the branch shows no `.epiphany/` path and no store file.
   This is the negative proof that minds left git.
3. At least one refusal is exercised live and returned typed — for example a
   `verdict` whose `Falsified` claim lacks a `Confirmed` finding, or a
   `cut_report` before its spec is admitted.
4. At least one resolution is admitted, and `open_items` afterwards equals
   exactly the unresolved set Self states from its own records.
5. **Rehydration.** A fresh agent with no transcript, no repo docs and no memory,
   given only the instance slug and the campaign slug, answers a fixed
   questionnaire using only `whoami`, `rulings_in_force`, `open_items`, `query`
   and `get`:
   - Which rulings are in force, and what did the operator say?
   - What landed, at which SHAs?
   - Which findings were Confirmed, and how was each resolved?
   - What follow-ups remain, and why can each wait?

   A Soul pass grades the answers against the mind and git. The proof fails on
   any factual miss.
6. **Unreachable is loud.** The operator stops the `idunn-huginn` unit; the next
   `admit` returns a typed `Unavailable` naming the endpoint, the agent stops,
   and **no local file appears anywhere**. `git status` and the scratchpad are
   both clean. This is the target's availability invariant, proved by a
   deliberate outage rather than asserted.
7. **Semantic recall.** A `query` with `semantic` set returns the Cut 16 ruling
   from a paraphrase that shares no exact words with it, and the hit resolves
   through `get`.

**Fail:**

- any criterion misses;
- any agent relays a typed artifact as prose to the next agent;
- Self reads source to verify instead of routing to Soul;
- any agent writes a local copy of state when the organ is unreachable.

**Subtraction ledger:** the CultLib diff is about −3 lines of C# plus one
contract sentence.

## Subtraction ledger

Estimates. Self reconciles each at its landing; a miss is allowed but must be
explained.

| Cut | Removed | Added | Deps / formats / targets |
|---|---|---|---|
| 4 | ~1,291 | ~20 | −1 store format, −1 lease, −2 git preconditions |
| 5 | ~46 | ~8 | none |
| 6 | ~623 from `epiphany-core` | ~700 Rust, ~400 JSON | +1 package `epiphany-pipeline`; +3 schemas |
| 7 | ~307, 16 files | ~30 | −1 npm package, −1 CLI, −1 sibling dep |
| 8 | 0 | ~1,400 | +`epiphany-pipeline` |
| 9 | 0 | ~600 | none |
| 10 | 0 | ~900 | +1 binary; +`cultmesh-rs`, `cultnet-rs`, `signal-hook`, `fs2` |
| 11 | 0 | ~800 | +`reqwest` (daemon only); +1 Qdrant collection |
| 12 | 0 | ~500 | none |
| 13 | 0 | ~600 | +1 binary; +`rmcp`, `tokio` |
| 14 | 0 | ~250 ops | +1 workload, +1 route, +2 brakes, +1 backup path |
| 15 | prose shrinks | — | `cut-map.md` replaced |
| 16 | ~3 C# | 1 sentence | none |

**Epiphany's net change across this campaign is negative**: it loses 1,960 lines
and gains a leaf package it does not itself depend on. The growth is in Huginn,
and it buys the capability the target names. The liability retired lives outside
the line count: prose cut maps, header bookkeeping, relayed reports, transcript
crawls at postmortem, and — new with this rewrite — a per-clone lease, git
preconditions and an unbuilt merge tool.

## Operator questions

Each has a recommendation. Q6 and Q7 are real forks; Q8 is a subtraction the
operator may simply want recorded.

- **Q6. Instance identity: declared, or signed?** Depends on: Cuts 8 and 13, and
  D8.
  - **A. Declared.** `eureka-state` sends its configured instance slug;
    admission refuses a mismatch against the mind's `instance` document.
    Attribution and collision control, not authentication.
  - **B. Signed.** Enrol a per-instance CultNet service identity
    (`enroll_service_identity_at`, already used by Epiphany's permit path) and
    verify the signature at admission.

  **Recommended: A**, with B as a named later cut. On a single-operator LAN
  behind WireGuard and loopback binds, B protects against an attacker who
  already has the machine. It also needs key distribution and rotation to every
  workstation that runs Claude Code, and the target already defers identity
  binding to a later campaign (ruling 9: answering over a chat channel "needs
  identity binding first"). Choosing A means saying out loud, in the README and
  the runbook, that instance identity is not authentication.
- **Q7. How does the workstation reach the organ?** Depends on: Cuts 13 and 14.
  - **A. WireGuard.** `eureka-state` talks RUDP to `10.77.0.1:17872` over the
    existing mesh. No tunnel change.
  - **B. SSH tunnel.** Add a forward to `start-yggdrasil-tunnel.ps1`.

  **Recommended: A.** The organ speaks RUDP over UDP, and
  `runbooks/yggdrasil-ssh-tunnel.md:32-39` already records that SSH local
  forwards cannot carry raw UDP and recommends WireGuard for exactly this case.
  B would require giving the organ a TCP transport it does not need. The cost of
  A is that the workstation must have the mesh up, which it already does for
  other services; the check belongs in `whoami`.
- **Q8. Does `TypedCommitStore` go (Cut 5)?** Depends on: Cut 5 existing at all.
  - **A. Collapse it,** as Cut 5 specifies.
  - **B. Keep it.**

  **Recommended: A.** It was generalised for a second profile that this rewrite
  moves to another repo, so it is now a one-implementation abstraction — a named
  STOP condition. Keeping it costs a parameter on every Mind commit and an
  indirection in the owner. The honest counter-argument, which is why this is a
  question rather than a silent cut: Cut 2 was three commits and a Soul pass, and
  deleting it reads as churn. It is not churn; it is the correct response to the
  ownership change. If the operator prefers B, Cut 5 is dropped and the map says
  why.
- **Q9. Qdrant: share voidbot's, or run Huginn's own?** Depends on: Cuts 11 and
  14.
  - **A. Share.** Huginn connects to `127.0.0.1:6333` and owns the
    `huginn_pipeline_documents` collection.
  - **B. Own.** A second Qdrant under Idunn with its own storage and port.

  **Recommended: A.** Qdrant already runs on Yggdrasil, host-networked on
  loopback, and collections are the isolation unit; ruling 16 says the organ owns
  its own *collections*, which A satisfies. The honest cost: the container is
  started by `voidbot-retrieval.service` and defined in voidbot's compose file
  (R15), so Huginn's index availability is coupled to a unit Idunn does not own,
  and that coupling must be declared as a `[[dependencies]]` entry rather than
  left implicit. B removes the coupling and costs a second Qdrant's memory and
  storage for one small corpus.
- **Q10. May one record be superseded by several?** Depends on: Cut 8's
  admission rules. Cut 6c ships `Superseded { by: Vec<PipelineRef> }` bounded
  at 8, so the shape admits it. **Recommended: yes, each named.** Cardinality
  is an admission rule; the library carries the shape.
- **Q11. Does a resolution of a resolution stay expressible?** Depends on: Cut
  6b, and the resolution matrix.
  - **A. Nesting stays.** The grammar reads it back, bounded by the 64-byte
    local (five or six deep depending on the subject); no arm, no guard, no
    check.
  - **B. A `SubjectKind` type** with twelve variants, so the case stops
    compiling. About +25 lines and one schema file moves.
  - **C. Runtime refusal** in `pipeline_key`, +3 lines.

  **Recommended: A.** B and C move a row of the resolution matrix out of
  admission and into the library, and the matrix is admission's. The
  incoherence was the key shape, and it is gone; deleting the capability is
  right only when the capability is the incoherence. If the operator wants
  nesting refused, admission is the coherent place, beside `instance` and
  `hand_off`. Hands lands 6b under A.
- **Q12. Should `OrgRepo` be tightened?** Depends on: nothing in a cut. Soul
  measured that `a/b.`, `./.` and `a/.b` used to be refused as key segments and
  now key, because `.` is escaped rather than refused. That is injective, so not
  a collision, but `./.` is not a repository. The widening's real location is
  `org_repo_text` (`:141-150`), which imposes no byte class at all; the key
  layer had been acting as its format check by accident.
  - **A. Tighten `org_repo_text`** to refuse an org or repo part that is
    exactly `.` or `..`. About +4 lines, closes it everywhere `OrgRepo` appears,
    mutation M7 killed by `a_repo_part_is_not_a_bare_dot`.
  - **B. GitHub's full rule**, `[A-Za-z0-9._-]`. Not verified from a source, and
    it would refuse repos that key fine today.
  - **C. Leave it.**

  **Recommended: A**, as a four-line follow-up after 6b lands, with no key
  moving. B is a separate small cut with its own evidence if wanted. 6b lands
  with C in place.
- **Q17. Can a resolution be undone?** Depends on: Cut 8's matrix and Cut
  9's in-force derivation. Under the key grammar a subject has one
  resolution key, outcome-invariant, so a withdrawn resolution occupies the
  slot forever and nothing can re-resolve the subject.
  - **A. A subject resolves at most once, and a resolution is not
    resolvable.** The matrix row for `resolution` is "none"; the grammar
    still expresses a resolution of a resolution (Q11 A) and admission
    refuses it. Nothing moves in the leaf.
  - **B. Resolutions carry a sequence in their key** (`…:resolution:question.Q1.2`),
    so a subject can be re-resolved after a withdrawal. A key-grammar change
    in the leaf; free today, since no mind exists yet on Yggdrasil.
  - **C. In-force ignores a withdrawn resolution and admission lets a new
    resolution overwrite it.** Overwrite is the rule this campaign refuses.

  **Recommended: A**, applied as Self's default in the Cut 8 fix batch so
  Hands is not blocked; reversible if the operator wants B.

  **Ruled B, 2026-09-16.** The operator asked what a resolution is (the
  record of how a subject was closed and by what; a question's is
  `Answered { by: ruling }`, not the answer text) and then: "we definitely
  want to keep a log of withdrawn ones, attached to the subject itself."
  So a resolution's key gains a per-subject sequence, withdrawn resolutions
  stay as records under their subject, and the in-force resolution is the
  latest not withdrawn. Self's default was withdrawn from the Cut 8 fix
  batch before it landed. The key change is a leaf cut, mapped with Q18's.

  The operator then checked the model behind the ruling: a subject is like
  a GitHub issue or a Stack Overflow question, one canonical resolution
  with an affordance to withdraw it, so discarding the withdrawn record
  would waste context, "but that's making an assumption that this context
  is even made available to the agents to begin with, and that it is
  important." Self's answer, recorded as two obligations the ruling
  carries: the ruling stands on the campaign's own principle, nothing is
  overwritten and in-force is derived, which Self's default had violated
  for this one kind; and the context reaches agents only if **Cut 9
  exposes a subject's resolution history, not only its in-force state**
  (ruling 2: rehydration and precedent before browsing) and **Cut 11
  indexes withdrawn resolutions with their reasons**. Both are now
  requirements on those cuts. The evidence that reversals matter is this
  map: Q5 ruled A then C, the pause ruled then unruled, a byte-identical
  claim withdrawn.
- **Q18. Can a stewardship be regained after a hand-off?** Depends on: Cut
  12. The derived stewardship keys `<instance>:stewardship:<repo>`, so a
  repo handed away and handed back collides with the withdrawn record.
  - **A. Key stewardship by repo and date**, `<instance>:stewardship:<repo>.<assigned_on>`;
    the in-force stewardship is the latest not withdrawn. A key change in
    the leaf, free today.
  - **B. One stewardship per (instance, repo), ever.** A repo handed away
    cannot come back to the same instance. Simplest; probably wrong for
    the operator's own workflow, where one instance stewards several repos
    over time.

  **Recommended: A**, landed as a small leaf cut before Cut 12, not now.

  **Ruled A, 2026-09-16**, with a framing that changes Cut 12's meaning.
  The operator: "I actually was not planning for such handoffs but I can
  definitely see it being useful for when a campaign has cross-cutting
  concerns and the main steward for it must lease another steward's
  authority over a repo to resolve it." Self first read that as "a hand-off
  is a lease"; the operator corrected it the same day: "A hand-off is not
  *always* a lease. We might start with, say, Odin as steward over a whole
  swarm of infra tools, and spin off a new steward only when the workload
  justifies it. Odin wouldn't be getting it back in that case." So a
  hand-off is a transfer of stewardship, symmetric and final as a record; a
  lease is two transfers and carries no field of its own. Stewardship keys
  by repo and assignment date; the in-force stewardship is the latest not
  withdrawn; a return, when there is one, is an ordinary second hand-off.
  Mapped as a leaf cut with Q17's.
- **Q19. Can a withdrawal be withdrawn?** Raised by Imagination mapping
  Cut 6d. **A. No**: a subject reopened by withdrawal is resolved again at
  the next sequence; chain depth two at admission. **B. Yes**, a withdrawal
  of a withdrawal reinstates. **Recommended and ruled A, 2026-09-16**
  ("Agreed on 19 and 20"); it is what gives "latest not withdrawn" one
  reading.
- **Q20. Stewardship key granularity.** Imagination found that keying
  stewardship by repo and date lets a repo move at most once per day per
  instance, and `Date` is day-granular by design. **A. A per-(instance,
  repo) sequence**, `<instance>:stewardship:<repo>.n<N>`, writer-set and
  checked by admission as previous plus one, exactly as Q17's resolutions;
  `assigned_on` stays a field. **B. Keep the date** and state the limit.
  **Recommended and ruled A, 2026-09-16.** Supersedes Q18's "by repo and
  date" wording; Q18's substance (a repo can come back) stands.

## Target contradictions for Self to reconcile

1. **"Embeddings come from Ollama on Nightwing"** (target, ruling 16). The Body
   disagrees for a Yggdrasil daemon: `epiphany.service` embeds against
   `http://10.77.0.1:11435`, a Yggdrasil-local Ollama, with the same
   `qwen3-embedding:0.6b` model (R16). Only voidbot's indexer uses Nightwing
   `10.77.0.3:11434`. **Recommendation: follow the Epiphany precedent and embed
   locally at `10.77.0.1:11435`,** which removes a cross-host dependency from the
   admission path. The target's sentence should name the endpoint, not the host.
2. **"Huginn is the memory organ ... which also ends the standing authority
   vacancy where doctrine named Huginn the Persona-state steward"** (ruling 17).
   It ends the vacancy only for pipeline state. `F:\Projects\CLAUDE.md` names
   Huginn the runtime steward of `gamecult.persona_state.v0` inspection and
   migration, and this campaign builds none of that; VoidBot keeps that path.
   The doctrine paragraph still needs a correction, and it is not this
   campaign's. Recorded as FU-2.
3. **"Cut 3a code: keep the document kinds, keys, validation and typed
   refusals"** (target, Shape decided). Accurate, but the typed refusals split:
   six of the thirteen landed variants are repo-store refusals and die (D2).
4. **Document set.** The target's End state lists ten kinds. Ruling 14 requires
   three more — `instance`, `stewardship`, `hand_off` — each with a live
   consumer (D2). The target's Documents bullet should name thirteen.
5. **"a typed hand-off for reassigning stewardship, and an import path for
   another instance's mind"** (target). Built in Cut 12, but note it is not a
   merge: two minds cannot diverge, so import is a transfer of named documents,
   not a reconciliation.

## Follow-ups outside this campaign

- **FU-1. Huginn's legacy `.voidbot` Persona state.** `.voidbot/voice/identity.json`
  says the Persona is Huginn; `.voidbot/state/huginn.cc` says the jurisdiction is
  `repo:CultCacheTS` and holds eight legacy `void.*` types and zero
  `gamecult.persona_state.v0` documents. Out of scope by the target. No cut
  touches `.voidbot/`. Owner: whoever runs the portable-Persona migration.
- **FU-2. Doctrine names Huginn the Persona-state steward.** After this campaign
  Huginn is a real runtime, which makes the doctrine paragraph in
  `F:\Projects\CLAUDE.md` more tempting to believe and no more true. Either build
  the Persona path in Huginn or correct the doctrine to name VoidBot. Owner:
  the operator, via a Mind Steward proposal.
- **FU-3. The receipt is implemented twice.** `EpiphanyMindCommitReceipt` in
  Epiphany and `HuginnCommitReceipt` in Huginn share a shape and the digest,
  replay and CAS logic (D3). Epiphany's path is about 260 lines at `dddf9ede`
  (`reasoning_context.rs:543-624`, `:1584-1701`, `:1836-1893`), not the 120
  first written here; Huginn's copy is bounded to one file of at most 220,
  smaller because it drops the authority enum, companions, `invariant_owner`
  and `store_id`. Moving it was priced in Cut 8 (a CultLib change under the
  QUIC campaign, two re-pins, a C# parity question) and declined. This is the
  price of the service boundary. If a third consumer ever appears, extract
  the primitive into CultLib rather than adding a third copy.
- **FU-4. `EpiphanyMindCommitReceipt` naming scar** is now resolved by accident:
  with the pipeline profile gone, the type serves only Mind again. No action.
- **FU-5. The Epiphany map's epoch claim is stale.** `state/map.yaml:76-77` says
  runtime/Mind "v45/v11"; code has `epiphany.runtime_spine.v47`
  (`runtime_spine.rs:58`). Mind Steward's surface.
- **FU-6. `state/map.yaml:261`** carries a long prose summary of this campaign
  that already describes the organ model. It will go stale as the cuts land.
  Mind Steward's surface, at each phase boundary.
- **FU-7. Vendored Eve copies and VoidBot prose still name Huginn as the
  inspector.** Ghostlight's `vendor/eve` submodule (pinned at `672c0c1`) and
  the `Eve-aetheria-authority` worktree carry the fixture with
  `"ownerRepo": "Huginn"` until re-vendored; VoidBot's
  `scripts/export-voidbot-provider-advertisements.mjs:224-233` advertises a
  `huginnInspectionHandoff` for `.cc` inspection that Huginn no longer does.
  No code calls Huginn, so nothing breaks. Owners: whoever next bumps the
  vendored Eve, and VoidBot's Persona doc.
- **FU-8. EveConformance's parity harness cannot reach a report.**
  `tools/parity/parity-manifest.json:250` wants
  `F:\Projects\Aetheria\conformance\eve\aetheria-world-surface.json`, absent
  since the manifest was last changed 2026-07-11. Found by Soul on Cut 7;
  outside every campaign here. Owner: Eve/EveConformance.
- **FU-9. CultCache's owned store signals a held lock by error text only.**
  `Mind::open` matches the message ("already has an active owner") to map
  the failure to `MindAlreadyOwned`; a re-pin that rewords it demotes the
  refusal to `Unavailable` with nothing red. The coherent fix is a typed
  error in `cultcache-rs`. Owner: CultLib, after the QUIC campaign.

---

# History

Everything below is the record of superseded design. **Nothing here is live.**
It is kept because the rulings and probes that produced it explain why the
current design looks the way it does.

## Landed: Cut 1. Re-pin CultLib to `a0813c6`

Landed at `2b76c2e7` (re-pin) and `df82992c` (pin tests). Passed Soul.

Replaced `rev = "e171eca3..."` with `rev = "a0813c6..."` in six manifests,
converted twelve `load_envelope` sites to `put_envelope` (the API was deleted by
CultLib `4ed9871`; every site used a storeless cache, so semantics were
identical), and added `?` at eight `add_generic_backing_store` sites.

- **Verification:** core tests 156/156 with zero warnings; four library packages
  and all nine bins check.
- **Soul found no drift on a live path.** Recorded: **F1 (medium, latent)** —
  under cultcache-rs 0.2.0 a cache with no store accepts `put`/`delete` in memory
  only, where `e171eca3` refused; Epiphany's four read caches have no mutating
  caller today. **F2 (low)** — the pin tests proved "this store file is not
  rewritten", not "no store is attached"; fixed in Cut 2. **F5 (gap)** — typed
  reads of current-epoch stores are unproven because no current-epoch store
  exists locally.

## Landed: Cut 2. One commit owner, two store profiles

Landed at `00991c1b` (F2 fix: storeless reads pinned by directory snapshot),
`46460efc` (the `TypedCommitStore` profile) and `cb6ef5d2` (the fixes). Passed
Soul at 160/160, with every mutation caught including two of Soul's own.

Deleted three hard-coded Mind choices from `commit_authorized_mind_mutation` and
replaced them with a profile: `store_id`, `backing_store`, `open_cache`,
`validate_writes`. The profile owns its backing store, so an opener cannot pick
a different one; the Mind epoch refusal and validation-before-replay are pinned.

**Cut 5 now collapses this.** The second profile it was built for moved to
another repo. Recorded honestly rather than quietly kept.

Open decisions it left for the old Cut 3b, both now moot: replay under a changed
validator, and mapping a lost race between identical commits to `AlreadyAdmitted`.
The organ's receipt (D3) should handle the second case from the start.

## Landed: Cut 3a. Documents, opener, lease, schemas

Landed at `a1473c45` and `ad18c385`, with fixes at `b4f88d29`, `187e01e7` and
`a317d4cf`. Tests 180/180, 27 mutations defined and caught.

**Surviving into the new design:** the ten document kinds, the `value_types!`
single field list, the bound aliases, the format types, key derivation,
`validate_pipeline_write_envelope`, and the ten derived schemas with their
byte-for-byte derivation test.

**Deleted by Cut 4:** `pipeline_store.rs` entire — the opener, the writer lease,
the holder record, `main_work_tree`, the committed-blob attribute checks, branch
binding, and six refusal variants.

**Soul's Cut 3a findings F1-F8** were all fixed before the rewrite. Their
disposition under the new model:

| Finding | Was | Now |
|---|---|---|
| F1 identity-less first commit bricks the store | Fixed by ruling 12 | **Survives** as Cut 8's `first_write_must_carry_identity_and_instance` |
| F2 key collisions through dotted labels | Fixed by ruling 11 | **Survives** into `epiphany-pipeline` (Cut 6) |
| F3 parent ids only prefix-checked | Fixed | **Survives** into `epiphany-pipeline` |
| F4 resolution keys skip label validation | Fixed | **Survives** into `epiphany-pipeline` |
| F5 store writable without lease or admission | Fixed | **Moot.** There is no lease; `admit` is the only write path |
| F6 a worktree takes its own lease | Fixed by ruling 10 | **Moot.** No worktrees, no lease |
| F7 a stale holder is named after a crash | Fixed | **Moot.** No holder record |
| F8 git reads local excludes and global attributes | Fixed | **Moot.** No git reads |

**A later Soul pass was in flight when the operator rejected the ownership
model.** Its findings are referred to elsewhere as S1-S4 and S8. **That
pass's report is not on disk in this workspace** — the only `S`-labelled
artifacts in the scratchpad are Soul's *mutation* labels from the Cut 3a fix
pass (`c3afix-mutations-run.log:15`, mutation S1 against
`parent_ids_are_parsed_strictly` and `resolution_subject_is_a_full_id_of_its_kind`;
mutation S3 against the marker rule). So this map classifies by **layer**
rather than by label, and Self should bind the labels before briefing Hands:

- **Findings against key parsing, identity, bounds, formats or the resolution
  matrix survive**, and their tests move to `epiphany-pipeline` in Cut 6.
- **Findings against the store opener's refusals survive in substance**, and are
  re-tested against redb in Cut 8.
- **Findings against the lease, the holder, `main_work_tree`, the committed-blob
  checks, branch binding, or the merge tool are moot with the layer**, and Cut 4
  is the fix.

If any S-labelled finding does not fall into one of those three buckets, it is
not covered by this map and Self should route it back to Imagination.

**Self's binding (2026-09-16).** The pass's report was in Self's context, not on
disk. Its findings bind as follows:

| Finding | Layer | Disposition |
|---|---|---|
| S1 `main_work_tree` resolves into a foreign work tree (`--separate-git-dir <holder>/.git`) | git resolution | **Moot.** Cut 4 deletes it |
| S2 the parent-toplevel check is unpinned (a bare `exists()` survived the suite) | git resolution | **Moot** with S1 |
| S3 the attribute rule reads the `HEAD` blob, not what git consults | committed-blob checks | **Moot.** No git reads |
| S4 no-commits and detached-HEAD edge cases of that rule | committed-blob checks | **Moot** with S3 |
| S5 the API guard is not a file guard: a scratch crate wrote `pipeline.cc` with a forged record | store file | **Moot as written** (no local store), but its substance survives: the organ must be the only writer of its mind on disk. Cut 8 owns refusing foreign records on open; Cut 14 owns volume ownership and permissions |
| S6 ruling 12 moved Mind write validation after identity-uniqueness and after the store opens | Mind commit path | **Survives.** Untouched by subtraction, and Cut 5's collapse must preserve validation before replay and the current fail-closed order |
| S7 the target and D6 contradicted ruling 10 | docs | **Resolved** by the 2026-09-16 target rewrite and this map |
| S8 a lease held by another user's process reports "unknown holder" | lease | **Moot.** No lease |
| S9 Hands' 27 mutations had no artifacts on disk, so the claim was unverifiable | process | **Standing.** The Eureka Hands brief now requires each mutation to be defined exactly in the report and reproducible from a committed script |

Nothing fell outside the map, so nothing routes back to Imagination.

## Superseded: the repo-owned store design

The following are dead and are not reproduced: the old **D2** (store path
`<repo_root>/.epiphany/pipeline/pipeline.cc`, required `.gitattributes` and
`.gitignore` lines, `repo_root` work-tree checks, branch binding), the old **D3**
(the per-clone session lease in the git common dir, the holder record, who holds
it), the old **D5** (the merge tool and `pipeline-merge` subcommand), and the old
**D6** (sharing across repos by foreign read, `ForeignRef` citation and voidbot
discovery — of which only `ForeignRef` survives, as a field on `ruling` and
`finding`).

Dead cuts: **old Cut 3b** (admission and queries in `epiphany-core` — replaced by
Cuts 8 and 9 in Huginn), **old Cut 3c** (the merge command — replaced by Cut 12's
hand-off), **old Cut 4** (`eureka-state` as an Epiphany package — replaced by
Cut 13 in Huginn), **old Cut 5** (skill wiring against `repo_root` — replaced by
Cut 15), **old Cut 6** (the proof campaign with attribute commits and a lease
check — replaced by Cut 16), and **old Cut 7** (the voidbot semantic projection,
its crawler exclusion, its vendored cultcache-ts replacement and its
`search_pipeline_state` tool — replaced by the organ's own Qdrant collections in
Cut 11, per ruling 16).

Dead rulings: **5** (the store lives in the repo where the task runs), **6** (one
runner per repo plus a merge tool), **10** (one store per clone, in the main
working tree), **12** (the profile owns identity-on-first-write — the *rule*
survives in Cut 8, the *profile* does not), and **Q1, Q3, Q4** (store layout, merge
settlement, voidbot index scope), all of which asked questions about a store that
no longer exists.

Dead operator question **Q2** survives as ruling 8's epoch rule; **Q5** survives
as the `schemars` dependency, now in `epiphany-pipeline`.

- **2026-09-22: Epiphany could not be built from a clean machine.** It pinned Ghostlight `22281891`, whose `vendor/cultcache-py` submodule points at the deleted `GameCult/cultcache-py`. Only local cargo caches hid this. The fix moves the pin to Ghostlight `22937b71`, where that submodule was replaced by the PyPI dependency, and makes three call-site edits in `epiphany-core/src/persona_turn.rs`: `word_budget: 180` (Ghostlight's own `PERSONA_WORD_BUDGET`), plus `output_schema` wrapped in `Some`.
  - Commits: `codex/eureka-pipeline-state` `0bc90b9c`, `main` `4d1113ef`, `codex/epiphany-shakedown-live` `075049b9`, `codex/epiphany-model-bridge` `ed3c7471`.
  - Clean build on Yggdrasil: 244/244 tests pass. Peak 4.36 GiB.
  - **Recorded, not fixed:** `codex/epiphany-model-bridge` has its own pre-existing break. It depends on `path = "../vendor/cultcache-rs"` and `"../vendor/cultnet-rs"`, which do not exist.
- **2026-09-22: the RS fix batch landed** at Huginn `056db28` (Sonnet, verified on Yggdrasil).
  - F1: reads go through `head`.
  - F2: `open_with` is test-only, with a private `open_checked`.
  - F3: R5b and R5c fixtures.
  - F5: prose.
  - Mind 70/70, daemon 18/18. The release check has no `test-support`.
  - The mutation harness was retired mid-batch (operator ruling), so the `.psd1` edits were never committed.
- **Soul on the RS fix, 2026-09-22**, the campaign's first **cargo-mutants** pass (Opus, on Yggdrasil).
  - **RS-2 closes.**
  - cargo-mutants over `92ac16c..056db28` produced 12 mutants: 6 caught, 0 missed, 6 unviable. The tool cannot delete a statement or reorder calls, so Soul's hand probes covered F1 (killed) and F3.
  - **S1, blocking: the R5b and R5c fixtures are degenerate.** Their "foreign" type `epiphany.pipeline.campaign.v1` is a known type at the pinned leaf, so both gate-order mutants pass 70/70. A truly foreign id kills both.
  - **S2:** `Mind::receipts()` is `pub` and skips `head`, an API gap rather than a wire gap.
  - **S3:** `--all-targets` release builds unify `test-support` into the daemon.
  - **S4:** prose.
  - **Closing fixes are in Hands (Sonnet). RS-1 closes when S1 is fixed and proven by hand probe.**
- **2026-09-22: RS-1's closing fixes landed.** Huginn `fbf793f..8c5f784` (Sonnet, on Yggdrasil):
  - S1: the fixtures use `NEVER_A_KNOWN_TYPE`, which `is_known_type` rejects at every epoch.
  - S2: `Mind::receipts()` is `pub(crate)`.
  - S3: `open_with`'s doc states the real release condition. No Idunn deploy recipe for Huginn exists yet.
  - S4: prose.
  - Tests: mind 70/70, daemon 18/18.
  - **Hand probes:** R5b and R5c both survived at `7771c37` and are both killed at `8c5f784`.
  - **RS-2 and RS-1 are closed.**
- **2026-09-22: the committed mutation suites are deleted** (operator ruling, "better nothing than a harness that punishes refactoring").
  - Huginn `7771c37` (2,383 lines).
  - Epiphany `703fdd0b` (1,021 lines, the leaf suites).
  - Tests pin behaviour. Soul runs cargo-mutants on the diff and makes one-off hand probes.
- **Next on this campaign:** BP-2, the widened pin move, waits on selection Cut 1 merging and on BP-1. The RS-L fix (`8473e75d`) gets its Soul pass now.
- **Soul on the RS-L fix, 2026-09-22** (Opus, on Yggdrasil).
  - **Held:**
    - cargo-mutants 27.1: 22/22 caught. Leaf tests: 37.
    - OrgRepo bounds are exact.
    - The listed title classes are refused.
    - Huginn builds against the new leaf. Its tests show 8 expected `Title` compile errors, which are BP-2's load.
  - **RS-L does not close.** Findings:
    - **S1:** DEL (U+007F) passes. The hand-written C0/C1 ranges skip it.
    - **S2:** "visible" is undefined. A title made only of bidi controls, LRM/RLM, U+180E, a lone combining mark, variation selectors, tags or Hangul fillers is accepted. RLO followed by text allows Trojan-source spoofing.
    - **S3:** refusing ZWNJ/ZWJ rejects Persian text and emoji sequences.
    - **S4:** hand probes H01–H10 survive. Unpinned: the owner and repo bounds, a hyphenated repo, U+2028/9, the C1 and C0 edges, U+200C/D, U+2060, and NBSP.
    - **S5:** `a/b.git` is accepted, giving the same repo a second identity.
    - **S6:** GitHub names are case-insensitive. Huginn compares repos by exact string (`admission.rs:505`, `docs.rs:103`), so one repo can hold two stewardships.
    - **S7:** schema prose. `Title` names the private `title_text`, and OrgRepo says maxLength 200 where the true maximum is 140.
    - **S8:** the repo-contains-slash check is dead.
- **Self's rulings on the RS-L second fix, 2026-09-22:**
  - **Title.** A title is valid only if all of these hold:
    - it contains at least one `char::is_alphanumeric()` character;
    - it contains no `char::is_control()` character;
    - it contains no bidi control (U+061C, U+200E/F, U+202A–E, U+2066–9);
    - it is 1–200 bytes.

    **The zero-width denylist is deleted**, so ZWJ and ZWNJ are allowed alongside real content. The rule uses std only, with no Unicode property tables. An emoji-only title is refused. **Flagged for the operator: this refines the accepted F2 rule.**
  - **OrgRepo.**
    - Refuse repo names ending in `.git`.
    - **Identity is ASCII-case-insensitive.** The leaf exposes one canonical key, `OrgRepo::identity()` (ASCII lowercase), and every comparison and key goes through it. Huginn adopts it at BP-2.
    - Double hyphens stay allowed.
  - **S4:** pin every boundary and class with a behavioural test.
  - **S7:** schema descriptions state the rules and the true maxima.
  - **S8:** delete the dead check.
  - **In Hands (Sonnet).**
- **2026-09-22: the RS-L second fix landed** at `998f97e8` and `ec704d64` (Sonnet, on Yggdrasil).
  - The title rule is the three-rule algorithm. The denylists and the whitespace check are deleted.
  - `OrgRepo` refuses a `.git` suffix and gains `identity()`.
  - Nine schemas were regenerated, and the byte-for-byte pin passes.
  - Leaf tests: 37/37. No other crate depends on the leaf, and no fixture in the workspace is refused.
  - Hand probes: every bound probe (H01–H05d) is still killed. The class probes (H06–H10) no longer apply, because the code they mutated is gone.
  - **Stated limit, accepted by Self:** U+3164, the Hangul Filler, is `Lo`, so `is_alphanumeric` accepts it. A title made only of such characters is invisible and valid. Hands pinned it as accepted rather than reintroduce a one-code-point denylist, which is what this rewrite removed. Closing this properly needs Unicode property data (Default_Ignorable_Code_Point), which is a dependency the leaf does not carry. **Follow-up, FU-Title-Ignorable**, if a display surface ever cares.
  - **Recorded, not this cut's:** `epiphany-core`'s `process_observation` test is flaky in the container, with a different test name on each run. It reads `/proc` and is environmental.
  - **Soul's pass on the new rules is dispatched.**
- **Soul on the RS-L second fix, 2026-09-22** (Opus, on Yggdrasil). **RS-L does not close.**
  - **Held:** 37/37 tests twice; the schema pin; cargo-mutants 9/9 caught; all five required hand probes die; the bidi set is exactly Unicode's `Bidi_Control`; the byte bound is exactly 1–200 bytes; the dead check's deletion is sound; `a/b.git.git`, `a/.git` and `a/b..git` refuse while `a/git` and `a/.gitignore` are accepted.
  - **S-1, high: `identity()` is exposed and never adopted.** It has no non-test call site. Key derivation still uses the raw string (`lib.rs:844,853`), so `GameCult/Epiphany` and `gamecult/epiphany` produce two store keys. `bounded_text!` derives `PartialEq`, `Ord` and `Hash` on the raw string, so `==` and every map key compare case-sensitively, and that is the path callers reach for. The doc comment claims the opposite. Two stewardships of one repository can each call the other unstewarded.
  - **S-2, high: the `.git` refusal is case-sensitive.** `a/b.GIT`, `a/.GIT` and `A/B.GIT` are accepted, and their `identity()` is a string the validator refuses. The alias hazard survives through a case variant.
  - **S-3, medium-high: the bidi class is unpinned.** Deleting U+2069, U+2066, U+061C, U+200F, U+200E or all of U+202A–D survives the suite. Only U+202E is pinned. Every other bidi assertion uses a lone control, which the alphanumeric rule already refuses, so it pins nothing. cargo-mutants does not generate class-member deletions.
  - **S-4, medium: the rule widened where it should not have.** `is_control()` is `Cc` only, so U+2028 (`Zl`) and U+2029 (`Zp`) are now accepted inside a title. A one-line field can carry a hard line break. FEFF, U+200B, U+2060 and U+00AD embedded are also accepted now.
  - **S-5, low-medium: the schema lost its caveat.** The description says "1 to 200 UTF-8 bytes" beside `maxLength: 200`, which a validator reads as 200 characters. A 200-character CJK title passes the schema and is refused by admission.
  - **Recorded:** the Hangul-filler limit covers a class, not one code point: U+115F, U+1160 and U+FFA0 behave the same. **FU-Title-Ignorable names the class.**
- **Self's rulings on the RS-L third fix, 2026-09-22:**
  - **S-1: make it structural, not advisory.** `OrgRepo` gets hand-written `PartialEq`, `Eq`, `Hash`, `PartialOrd` and `Ord` over `identity()`, so `==`, `BTreeMap` and `HashMap` are case-insensitive by default and a caller cannot reach the old behaviour by accident. Key derivation (`lib.rs:844,853`) goes through `identity()`. The raw string stays for display only, and the doc says so. Tests: equality, a map with both spellings holding one entry, and one store key.
  - **S-2:** the `.git` refusal is ASCII-case-insensitive, and `identity()` is always itself a valid `OrgRepo`. Test `a/b.GIT`, `a/.GIT` and `A/B.GIT`.
  - **S-3:** pin every bidi code point with an **embedded** fixture, such as `"a\u{2066}b"`, so the alphanumeric rule cannot mask it. All eleven.
  - **S-4:** refuse U+2028 and U+2029 as well as `is_control()`. Everything else stays allowed, so ZWJ and ZWNJ still work. Test both embedded.
  - **S-5:** restore the bytes-versus-characters caveat in the `Title` description.
  - **In Hands (Sonnet).**
- **2026-09-22: the RS-L third fix landed** at `0631cbef` and `466628ac` (Sonnet, on Yggdrasil).
  - **S-1:** `OrgRepo` is hand-written outside `bounded_text!`, with `PartialEq`, `Eq`, `Hash`, `PartialOrd` and `Ord` over `identity()`. Key derivation for Stewardship and HandOff escapes `repo.identity()`. Every other macro member is untouched.
  - **S-2:** the `.git` refusal is case-insensitive.
  - **S-3:** all twelve bidi code points are pinned with embedded fixtures. *Self's ruling said "eleven" and then listed twelve; Hands pinned twelve.*
  - **S-4:** U+2028 and U+2029 are refused.
  - **S-5:** the schema caveat is restored, nine schemas regenerated, and the byte-for-byte pin passes.
  - Tests: 37/37, with assertions added to existing bodies.
  - **Hand probes: 15 of 15 killed**, one per bidi point plus S-1, S-2 and S-4.
  - `composed_keys_cannot_collide` had assumed case-preserving keys and was corrected in the same commit, as part of the same structural change.
  - **Soul's pass dispatched.**
- **Soul on the RS-L third fix, 2026-09-22** (Opus, on Yggdrasil). **RS-L closes on its stated invariants.**
  - **Held, proven rather than asserted:**
    - `Hash`, `Eq` and `Ord` all delegate to `identity()`, so agreement is total for every input.
    - ASCII-only folding is airtight here, since the grammar is ASCII and non-ASCII case variants are refused first (probed with U+0130, U+212A, U+1E9E, U+00DF).
    - Serde round-trips a map with both spellings to one entry.
    - `identity()` is always itself valid, over all 16,384 two-character ASCII values and all 6,561 `.git`-window cases.
    - All twelve bidi points and both separators die under per-point deletion.
    - Nothing outside `is_control()` plus the fourteen named points is refused, across U+0000–U+2FFF.
    - 37 tests, schema pin green, cargo-mutants 17 caught and 3 missed.
  - **Closing fixes, ruled by Self:**
    - **1 and 2:** every `OrgRepo` assertion compares values that should be equal, so `eq -> true`, `hash -> ()` and `cmp -> Equal` all survive. Add the unequal direction: `assert_ne!` on two different repos, different hashes, a non-`Equal` ordering, and `partial_cmp == Some(cmp)` for both equal and unequal pairs. That is S-3's lesson in a new shape.
    - **3:** the `OrgRepo` schema description is about 1,100 characters of Rust-macro justification, published into nine contracts. Cut it to what a consumer needs: the grammar, the bound, and that identity is case-insensitive. The reasoning belongs in the source comment.
    - **4:** `lib.rs:2868` says "eleven" above twelve fixtures. **Self's ruling made the same slip.** Say twelve.
    - **6:** `campaign.repos` accepts both spellings of one repo and validates. Refuse duplicates **by identity**.
  - **7, recorded, not fixed:** `identity()` allocates per comparison. Revisit only if a profile says so.
  - **2026-09-22: the closing fixes landed** at `38bc5381` and `bcfc0d54`. **RS-L is closed.**
    - The unequal direction is pinned, and all five mutants die: `eq -> true`, `hash -> ()`, `cmp -> Equal`, `partial_cmp -> None`, and a raw-string `partial_cmp`.
    - The `OrgRepo` schema description goes from 1,207 to 313 characters, with the reasoning moved to a source comment that schemars never sees.
    - The bidi count comment says twelve.
    - `campaign.repos` refuses duplicates by identity, as `DuplicateRepo { field, repo }`.
    - Tests: 38. The schema pin is green.
    - **Two corrections to Self's ruling:** only **8** schemas reference `OrgRepo`, not nine, confirmed from the test's own diff. The duplicate check needed an optional hook on `value_types!`, because a cross-element invariant cannot be expressed per field; it is documented at the macro and at the call site.
  - **5, answered here so BP-2 does not rediscover it.** Stewardship and hand-off keys change for any repo with an uppercase letter, which would strand documents in an existing store. **BP-2 moves the leaf to epoch v2, and Huginn refuses a store from a previous epoch at open.** So the mismatch cannot happen silently: an old store is refused loudly, which is the designed migration. BP-2 states this rather than writing a key migration.
