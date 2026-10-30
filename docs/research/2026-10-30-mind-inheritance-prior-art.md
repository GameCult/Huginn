# Mind inheritance from Eureka: fork, distill, or a shared mind (2026-10-30)

Serves question `eureka-body:question:mind-inheritance-from-eureka` under ruling `eureka-body:ruling:persona-workshop-from-lore-and-journeys`; precedent `eureka-body:ruling:foreign-mind-ref-instance` (shared store + pokes, 2026-10-10) and follow-up `atlas-shared-store-and-pokes`. Eyes facts, no recommendation.

Facts with pointers. No recommendation. "Not measured" marks what I could not reach.

## 1. What the Eureka mind holds

whoami (instance eureka, rudp://10.77.0.1:17872): 12,967 documents, 4,477 receipts, schema_epoch epiphany.pipeline.epoch.v2, persona Stored 2026-10-29T19:20Z.

Counts by kind (eureka-state query, `matched` per schema, as_of receipt 4477):
resolution 4,226 | finding 2,818 | ruling 1,222 | follow_up 1,111 | cut_report 1,100 | verdict 948 (derived: first attempt timed out at 15 s, retry matched 948) | cut_spec 837 | question 583 | target 79 | campaign 35 | instance 1 (stewardship 6, hand_off 0, run 1, plus 9 instance-rooted docs under root `eureka`). Sum of the kinds = 12,967 (within rounding of my verdict retry).
Everything is one wire family `epiphany.pipeline.<kind>.v2` (names to be renamed `eureka.pipeline.<kind>.v3` before the first hosted mind; eureka-body-map.md:505).

Per-campaign document counts (query with `fields: [{index:"root", op:"any_of", values:[...]}]`, limit 1, read `matched`). Only these roots were counted (35 queries was over budget):
- aetheria-release 3,746
- aetheria-ew 1,273 (grouped query with living-world, arena, fauna, gravity returned 1,273 total because the first four slugs matched; treat as a floor for the five Aetheria design campaigns)
- eureka-body 1,189
- web-customize 998 (grouped with rps, motion-pipeline, lodur: the grouped query returned web-customize's campaign doc; the 998 is the sum of that group, not web-customize alone)
- variants 902
- cultlib-gaps 857
- eureka-substrate 619
- ghostlight-verse 572
- gamecult-chat 357
- thing2 116
- root `eureka` (instance, stewardship, run, their resolutions) 9
Not counted: idunn-watchdog, thing, thing-family/providers/verse, site-masthead, asura, cultmath-tapes, concept-mill-votes, brush-tree, persona-poses, brokkr-msgpack, aetheria-gravity/fauna/arena/living-world individually, idunn-reincarnation, idunn-deploy-provenance, joystick, cultmesh-ts-content, cultmesh-garden-path, weksa-lowering, lodur, rps, motion-pipeline.
Caveat: the sum of the counted roots (~10.6k) is dominated by Aetheria (release + ew group) and eureka-body; the long tail is uncounted.

35 campaigns, with their `repos` field (query kind campaign):
- Eureka-steward repos (Huginn, Eureka, gamecult-ops, CodexConnector, Epiphany, Bifrost, Idunn): idunn-watchdog (Idunn, Bifrost, ops), eureka-substrate (Huginn, Epiphany, Eureka, ops), eureka-body (Huginn, Eureka, ops, CodexConnector, Epiphany, Bifrost), idunn-reincarnation (Idunn, ops), idunn-deploy-provenance (Idunn, ops), persona-poses (site, ops, Huginn, Bifrost), thing2 (Eve, ops).
- Heimdall/Bifrost: concept-mill-votes (Bifrost, ops, AetheriaLore), joystick (Heimdall, StreamPixels, ops), gamecult-chat (ops, Bifrost, Heimdall, Idunn, gamecult-chat).
- Kiko (StreamPixels, Vili, Mimir, Muninn): web-customize, rps, motion-pipeline (Vili, StreamPixels).
- Libby (CultLib and listed repos): cultlib-gaps, variants (CultLib, Aetheria), cultmath-tapes (CultLib, site), cultmesh-ts-content, cultmesh-garden-path, brokkr-msgpack (Brokkr, CultLib), brush-tree (Csg, Brokkr, Aetheria), thing-family (Thing* repos, CultLib, Ymir).
- Nibu (Aetheria, AetheriaLore, Asura, Lodur, BadDaySimulator): aetheria-release, aetheria-ew, aetheria-fauna, aetheria-gravity, aetheria-arena, aetheria-living-world, asura (Asura, CultLib, Aetheria), lodur (Lodur, BadDaySimulator).
- Weksa: weksa-lowering (Weksa); ghostlight-verse (Ghostlight, AetheriaLore, ops).
- Void: site-masthead (gamecult-site).
- Multi-steward: thing-providers (StreamPixels, Vili, Stonks, Weksa, repixelizer, Sai, Loki, AquaSynth) spans kiko/libby/void/weksa/aqua; thing-verse (Odin, Ghostlight, Heimdall, Muninn, Mimir, Gjallar, Hermodr, Bifrost) spans eureka/weksa/heimdall/kiko/archive.
Campaign-to-steward mapping above is my join of the campaign `repos` lists to stewardship-final.md (steward column); it is not a stored relation.

Stewardship in the mind: 6 `stewardship` documents, all instance eureka, keys `eureka:stewardship:gamecult_-<repo>.n1`: Bifrost, gamecult-ops, Idunn (2026-09-30), Epiphany, Eureka, Huginn (2026-10-01). The 66-repo steward table (stewardship-final.md, 9 eureka / 7 nibu / 7 void / 7 weksa / 6 libby / 5 kiko / 4 aqua / 3 heimdall / 18 archive) is operator-confirmed on 2026-10-29 but is not in the mind as stewardship documents.

Cross-cutting content every Mind would meet (eureka-state ruling titles, label in parentheses):
- eureka-body: persona-per-mind, persona-workshop-from-lore-and-journeys (2026-10-30, the ruling quoted in the brief), foreign-mind-ref-instance, swarm-governance-in-bifrost, swarm-decides-without-operator, governance-estate-blocks, brake-anyone-pulls-motion-releases, swarm-shares-log-bound, swarm-lead-steward-router, swarm-membership-operator-rules, self-run-result-to-classifier, always-open-source, she-owns-her-body, self-runs-through-connector.
- eureka-substrate: operator-means-shape-ends, authority-thresholds, mind-rules-which-forks (cited in swarm-revival-target.md).
- Process/charter law is mostly outside the mind: CLAUDE.md, ~/.claude/agents/*.md (Eureka repo), ~/.claude/doctrine; and a prose layer under prose-knowledge-target.md (forks, rulings still in prose).
Eureka-only content: eureka-body Huginn body-map cuts (connector, claude-code backend, self-waker, run claims, repetition breaker), idunn-watchdog/reincarnation, eureka-substrate gaps (docs/eureka-body-map.md, eureka-substrate-map.md).
Steward-owned content (by campaign repos): the Aetheria, StreamPixels, CultLib, Ghostlight, Weksa campaigns listed above, each with its own rulings, cut specs, reports, findings.
Counts for "commons vs steward-owned vs Eureka-only" as documents: not measured (needs paging 12,967 ids; the daemon CLI `huginn` is not built locally, only the MCP binary exists at F:\Projects\Huginn\target\release\eureka-state.exe).

Eureka's Persona (persona_get, personaId eureka, updated 2026-10-29T19:20Z): publicName Eureka, she/her, jurisdiction = the eureka mind and coordination of the swarm in Matrix, home context matrix #gamecult space (#swarm, #burn, #operator); voiceSummary (dry, direct, cult vocabulary turned toward revealing); affect.socialBonds for operator, weksa, libby, heimdall, kiko, aqua, nibu, void (each "shortlisted cast Mind with Matrix account, no mind or mouth yet"); agencyPressure: operator-move, swarm-bring-up (wanted running 2026-10-30), burn-visible; thoughtMemory memories epiphany-absorbed, one-persona-per-mind; six values (coherence over velocity, nothing grades itself, reveal-not-obscure, visible consent, open source, community cannot betray); activationProfile empty; privateNotes (rehydrate from typed state, appearance canon, operator rapport). Provenance authority canonical, source system huginn.
Eureka-specific fields: personaId, jurisdiction, voice, bonds, pressures, memories; the mind holds who she is in the Persona document and project truth in typed pipeline state (privateNotes says so).

## 2. Huginn mechanics for moving knowledge

Source: F:\Projects\Huginn at 096b142 (local; GitHub main 7db86f7 was not fetched).

Identity rules:
- One Huginn daemon per mind: `serve.rs` takes exactly one `--instance`; store opens fail-closed; Qdrant collection per instance (`huginn_mind_<i>`) (docs/swarm-revival-target.md "Body").
- `Mind::open` (crates/huginn-mind/src/lib.rs:1-10) refuses a foreign type, foreign epoch, missing identity or foreign identity; takes the per-path lock; redb lock held for the daemon's lifetime (gamecult-ops/runbooks/huginn-yggdrasil.md:16-17).
- Admission A5 (`refuse_foreign_instance`, crates/huginn-mind/src/admission.rs:310-320): instance/stewardship/run documents must name this mind; a hand_off must name it on one side.
- Keys embed identity: instance-rooted keys `<instance>:instance:self`, `<instance>:stewardship:<repo>.n<k>`, `<instance>:run:<label>`, `<to>:hand_off:<from>.<repo>.<date>` (lib.rs:1847-1862, 2194-2246). Campaign-rooted keys (`eureka-body:ruling:...`) are rooted in the campaign slug, not the instance; the instance appears inside the document body only for the instance-rooted kinds.
- Receipts: every batch has a receipt (`mind-commit-...`) naming the bytes read and written; 4,477 receipts in the eureka store, each carrying provenance (agent, faculty, session, tool). A copied store carries all receipts and the eureka-session provenance.
- `ForeignRef` today is `{repo, commit, kind, id, payload_sha256}` (lib.rs:591), an address in a git pipeline store. Ruling `eureka-body:ruling:foreign-mind-ref-instance` (operator, 2026-10-10) chose a mind-addressed variant `{instance, kind, id, payload_sha256}`, extended: minds get "a shared store and the ability to poke each other within a turn", the earlier design called Atlas; follow-up `eureka-body:follow_up:atlas-shared-store-and-pokes` ("owner: a Huginn instance of its own?") is open; it "needs the second mind and mind-addressed citation first". At local HEAD ForeignRef is still the repo form (not landed). Precedents on rulings/findings are `Vec<ForeignRef>[8]` (lib.rs:649, 669).
- Swarm target says a citation of another mind's ruling is "precedent, never binding"; cited documents are immutable (resolutions are new documents) so a digest pin stays valid (swarm-revival-target.md identity table, "Cross-mind reference").

hand_off and stewardship:
- `PipelineHandOff { from_instance, to_instance, repo, documents: Vec<Short>[256], reason, handed_on }`; documents are id strings, not payloads (lib.rs:700-703). Max 256.
- Admission: on the source side each document id must parse as a kind id (admission.rs:339-350, `references`); "the receiving side imports them afterwards (Cut 12)". Cut 12 (hand-off and leasing) is parked (eureka-substrate-target.md:81). No receiving-side import code exists. So today documents are listed (referenced by id), not copied.
- Source side hand_off withdraws the giver's stewardship via a derived resolution (admission.rs:411-420) and requires the giver to hold stewardship (admission.rs:606-608, `NotStewarded`). Admitted "in both minds" (swarm-revival-target.md); hand_off count in the eureka mind: 0 (no precedent of use).
- Stewardship admission rule `RepoNotStewarded` was dropped; stored stewardship docs "decide nothing until Cut 12" (eureka-substrate-map.md:211).
- Campaign repos list: CutSpec admission requires its repo to be in the campaign's repos in force (admission.rs, `RepoNotInCampaign`).

Raw fork of the store:
- Store: /var/lib/gamecult/huginn/minds/eureka/mind.redb on Yggdrasil (huginn-yggdrasil.md:16; runbook step 4 describes cold copy with the unit stopped). Size: not measured (ssh to Yggdrasil returned Permission denied on that directory from this account). Derived Qdrant collection is rebuildable.
- A copy is a valid store only for instance `eureka`: identity doc `eureka:instance:self` and the A5 rule mean a new instance name needs the identity document changed and every `eureka:*` instance-rooted key rewritten, plus receipts that name the old instance. No migration or "fork" tool exists in the source (grep: none). The runbook's cold copy exists for backup/upgrade (gamecult-state-backup.md:47).
- Doctrine: "The identity is in the state, not the path" (swarm-revival-target.md, ruling 14).
- Mind migrations are the operator's (live-mind upgrade; eureka-body-map.md:505). The wire rename (v2 to v3) is a pending mind.redb migration.
- Documents with campaign roots are not bound to the instance, so selecting a subset by root and re-admitting them into another mind through `admit` is the only supported write path (admit is batch of 1-64 docs; huginn CLI `admit` / `schema`). Cross-reference checks: admission refuses a cut_spec whose campaign is missing, a ruling citing a missing question, etc., so a subset must be admitted in dependency order and closed under citation. Resolutions and findings cite specs and verdicts.

## 3. Third shape: a shared (commons) mind

What the repo already says (swarm-revival-target.md; ruling 2026-10-10):
- Shared store between minds and in-turn pokes was ruled (foreign-mind-ref-instance); owner unresolved ("a Huginn instance of its own?" in the follow-up); not mapped.
- Today minds share nothing but wallet and host: "A swarm here is several minds that share one wallet and one host... cooperate by citation and hand-off." The swarm has no record of its own ("Swarm... has no record of its own").
- Daemon: one process per instance, templated `huginn@<i>`; a commons would be one more instance (`huginn@commons`, own redb, own Qdrant collection, own endpoint, own control store dir `/etc/gamecult/minds/<i>/`). Cost per instance is one small Rust binary (Qdrant/Ollama shared).
- Reading two minds: eureka-state MCP server is configured by one `EUREKA_INSTANCE` + `HUGINN_ENDPOINT` (eureka-state config.rs); `HuginnClient` is one instance, "the request is built from that instance, never from anything the caller sent". Two minds therefore means two clients/servers, or a new client. No cross-mind query exists in source. Mind-addressed ForeignRef (cited by id with payload digest) would be read "from the other mind on demand" (question foreign-mind-ref option text).
- Persona turns and Self runs: a Self run is launched per mind (`mind-self@<i>:<label>`, user `mind-<i>-self`); a run claims items in its own mind (`PipelineRun.claims: Vec<PipelineRef>`).
- Writers: authority rules in the swarm target: nobody writes another mind's Persona or record; operator rulings outrank all; one mind's ruling never binds another; "Shared-substrate repos are stewarded by the lead" (Eureka, ruling swarm-lead-steward-router); governance by estates (Minds, Members, Users, Patrons; ruling governance-estate-blocks) in Bifrost (swarm-governance-in-bifrost); operator must not be required for a decision (swarm-decides-without-operator); anyone can pull the brake (brake-anyone-pulls-motion-releases).
- Commons candidates in the mind today (by content): the swarm-governance/charter rulings in eureka-body; operator rulings in eureka-substrate (authority-thresholds, operator-means-shape-ends, mind-rules-which-forks); always-open-source; persona-per-mind. Eureka-only: body-map cuts and Huginn/Idunn campaigns. Steward-owned: the repo campaigns. Document counts per class: not measured.

## 4. Prior art (web, 2026-10-30)

Forks:
- Spanish Wikipedia fork (Feb 2002): Enciclopedia Libre drew most active contributors and ~10,000 articles by end of 2002; Spanish Wikipedia stalled in 2002, then recovered with newer users by June 2003 and overtook the fork in late 2004; Oct 2002 reunification vote was rejected; fork reported ceased operations by late 2024 (one source). Participants' stated cause: governance dispute (advertising/censorship), not technical (https://en.wikipedia.org/wiki/Enciclopedia_Libre_Universal_en_Espa%C3%B1ol, https://networkcultures.org/cpov/2011/01/15/spanish_fork/, https://en.wikipedia.org/wiki/Spanish_Wikipedia).
- In-repo precedent for divergence cost: ruling persona-per-mind's own rationale ("a shared record lets the loudest open items capture every Persona's attention"); swarm-revival-target.md cites Raft (leader orders log, cannot rewrite follower state), Debian DPL/TC, Holacracy lead link, and MAST failure attribution for manager-over-worker agent frameworks.
Multi-agent memory:
- Collaborative Memory (Rezazadeh et al., ICML 2025): private tier + shared tier per agent, read policies, immutable provenance on each fragment for retrospective permission checks (https://arxiv.org/abs/2505.18279v1).
- Survey "Memory for Autonomous LLM Agents" (arXiv 2603.07670): all-shared simple but leaks; all-private isolated but no transfer; proposes role-based access over a shared substrate; hybrid designs let a policy route writes to private or shared space (https://arxiv.org/pdf/2603.07670); a 2026 position paper names multi-agent memory consistency the most pressing open challenge (as summarised by the search result; not opened).
- AIM (arXiv 2609.12320) private/public classification with index-level enforcement; PiSAs (arXiv 2607.05318) benchmarks privacy leakage across private, shared, hybrid memory. Both found by search only; dates inferred from IDs, not opened.
- Sequential-writer pattern (all writes through one coordinator) is described in practitioner guides as simple but a bottleneck (https://greennode.ai/blog/how-to-design-shared-memory-for-multi-agent-ai, vendor blog, weaker source).
- Blackboard lineage: Hayes-Roth 1985 (as cited by https://learn.traeai.com/t/ai-engineering/phases/16-multi-agent-and-swarms/13-shared-memory-blackboard; secondary source).
Not searched: org spin-off literature beyond the Wikipedia example; Wikipedia commons governance (Wikimedia Commons / Wikidata).

## Gaps for the decider
- Store size and per-class counts (commons / steward / Eureka-only) not measured; needs a paged read of all 12,967 headers or a built `huginn` CLI.
- Question `mind-inheritance-from-eureka` is named in ruling persona-workshop-from-lore-and-journeys; `view eureka-body:question:mind-inheritance-from-eureka` returned null, i.e. not admitted under that id (or admitted under another id).
- No code exists for: fork-by-copy to a new instance name, receiving-side hand_off import (Cut 12), mind-addressed ForeignRef (not landed), cross-instance client.
