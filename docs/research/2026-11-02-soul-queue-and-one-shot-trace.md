# Facts: Soul queue, one-shot Persona chain, briefs.md (Eyes, 2026-11-02)

Method: huginn queries (paged, limit 200), git merge-base --is-ancestor against fetched origin/main, ssh read-only to yggdrasil. Nothing changed or admitted. ModelConnector has no local clone; checked via a bare scratch fetch (scratchpad mc.git).

## Part 1a. Reports with no verdict (23 in eureka-body)

Live on Yggdrasil: Huginn release /opt/gamecult/huginn/current -> ffd4136a3 (merge of refusal-loop, 2026-11-02 16:04 UTC); claude-connector current -> 7d4c1bf (= ModelConnector main HEAD). Huginn origin/main is 62a4a7e (docs commit after ffd4136).

### Truly unverified (latest attempt of its cut has no verdict)
| report | repo, head | merged to origin/main? | live? |
|---|---|---|---|
| cut-connector-usage-only.h2 | ModelConnector c1802d8 | yes (ancestor of main 7d4c1bf) | yes (connector 7d4c1bf) |
| cut-source-search-forge-public.h3, .h4 (verdicts exist only on h1, h2) | Huginn 715ec58, 2f36f63 | yes | yes (inside ffd4136) |
| cut-ops-usage-only.h5 (verdict s4 is on h4) | gamecult-ops 6e4dc96 | yes | ops scripts, not a binary; window ran 10-31 |
| cut-persona-doctrine-paragraph.h1, .h2 (no verdict at all) | Eureka 251b673, b125db1 | yes | Eureka doctrine file; installed pin on Yggdrasil not checked |
| cut-persona-doctrine-set.h3 (s2 on h2) | gamecult-ops 6739a9d | yes (merge 1efa5fb) | installer script; whether a Mind's seed holds the new pin not checked |
| cut-turn-core-instruction.h7 (s4 is on h6) | Huginn ffd4136a3 | yes, it IS the live release head | yes |
| cut-tempo-pid-sim.h3 (s2 on h2) | Huginn 99b525632 | NO; branch origin/eureka-body/tempo-pid-sim only | no |
| cut-steward-orders-1.h3 (s2 on h2) | Idunn d9710c9 | NO; origin/hands/steward-orders-1 only | no |
| cut-tell-delta.h1 (head 3e90d138e; verdicts s1 on h2, s2 on h3) | Huginn | NO; branch tip origin/hands/eureka-body-tell-delta is d6a4198, 3 commits past h3's 9202a25 | no |

Note: tell-delta h1 is admitted 2026-11-02 17:11Z but its verdicts are on h2/h3; by "later attempt has a verdict" it sorts under superseded (below). Likewise turn-core-instruction.h7 is later than the last verdict (s4 on h6), so it is the one live and unverified.

### Unverified and not live
cut-tempo-pid-sim.h3, cut-steward-orders-1.h3, and the post-h3 tell-delta tip d6a4198 (three commits with no report). Hands branches only.

### Superseded (a later attempt of the same cut has a verdict)
- cut-leaf-into-huginn.h2 (verdicts s2 on h3, s3 on h4; h4 e516c58 merged)
- cut-mind-self-via-lease.h1 (s1 on h2, s2 on h3, s3 on h4; merged as d54ba01, a squash; h4 head a91fc1c absent locally)
- cut-swarm-bring-up.h1, .h2, .h3 (s1..s3 on h4..h6; squashed as gamecult-ops 41c19bc, "Soul s1-s3"; h6 head 9199224 is an ancestor of the squashed range end 1061676 but squash means no head is an ancestor of main)
- cut-swarm-units-yggdrasil.h1, .h2 and cut-swarm-connector-yggdrasil.h2: no verdict on these cuts' latest attempts (connector has s1 on h1 only; units has none). Their branches sit inside eureka-body/swarm-bring-up and were squashed into 41c19bc whose message names both. Classify as folded into the swarm-bring-up squash, not verdict-cited.
- cut-heartbeat-joint-lever.h1 (s1 on h2 .. s4 on h5; h5 b69d02a merged)
- cut-self-session-live.h1 (s1 on h2 ... s7, s8 on h8; fb197039b merged)
- cut-turn-core-instruction.h3, .h4 (s2 on h2? no: s3 on h5, s4 on h6 later; h6 cce5e1fe7 merged)
- cut-tell-delta.h1 (verdicts on h2, h3)

Weak point: a Falsified later verdict still "supersedes" by the rule used; outcomes of the latest verdicts not re-read per cut.

## Part 1b. Open High/Blocker Soul findings in eureka-body
54 High/Blocker findings; 30 Resolved:Fixed; 24 InForce (no resolution). Code at fetched origin/main (or noted branch):

Defect gone (fixed without a resolution):
1. cut-claude-self-run-lease.s1.lease-reservation-caps-real-spend: ModelConnector main lease.rs module doc: "Nothing is billed and nothing is reserved" (usage-only rewrite).
2-5. cut-heartbeat-joint-lever.s1.failing-winner-starves-beat (Huginn main heartbeat.rs: `while due && let Some(winner) = tempo::pick(&passed...)` with `passed[winner].eligible = false`), s1.standing-vote-expires-by-count (standing_vote queries index voted=true, newest, limit 1), s2.pulse-paging-quadratic (same), s2.withdrawn-launch-walk-unbounded (last_turn is one query on index ran=true).
6. cut-source-search-forge-public.s1.pool-blackhole-stalls: pool.rs admit spawns probe threads (line ~189) with a bounded wait.
7. s1.query-embed-on-cpu: live huginn.service ExecStart gives two GPU --ollama-url primaries plus the CPU as --reserve-ollama-url; queries go through the pool. Not read at source level for the 30 s deadline; SEARCH_DEADLINE is still 30 s (mod.rs:123).
8. cut-swarm-bring-up.s1.aqua-port-held: swarm-bring-up-yggdrasil.sh header and step order put discord-off before huginn-instances.
9. s1.huginn-flip-bypasses-upgrade: install-huginn comment says current is flipped by upgrade-huginn, not there; upgrade-huginn has a gate and flip-back.
10. cut-source-search-forge-public.s2.ops-branch-reverts-self: gamecult-ops main systemd/mind-self@.service has StateDirectory=mind-%i-self and WorkingDirectory=%S/mind-%i-self.
11-14. cut-ops-usage-only.s1.window-rollback-incomplete, s2.window-blocks-not-runnable, s3.rollback-dies-on-real-root, s3.rollback-leaves-connector-unit-new: scripts/usage-only-window.sh on main computes the archive at run time (header), ROOT defaults "" with no `${ROOT:?}`, claude-connector.service is in the archive list (line 41). Only partly read; the first rollback ordering claim (huginn flip barred after a Pulse) not checked. The window is past (10-31).

Defect gone only on an unmerged branch:
15. cut-tempo-pid-sim.s1.vote-tracking-parks-integral: origin/eureka-body/tempo-pid-sim tempo.rs: while voted nothing integrates, a fading `carry_per_h` replaces back-calculating the integral. origin/main tempo.rs has no integral at all (grep count 0), so the controller isn't on main.
16-17. cut-tell-delta.s1.thread-in-a-shown-room-loses-the-exchange and s2.notice-misses-live-holders-brief: the cited functions rooms_shown and run_that_took_it_and_ended no longer exist at the tip d6a4198; tip persona.rs says an earlier line "prints once, in a thread or a room alike" and `runs_that_claim` collects every taker of a brief; commit subjects 3eee12f and b6c6c63. Not verified by Soul on the tip; not merged.
18-19. cut-steward-orders-1.s1.package-install-removes-and-expands (tip has `--no-remove` and `?exact-name(spec)` at lines 260, 2300, 2331) and s1.start-reaches-refused-through-conflicts (Edge::Conflicts at 620-770 with tests at 2535-2583).

Defect still present (cited code at the checked ref):
20. cut-source-search-forge-public.s2.reserve-bounded-like-a-gpu: ollama.rs `pool()` builds reserve endpoints with `OllamaEmbedder::new` = `EmbedBounds::DEFAULT` (floor 20 s + 0.5 s per text); no separate reserve bound on origin/main.
21-22. cut-steward-orders-1.s2.emergency-action-reboots-host and s2.rescue-start-isolates-default-target: tip d9710c9 src/steward_orders.rs has no SuccessAction/FailureAction/StartLimitAction handling and no isolate/sulogin handling (grep empty); "rescue" only in test fixtures. The h3 report (unverified) is the attempt that might address them; branch not merged.

Not determinable:
23. cut-lease-persona-callers.s1.persona-lease-extends-token-reach: PersonaTurn lease still exists on ModelConnector main. Rulings persona-unit-token-reach-bounded (Operator "Yes", 2026-10-31, superseded) and persona-session-landed ("unit may borrow the token, bounded by persona-lease-reach-is-binding") name the Persona holder; the finding's premise (no ruling) predates them. Not further read.
24. cut-minds-web.s1.preview-slot-same-origin-as-chat: gamecult-ops main nginx/chat.gamecult.org.conf lines 64-66 still serve /srv/cinny/preview/ under /preview/ on the chat origin (commit 87cf18a, older than the cut); installer has no "preview" reference on main, so the cut's installer change isn't on main. Open as stated.

## Part 2. How one-shot Personas entered the Body

Chain (authority field from the mind; faculty = admitting faculty):
1. 2026-10-02 11:33Z, question persona-turn-runtime, SelfFaculty (session self-2026-10-01b). Options: connector-request ("one typed request ... One request, one put, one signed document"), claude-code-actuation ("second print-mode session under the Self-run unit shape"), voidbot-machinery. recommended: connector-request. No option was a persistent session.
2. 2026-10-02 18:06Z, rulings self-waker (one-shot-under-timer) and self-run-actuator, both authority=Defaulted, Imagination (imagination-body-prior-art). self-waker says resident-loop was rejected because it "keeps a second owner of the consumption fact the run claim holds"; "each other option breaks a rule in force, so this was not a fork." Cites prior art (OpenClaw heartbeat incidents). self-run-actuator: "the connector never hosts an agent session."
3. 2026-10-02 18:27Z, ruling persona-turn-connector, SelfFaculty, authority=Operator, answers persona-turn-runtime, choice connector-request. operator_quote: "I take all your suggestions." (A blanket reply to Self's batch of recommendations; no words about persistent vs one-shot.) Transcript search for this phrase found no matching session.
4. 2026-10-08 16:14Z, ruling wake-target, SelfFaculty, Operator, quote "wake-target: waker-starts-persona" (the label only). Partly supersedes self-waker; keeps the waker a timer that starts a Persona turn.
5. 2026-10-22 15:45Z, ruling first-self-run-starter, SelfFaculty, Operator, "Chosen on the Fork Desk with no note: wait-for-persona." Ruling text: wake-target stands whole.
6. 2026-10-29 17:41Z, ruling persona-turn-drains-deliveries, Imagination, Defaulted (imagination-swarm-tonight): "mind-persona@<i> stays a one-shot unit (Restart=no ...)"; cut cut-mind-wake.r1-r3 specs "stateless one-shot" waker. Huginn fcaf0a1 (2026-10-29) "mind-persona: the Persona turn organ"; 7db86f7 (2026-10-30) one heartbeat routes every Persona turn. Also persona-reply-typed-by-schema (Imagination, Defaulted, 10-29) and self-run-actuator-2 (SelfFaculty, Defaulted, 10-29).
   `every-wake-a-run` is not a ruling key in eureka-body (absent); the phrase appears only as "the waker's every-wake path" in a Soul finding (cut-mind-launch.s2.persona-step-0-unpinned). git log of docs/eureka-body-map.md shows commits 2026-10-22 and 2026-10-29 (6963b75 "swarm by 2026-10-30 ... typed Persona reply"); not read in full.
7. 2026-10-31 06:11Z, mind-self-is-a-persistent-session, Operator, quote: "My hope was that the Self would behave much like my session with you does. You communicate with me about the stuff I asked you to work on, I tell you what's needed, your context persists until clear or compaction ..." The ruling itself records "Today's shape differs: Persona turns are fresh ... Self runs are one-shot claude processes".
8. 2026-10-31 06:14Z, persona-is-append-only-session, Operator: "The personas should also be such a persistent append only session that occasionally refreshes, otherwise we're wasting cache".
9. 2026-10-31 13:09Z, live-mind-sessions, Operator: "Persona Self is supposed to be a persistent session that compacts and hands off as appropriate, same thing with the Persona that drives it. I'm not getting that feeling with the swarm, here. ... Why is that not the case?" Ruling: the heartbeat "no longer starts one-shot turns."
10. 2026-11-01 10:23Z, persona-session-full-sandbox, Operator: "The design was for the Persona to drive a persistent Self session the same way I would via Claude code ... Both self and Persona sessions are meant to be persistent in the same way".
11. 2026-11-01 20:41Z persona-session-landed (Defaulted, SelfFaculty): "a Persona turn is not a connector request"; supersedes persona-turn-connector.

Evidence a one-shot was asked for by her: none found. The one-shot decisions are Defaulted (Imagination) rulings #2 and #6; the only Operator authority on the point is #3 (a blanket "I take all your suggestions" on Self's recommended connector-request option, whose text describes a single request per turn) and #4/#5 (label-only picks that kept the timer-woken Persona). Evidence she asked for persistent: items 7-10 (2026-10-31 onward), none earlier. Not searched: Fork Desk artifact text for 10-02 and 10-08 (what was shown to her), earlier Huginn persona.rs history before fcaf0a1 (file's first commit), full session transcripts (two keyword searches returned nothing relevant).

## Part 3. briefs.md and cargo-mutants
Yes. /Users/Meta/.claude/skills/eureka/references/briefs.md line 259: "For Rust the tool is `tools/cargo-mutants.sh <repo> <head> <base> [workdir]`, run ONCE per pass, at the end ... rerun only those names (NAMES=...), never a second whole-diff sweep". Line 436 (Hands verification: "never an inline `cargo mutants` command") and line 549 (Soul: runs through the script, "it shards; do not add --jobs"). The handoff (HANDOFF-eureka-self-eureka-2026-11-01c.md item 4) said "Not yet written ... check"; it is written.
