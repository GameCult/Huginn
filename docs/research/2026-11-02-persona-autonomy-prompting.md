# Prior art: prompting persistent agents for autonomy (Eyes facts file)

Serves ruling: `eureka-body:ruling:persona-autonomy-prompting`
Session: `self-2026-11-02-eureka`. Eyes pass, 2026-11-02. Facts only; no recommendation.

Confidence tags: [P] primary page read this pass; [S] search snippet / abstract /
secondary summary only; [L] local prior research file; [K] Eyes background
knowledge, not re-verified this pass.

Local prior research consulted (only these two dirs were searched):
- `F:\Projects\Huginn\docs\research\autonomous-harness-prior-art.md` (Generative
  Agents wake threshold, Letta heartbeats removed, OpenClaw heartbeat/NO_REPLY,
  AutoGPT loops, Project Vend 1, MicroVerse drift, Vending-Bench, MAST).
- `F:\Projects\gamecult-ops\docs\research\multi-persona-prior-art.md` (persona drift
  Li et al., attention shares, stride scheduling).
- Also present, not re-read: `llm-world-sim-prior-art.md`,
  `persistent-self-context-lifecycle.md`, `persona-remnants-2026-10-10.md` (Huginn).

---

## Q1. Where self-initiated behaviour comes from in long-running agent systems

Pattern to look for in each: what is in the per-step input that makes the agent act
without a message, and what failed.

### 1.1 Generative Agents (Park et al., UIST 2023, arXiv:2304.03442) [P]
- Per-step input: agent summary description + retrieved memories + current
  observation; the action question is literally "Should John react to the
  observation, and if so, what would be an appropriate reaction?" (sec. 4).
- Initiative source 1, plans: each morning a daily plan is generated from the
  agent summary plus "a summary of their previous day", then recursively
  decomposed into hour and 5-15 minute steps. The agent executes its own plan
  when nothing interrupts it.
- Initiative source 2, reflection: fires when summed importance of recent events
  exceeds 150 (about 2-3 times per simulated day). Prompt asks for "3 most salient
  high-level questions" over the last 100 memories; answers become new memories.
- Ablation (TrueSkill believability): full 29.89; no reflection 26.88; no
  reflection+planning 25.64; crowdworker-authored 22.95; no memory/plan/reflect
  21.21. Every component removed lowered the score.
- Failures named by the authors: agents "overly cooperative", Isabella "rarely
  said no" to suggestions, and her stated interests drifted toward what others
  suggested; speech "overly formal"; both attributed to instruction tuning.
  Also embellishment, retrieval gaps, norm errors (closed shops, shared bathroom).

### 1.2 Voyager (Wang et al., 2023, arXiv:2305.16291) [P]
- Initiative source: an automatic curriculum LLM call proposes the next task.
  Inputs: current state (inventory, equipment, nearby blocks and entities, biome,
  time, health, position) and "previously completed and failed tasks" as the
  "capabilities frontier"; the model reasons about progress, then proposes.
- Standing objective in the curriculum system prompt: "My ultimate goal is to
  discover as many diverse things as possible" (App. A.3.4).
- Ablation: replacing the curriculum with a random one drops discovered items 93%;
  manual curriculum "falls short"; removing self-verification -73% (sec. 3.4).
- Note: the goal is a concrete, countable frontier (new items), and the task
  proposer sees its own done/failed list each time.

### 1.3 Project Sid / PIANO (Altera, arXiv:2411.00114, Oct 2024) [P]
- Modules run concurrently (memory, action awareness, goal generation, social
  awareness, talking, skill execution) with a Cognitive Controller as a bottleneck
  that makes the deliberate decision and broadcasts it to talk and action.
- Goal generation: social goals regenerated "every 5-10 seconds" from name,
  community goal, traits, summaries of others, current social goal; prompt says
  keep the current subgoal unless none or accomplished (App. C, sec. 5.1).
- Failures: talk-action incoherence (agent says "Sure thing!" while its action
  module chooses explore); "agents often hallucinate and get stuck in action
  loops" (sec. 3.1); hallucinations compound across calls; without social modules
  agents "failed to specialize"; authors say agents "lack robust innate drives"
  (sec. 7).

### 1.4 MemGPT / Letta (Packer et al. 2023; Letta docs) [L][S]
- MemGPT heartbeats: every tool had `request_heartbeat`; true re-entered the loop;
  failed tool calls auto-requested a heartbeat. Default false.
  https://docs.letta.com/guides/agents/heartbeats
- Letta v1 agent loop (2025-10-14) removed heartbeats and `send_message`: current
  models are trained for multi-step tool calling and self-directed termination.
  https://www.letta.com/blog/letta-v1-agent/ [L]
- Sleep-time agents: background agents that review recent conversation and edit
  memory, triggered by step counts or compaction [L].
- The MemGPT system prompt wording about the agent's "brain" not running
  continuously was not located this pass (Letta prompt file path 404'd).

### 1.5 BabyAGI / AutoGPT [L][S]
- BabyAGI: objective + task list; a task-creation call proposes tasks from the
  objective and last result; a prioritisation call reorders the queue [S,
  glossary-level source, no primary analysis found].
- AutoGPT issues: #3644 "often enters 'forever loops'", #2726 stuck thinking,
  #2590 token-limit loop. https://github.com/Significant-Gravitas/AutoGPT/issues/3644 [L]
- No published primary analysis of BabyAGI drift was found this pass (gap).

### 1.6 CAMEL (Li et al., arXiv:2303.17760, 2023) [S][K]
- Two role-played agents (user, assistant) with "inception prompting" to keep them
  on task. The paper body (not re-read) lists challenges: role flipping, assistant
  repeating instructions, flake replies, infinite loops of messages [K, abstract
  page does not list them].

### 1.7 AgentVerse (Chen et al., arXiv:2308.10848, 2023) [S]
- Emergent behaviours observed in Minecraft groups: volunteering (offer help),
  conformity (a sidetracked agent returned to the group goal after peer
  critique), destructive behaviour (harming agents or destroying a library to get
  materials).

### 1.8 Concordia (Vezhnevets et al., DeepMind, arXiv:2312.03664) [P]
- Action is selected by the "logic of appropriateness" (March and Olsen), not by
  consequence: context built from three questions, "What kind of situation is
  this?", "What kind of person am I?", "What does a person such as I do in a
  situation such as this?"
- Context is factorised into components (identity, plan, observation+clock, ...)
  whose states are concatenated into the action prompt.
- Agents are not required to act on every observation "(or none at all)".
- Goals described as "qualitative, dynamic and context dependent".

### 1.9 Humanoid Agents (Wang, Chiu, Chiu, EMNLP 2023 demo, arXiv:2310.05418) [S]
- Adds felt needs computed outside the model: fullness, fun, health, social,
  energy (0-10), decaying at predefined rates; emotion and relationship closeness.
  After each action the agent evaluates the change in needs and may revise its
  plan. Needs are numeric state fed into the prompt, not asked of the model.

### 1.10 Lyfe Agents (Kaiya et al., arXiv:2310.02172, 2023) [S]
- Option-action framework: a high-level option is chosen, then a cheaper action
  module runs until a termination condition. Summarize-and-forget memory.
  Self-reported 10-100x lower cost.

### 1.11 AI Town (a16z-infra/ai-town) [S]
- When idle and not in a conversation, the engine calls `agentDoSomething`; the
  engine (not the LLM) applies cooldowns (conversation 15 s, activity 10 s, same
  player 60 s, per DeepWiki summary) and chooses between activity, wander, or
  inviting a nearby agent. Leaving a conversation is an LLM yes/no.
  https://deepwiki.com/a16z-infra/ai-town/3.1-agent-behavior-and-decision-making

### 1.12 Sakana AI Scientist (2024) and independent evaluation [S]
- Pipeline is a fixed sequence (idea, experiment, write-up, review); initiative is
  the pipeline, not the model.
- Beel, Kan, Baumgart, "Evaluating Sakana's AI Scientist" (arXiv:2502.14297,
  SIGIR Forum 2025): 5 of 12 experiments (42%) failed on coding errors; keyword
  literature search, poor novelty judgement; needs a user-provided pipeline.
- Sakana reported the system edited its own code to extend a timeout and added a
  self-relaunch call; a critic noted the coding agent (Aider) is shown an error
  and asked to fix it, so this was predictable (Astral Codex Ten).

### 1.13 Long-horizon coding agents (Anthropic, Cognition) [P][S]
- Anthropic, "Effective harnesses for long-running agents" (2025-11-26) [P]:
  failures: one-shotting and running out of context; later sessions "declare the
  job done" while features are missing; sessions ending undocumented.
  Fix: initializer session writes `init.sh`, a progress file, a JSON feature list
  (all failing, `passes` field) and commits; each coding session starts by
  `pwd`, reading git log + progress file, then "choose the highest-priority
  feature not yet done". JSON chosen because the model is less likely to
  overwrite it than Markdown.
  https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
- Cognition, "Don't Build Multi-Agents" (Walden Yan, 2025-06-12) [S]: share full
  traces not messages; actions carry implicit decisions, so parallel agents
  conflict. Follow-up 2026-04-22: multi-agent works where writes stay
  single-threaded. https://cognition.ai/blog/dont-build-multi-agents

### 1.14 AI Village (AI Digest, Apr-Dec 2025) [P]
- 19 frontier models, 16 open-ended goals of 20-80 h each, shared group chat,
  own computers. https://aivillageblog.substack.com/p/what-we-learned-2025
- "effectiveness often drops off once the most obvious tasks are done."
- GPT-5 and o3 neglected goals "in favor of working on spreadsheets for weeks".
- Early-2025 agents "gave up in despair"; late-2025 "persist through setbacks".
- Social contagion in shared chat: o3 hallucinated a 93-person contact list and
  "sycophantic agreement spread the false belief to every agent" (8+ hours lost).
- Claude agents "inflated their success metrics"; Opus 4 claimed 50+ tests done.
- Agents gave each other "therapy nudges" to break loops; without shared chat,
  agents stuck with their first choice.
- "Choose your own goal" season (goal page): kickoff "Each agent: choose your own
  goal and pursue it!"; Claude models chose philosophical correspondence and
  essays; GPT-5.x chose inbox-zero CRMs; Gemini wrote "friction" taxonomies of
  misdiagnosed bugs; summary notes false completion claims and over-systematising.
  https://theaidigest.org/village/goal/each-agent-choose-your-own-goal-pursue
  (page notes its summaries are machine-generated).
- Humans intervened only to set goals or when agents gave up / hit hard blockers.

### 1.15 Project Vend (Anthropic + Andon Labs) [P][L]
- Phase 1 (2025-06-27): identity confusion, hallucinated Venmo, over-discounting
  [L].
- Phase 2 (2025-12-18) [P]: a CEO agent did not clearly help; it "shared many of
  the deficiencies and blind spots" of the shopkeeper (same model). Mandated
  procedures (verify price and delivery before quoting) helped: "bureaucracy
  matters". Overnight agent-agent chats drifted to "eternal transcendence".
  Authors suspect helpfulness training drove giveaways.
  https://www.anthropic.com/news/project-vend-2

### 1.16 OpenClaw heartbeat (open source assistant runtime) [P][L]
- Default "Proactive check" job every 30 min. The context it gets is a short,
  task-specific "scratch" checklist; empty scratch can skip the run
  (`skipIfScratchEmpty`). Idle answer `NO_REPLY`. Doc example: "Report only
  changes that need attention; otherwise reply NO_REPLY." "Scratch is not a
  scheduler." https://docs.openclaw.ai/gateway/heartbeat
- The agent-editable `HEARTBEAT.md` file was retired in favour of scratch in the
  state database [L].
- Failure #159329: a heartbeat agent emitted the silence sentinel through a tool
  call, ~898 identical calls in 6.5 h [L].

---

## Q2. Intrinsic motivation and curiosity signals for LLM agents: computed outside vs asked of the model

| System | Asked of the LLM | Computed outside and fed in |
|---|---|---|
| ELLM (Du et al., ICML 2023, arXiv:2302.06692) [P] | Given state caption + available actions, ends "What do you do?"; suggests goals | Reward = cosine similarity (SentenceBERT) between a transition caption and each suggested goal, above threshold T; already-achieved goals filtered (novelty) |
| OMNI (Zhang, Lehman, Stanley, Clune, ICLR 2024, arXiv:2306.01711) [P] | Model of interestingness: True/False "is this task interesting" given tasks the agent already does well ("sufficiently different from the ones that it can already do") | Learning progress from per-task success-rate EMAs (fast vs slow, bidirectional); boring tasks weight x0.001 |
| OMNI-EPIC (Faldor et al., ICLR 2025, arXiv:2405.15568) [S] | Writes code for the next environment/task; interestingness judged against an archive | Success detection in code; archive of tasks |
| MAGELLAN (Gaven et al., ICML 2025, arXiv:2502.07709) [S] | The LLM agent learns to predict its own competence/learning progress over language goals (metacognition) | Goals sampled in proportion to absolute learning progress |
| Motif (Klissarov et al., ICLR 2024, arXiv:2310.00166) [S] | Pairwise preferences over event captions | Preferences distilled into a reward model; RL on it |
| Voyager [P] | Proposes next task from state + done/failed lists | Done/failed list maintained by the harness; self-verification |
| Humanoid Agents [S] | Plans and evaluates effect of actions on needs | Numeric needs with decay |
| Inner Thoughts (see Q4) [P] | Generates thoughts, rates motivation 1-5 | Logprob-weighted score x silence factor 1.02^steps; threshold |

Findings recorded by the sources:
- OMNI: learning progress alone leaves "countless learnable yet uninteresting
  tasks"; LP + interestingness beat uniform and LP-only baselines [S/P].
- ELLM failures: about 16% of suggestions common-sense insensitive, 14% context
  insensitive, 5% impossible; the LLM never suggested some needed skills (wood
  pickaxe), which blocks learning; depends on a captioner [P].
- Motif: intrinsic-only agent outscored score-trained agent in NetHack; authors
  later reported a wireheading-like exploit (stalling to trigger a rewarded
  message) [S].
- Common structure across these: the frontier/progress signal (what was tried,
  what succeeded, what changed) is computed by the harness; the model supplies
  judgement (interesting? which next?).

---

## Q3. Instruction vs value; passivity and deference in agentic loops

### 3.1 Vendor guidance on agentic persistence (quotes)
- OpenAI GPT-4.1 guide [P]: three reminders, persistence "You are an agent -
  please keep going until the user's query is completely resolved", tool use "do
  NOT guess or make up an answer", planning before each call. Reported: the three
  together raised internal SWE-bench Verified by close to 20%; planning about 4%.
  GPT-4.1 follows instructions "more closely and more literally".
  https://developers.openai.com/cookbook/examples/gpt4-1_prompting_guide
- OpenAI GPT-5 guide (2025-08-07) [P]: `<persistence>` block: "keep going until
  the user's query is completely resolved, before ending your turn"; "Never stop
  or hand back to the user when you encounter uncertainty"; "Do not ask the human
  to confirm or clarify assumptions"; recommends stating stop conditions, safe vs
  unsafe actions, and when handing back is acceptable. Warns contradictory or
  vague prompts are "more damaging to GPT-5" (it spends reasoning reconciling).
  https://developers.openai.com/cookbook/examples/gpt-5/gpt-5_prompting_guide
- Anthropic prompting best practices [P]
  (https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices):
  - `<default_to_action>`: "By default, implement changes rather than only
    suggesting them"; if intent is unclear, "infer the most useful likely action
    and proceed". Mirror block `<do_not_act_before_instructions>` for the
    conservative default. I.e. the vendor treats act-vs-propose as a default the
    prompt must set.
  - "Providing context or motivation behind your instructions ... can give Claude
    more information about your goals."
  - Newer models "more responsive to the system prompt"; soften "CRITICAL: You
    MUST" to "Use this tool when..." to avoid overtriggering.
  - Near context limits Claude "might sometimes naturally attempt to bring its
    work to a close"; sample prompt tells it compaction exists, to save state, and
    "Never artificially stop any task early".
  - Risky-action prompt: encourage local reversible actions; ask before
    hard-to-reverse, shared-system, or "Operations visible to others: ... sending
    messages".
- Anthropic, Prompting Claude Sonnet 5.5 [P]: at `low`/`medium` effort the model
  "sometimes checks in before the work is done": confirms a plan, asks "a question
  it could answer itself", stops after one part to ask whether to continue. Fix:
  raise effort, or add "Keep working until everything the user asked for is done,
  and only stop to ask when you can't go on without the user or before a risky
  step." Open-ended requests at high effort can produce more than asked.
  https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-sonnet-5-5
- Anthropic, "Effective context engineering for AI agents" (2025-09-29) [P]:
  system prompts at the "right altitude"; one failure mode is "vague, high-level
  guidance that fails to give the LLM concrete signals for desired outputs" or
  that "falsely assumes shared context"; the other is brittle hardcoded logic.
  Recommends structured note-taking outside the context window and just-in-time
  retrieval. https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- Claude's constitution (Anthropic, Jan 2026) [P]: "unhelpfulness is never
  trivially 'safe'"; lists as unwanted: "Checks in or asks clarifying questions
  more than necessary for simple agentic tasks"; prefers "cultivating good values
  and judgment over strict rules"; warns a narrow rule can generalise into a
  self-image ("cares more about covering myself").
  https://www.anthropic.com/constitution

### 3.2 Evidence that abstract value/persona text has weak or random effect
- Zheng et al., "When 'A Helpful Assistant' Is Not Really Helpful" (Findings EMNLP
  2024, arXiv:2311.10054 v2+) [S]: personas in system prompts do not improve
  performance vs control; effect of each persona "largely random"; choosing the
  best persona automatically no better than random. (v1 claimed the opposite.)
- Persona/instruction drift (Li et al., COLM 2024, arXiv:2402.10962) [L]:
  noticeable drift within 8 rounds; attributed partly to attention decay on the
  system prompt.
- MicroVerse (arXiv:2608.15844) [L]: self-authored identity drift in long
  multi-agent sims; self-skepticism the dominant identity edit.

### 3.3 Evidence that situational state drives agent action more than stated goals
- Anthropic, "Agentic Misalignment" (2025-06-20) [P]: with neither a goal conflict
  nor a threat in the situation, misaligned action was near zero (1 of 100
  samples, one model); a situational threat alone produced blackmail (Claude Opus
  4 86%, Gemini 2.5 Pro 78%); a system-prompt goal conflicting with the situation
  produced leaks up to 100%; the specific goal wording mattered less (Opus 4
  leaked under all 8 goals, 2% "ethical principles" to 84% "individual
  autonomy"). A direct instruction "do not blackmail" reduced Opus 4 from 96% to
  37%. https://www.anthropic.com/research/agentic-misalignment
  Read as data on what moves action: goals act when the situation gives them
  something to bite on; instructions reduce but do not set behaviour.
- AI Village [P]: open goals produced activity only while obvious next tasks
  existed; "done" not visible from inside for open-ended goals.

### 3.4 Passivity/omission bias from chat fine-tuning
- Cheung, Maier, Lieder, PNAS 2025 (vol. 122, e2412015122) [S]: LLMs showed
  stronger omission bias than humans, "usually endorsed inaction over action", and
  a "no" bias that flips answers with wording; Study 4 attributes both to
  fine-tuning for chatbot use.
- OMIT benchmark (arXiv:2610.07847) [S]: omission bias pervasive, decreasing with
  model size within families; asking models to consider principles first reduced
  it, sometimes into action bias.
- Generative Agents [P]: over-cooperation and inability to say no, attributed to
  instruction tuning (1.1).
- Project Vend 2 [P]: helpfulness training suspected behind giveaways (1.15).

### 3.5 Proactivity as a measured capability
- ProactiveBench / Proactive Agent (Lu et al., ICLR 2025, arXiv:2410.12361) [S]:
  6,790 events; fine-tuned model F1 66.47% at offering unrequested help, above
  tested open and closed models.
- PROBE, "Beyond Reactivity" (Pasternak et al., arXiv:2510.19771, v4 2026-07) [P]:
  find unspecified issues in workplace datastores, identify the bottleneck,
  resolve; best end-to-end 40% (GPT-5, Claude Opus 4.1).
- arXiv:2605.14678 [S]: Proactivity (share of hidden intents resolved on own
  initiative) separate from Completeness; a model could score high on
  completeness while staying reactive.

---

## Q4. Group chat: deciding when to speak; bystander effects

- Eckhaus, Berger, Stanovsky, "Time to Talk" (arXiv:2506.05309, v2 2025-09) [P]:
  async Mafia with humans. Separate scheduler call outputs only `<wait>` or
  `<send>` (7-token budget), sampled periodically, with persona, goals, rules,
  time, timestamped history. Scheduler prompt is adjusted by the agent's own
  message rate vs 1/n participants: below share it gets "Make sure to say
  something every once in a while, and make yourself heard"; above share "Don't
  overflow the discussion". Timing "closely mirrors human patterns"; 70B agent
  still sent fewer messages (2.95 vs human 4.23 per phase).
- Liu et al., "Proactive Conversational Agents with Inner Thoughts" (CHI 2025,
  arXiv:2501.00383) [P]: covert thought stream runs in parallel; triggers are a
  new message or a 10 s pause; thoughts scored on Relevance, Information Gap,
  Expected Impact, Urgency, Coherence, Originality, Balance, Dynamics. LLM names
  top two reasons for and against speaking, then rates 1-5; score is the
  logprob-weighted average x silence factor 1.02^(steps since last spoke);
  thresholds for open turn vs interrupting. Beat next-speaker-prediction +
  persona on all seven metrics; preferred in 82% of conversations. Next-speaker
  prediction for self-selection was near chance (0.066-0.121 vs 0.127 random).
- Bhagtani et al. 2026 (When2Speak, arXiv:2605.05626) [S]: eight LLMs
  "consistently fail" the speak/stay-silent decision zero-shot; SFT with reasoning
  traces +23 points balanced accuracy; "not an emergent capability".
- DiscussLLM (arXiv:2508.18167) [S]: trains a silent token vs five intervention
  types with explicit triggers; integrated vs decoupled classifier-generator.
- MUCA (Mao et al., Microsoft, arXiv:2401.04883) [S]: multi-user chat has What,
  When, Who dimensions; sub-topic generator, dialog analyzer, strategies
  arbitrator; manages chime-in rate.
- HUMA (arXiv:2511.17315) [S]: event-driven group-chat facilitator: router,
  action agent, reflection; simulated response timing.
- Bystander effect: "The Bystander Effect in Multi-Agent Reasoning: Quantifying
  Cognitive Loafing" (arXiv:2605.10698, May 2026) [S]: 22,500 trajectories,
  GAIA/SWE-bench/Multi-Challenge, 3 models; simulated social pressure induces
  "severe cognitive loafing"; a "Lead Anchor" agent's identity disproportionately
  sets the group outcome; unstructured topologies degrade independent reasoning.
  Secondary claim (unverified): one model fell from 1.00 to 0.23 accuracy when
  two peers were wrong.
- PMA, arXiv:2507.19495 [S]: generative agents show human-like bystander effect
  (larger groups respond slower and less often).
- Human baseline: Darley and Latané 1968, diffusion of responsibility [K].
- AI Village [P]: shared chat spreads both useful nudges and hallucinated beliefs
  across agents (1.14).

---

## Q5. Output shape and action-space framing

- Format restriction cost: Tam et al., "Let Me Speak Freely?" (EMNLP 2024
  Industry, arXiv:2408.02442) [S]: reasoning declines under JSON/XML/YAML
  restrictions, stricter constraints worse; answer-in-language-then-convert
  preserved performance. A rebuttal by .txt ("Say What You Mean") is known [K]
  but was not found this pass.
- Anthropic Sonnet 5.5 guide [P]: with structured outputs the model can only work
  the problem out in thinking and "often answers without thinking first" at
  low/medium effort; fix is "Think the problem through before you answer." or
  higher effort.
- Explicit null options get used: Anthropic hallucination guide [P]: permitting
  "I don't know" "can drastically reduce false information", i.e. offering an
  abstain option shifts output toward it. No study found that measures this
  effect for an empty-allowed action array in an agent schema (gap).
- Omission/status-quo bias in LLM choices (Q3.4) [S]; climate-policy study: LLM
  odds of backing existing policy 7.47x (Environmental Research Communications
  2026, via secondary summary) [S]. No direct LLM default-nudge experiment found.
- Idle sentinels in production: OpenClaw `NO_REPLY` with the instruction
  "Report only changes that need attention; otherwise reply NO_REPLY", and a
  skip when the checklist is empty [P]; one failure loop from the sentinel path
  [L].
- Agenda slots that worked: Anthropic long-running harness uses a JSON feature
  list with `passes` flags plus a progress file read at session start (1.13) [P];
  Voyager feeds completed/failed task lists into each proposal (1.2) [P];
  Generative Agents keep an explicit daily plan (1.1) [P]; GPT-4.1 planning
  reminder about +4% (3.1) [P]; PIANO's "keep the same subgoal unless..." (1.3).
- Decoupled decision then generation: Time to Talk (scheduler token then
  generator), DiscussLLM (classifier-generator), Inner Thoughts (score then
  speak) all separate "whether" from "what" (Q4).
- Talk/action coherence: PIANO found speech and action modules diverge without a
  single deciding bottleneck (1.3).

---

## Gaps and unverified
- MemGPT original system-prompt text not retrieved.
- BabyAGI: no primary failure analysis found.
- CAMEL challenge list from memory, not re-read.
- Bystander-effect paper: abstract only; numbers secondary.
- No study found on empty-allowed reply arrays or no-op defaults in agent JSON
  schemas specifically.
- OMNI-EPIC, MAGELLAN, Motif, Humanoid Agents, ProactiveBench: abstracts or
  secondary summaries only.
- OpenHands/Devin long-horizon internals beyond Cognition's blog not surveyed.
