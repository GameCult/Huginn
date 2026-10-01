# Eureka substrate: target

Status: target, Self, 2026-10-01. Campaign `eureka-substrate` in Eureka's mind
(instance `eureka`). Its questions, rulings, cut specs, verification and the
substrate gaps themselves are typed documents there. This page keeps the
rationale and the design truths the campaign produces. The cut map's body facts
and model page will live in `eureka-substrate-map.md`.

## Why

The first typed campaign, `idunn-watchdog` (postmortem: Idunn
`docs/watchdog-reporting-postmortem.md`, `6667f0f`), ran its whole pipeline
through Huginn and found the substrate's defects by hitting them. The operator
ruled, 2026-10-01:

> Yep, fixing Mind issues is highest priority, immediately once this campaign
> closes

The defects that cost the most:

- **A finding can be admitted that can never be resolved.** A key's local part
  is bounded at 64 bytes as a whole, and only when it is composed
  (`epiphany-pipeline` `LOCAL_MAX`). A resolution's local embeds its subject's
  local plus `finding.`/`follow_up.` and `.n<seq>`. So a subject whose own key
  fits can have no admissible resolution, under any outcome. Six findings in
  `idunn-watchdog` are stuck in force this way.
- **A shape refusal arrives labelled `Unavailable`.** Huginn's admission maps
  the leaf's key error through `anyhow` into `MindRefusal::Unavailable`. The
  skill tells every agent that `Unavailable` means the organ is down: retry
  once, then stop. A faithful agent would halt a campaign over a key that was
  too long.
- **Substrate gaps have no home every campaign can see.** Follow-ups are
  campaign-rooted, so 20 `gap-*` follow-ups sit under `idunn-watchdog`, and a
  later campaign's recipe cannot see them.
- **Smaller defects:**
  - Revising one spec forced a revision of every spec that named it in
    `depends_on`, twice, when the dependency was on a cut.
  - A short sha and the full sha of one commit are different values to
    admission.
  - A finding cannot be deferred to the cut that fixes it, only to a pointer
    follow-up.
  - The faculty is named Life, but the enum says `MindSteward`.
  - Stewardship is universal (operator, 2026-09-30), yet admission demands a
    per-repo stewardship record that decides nothing.

The code the live mind runs sits on two long-lived branches: Huginn
`eureka/memory-organ` (release `9dd6d80`), and Epiphany
`codex/eureka-pipeline-state`, where the leaf Huginn pins lives. The operator
ruled that they merge to main first.

## Design truths this campaign produces

- **Every admitted subject stays resolvable.** If admission accepts a document
  whose kind can be resolved, every outcome its subject allows can be admitted
  for it, up to a stated sequence bound. A grammar that cannot hold a key
  refuses the subject at admission, not its resolution later.
- **A refusal names what was wrong.** A document that breaks a shape, bound or
  key rule is refused as a typed document refusal. `Unavailable` means only
  that the organ could not serve.
- **Stored documents stay valid.** No change in this campaign re-keys,
  re-encodes or invalidates a document already in the mind. The epoch stays
  `epiphany.pipeline.epoch.v2`. Every change is a widening, an added variant,
  or an admission rule that judges new batches only.
- **One home for substrate gaps.** This campaign is the standing home for gaps
  in the mind's own substrate. The skill's recipe names its root, and every
  campaign reads it.
- **The mind's code comes from main.** The Huginn daemon, the leaf and the
  `eureka-state` client the live mind runs are built from commits on Huginn's
  and Epiphany's main.
- **The live mind survives its own upgrade.** Each deploy upgrades
  `/var/lib/gamecult/huginn/minds/eureka/mind.redb` in place under a release
  symlink that can be flipped back. Every document admitted before the upgrade
  reads back after it.

## Not this campaign

- **Verify-stopgap gaps** (artifact return, `rg` in the image, sibling repos):
  Idunn's verify campaign.
- **CultNet selection** (`gap-citer-faculty-hop`): CultLib.
- **The cold-embed timeout** and **a session-id tool**: recorded follow-ups.
- **Leasing stewardship between agents and `hand_off`**: Huginn Cut 12, parked.
- **Re-encoding stewardship ids into a readable form**, which would re-key
  stored documents.
- **Persona-state reading in Huginn.**
