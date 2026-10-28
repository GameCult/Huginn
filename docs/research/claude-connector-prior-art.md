# Claude connector: prior art and probes

Serves: map line B4 `claude-connector` (GameCult/ModelConnector, ruling
`eureka-body:ruling:persona-first-connector-claude`), its cut spec
`eureka-body:cut_spec:cut-claude-backend.r1` (B4a), question
`eureka-body:question:claude-connector-credential`, and follow-ups
`eureka-body:follow_up:claude-tool-exchange-boundary` (section 4),
`claude-byte-bound-live-check` (section 5). Builds on Huginn `725f5a5`
`docs/research/connector-contract-v3-prior-art.md` (sections 1a, 2d, 2e), which is not repeated.

Fetched 2026-10-28 by Imagination (`imagination-claude-connector`, session
`self-2026-10-28-eureka`) from current primary docs; section 8 is probes on Yggdrasil. UNVERIFIED
marks what the docs did not say. No Anthropic API call was made with any credential.

## 1. Which credential a third-party program may use

Source: https://code.claude.com/docs/en/legal-and-compliance ("Authentication and credential use").

- OAuth "is intended exclusively for purchasers of Claude Free, Pro, Max, Team, and Enterprise
  subscription plans and is designed to support ordinary use of Claude Code and other native
  Anthropic applications."
- Developers building products or services, the Agent SDK included, "should use API key
  authentication through Claude Console or a supported cloud provider". Anthropic does not permit
  third parties to route requests through Free, Pro or Max plan credentials, and developers may not
  collect, store or intermediate Claude.ai credentials or session tokens.
- The restriction does not limit how a customer provisions its own API keys, for example in a
  secrets manager or machine image for its own authorized users, provided the usage is billed to the
  key owner and not resold or intermediated.
- An end user may still sign in to the unmodified Claude Code binary with their own subscription.
  "Advertised usage limits for Pro and Max plans assume ordinary, individual usage of Claude Code
  and the Agent SDK." Anthropic "may [enforce] without prior notice".

Reading: a connector daemon that calls the Messages API with the operator's subscription OAuth token
is the routing the page forbids. Driving the unmodified `claude` binary with her subscription is the
permitted Claude Code path (the Self-run unit, ruling `self-run-token-reach`), but it is Claude
Code's harness, not a Messages request the connector lowers (section 6).

## 2. What an API key can be capped by, provider-side

Source: https://platform.claude.com/docs/en/api/rate-limits, https://platform.claude.com/docs/en/api/errors.

- Two limit kinds: monthly spend limits and rate limits (RPM, ITPM, OTPM per model class), enforced
  at the organization, with user-configurable limits per workspace.
- "You can't set spend limits on the default Workspace." Workspace limits are set per limiter type;
  organization limits always apply too. The `anthropic-workspace-id` response header names the
  workspace a key resolved to.
- Tier monthly spend caps: Start $500, Build $1,000, Scale $200,000. Reaching the tier cap returns
  429 `rate_limit_error` with `error.details.error_code: "enforced_spend_limit_reached"` and no
  `retry-after`; retries fail until the next month.
- A spend limit you set returns 400 `invalid_request_error`, message starting "You have reached your
  specified API usage limits" (or "...specified workspace API usage limits").
- Error types: 400 invalid_request_error, 401 authentication_error, 402 billing_error,
  403 permission_error, 404, 409, 413 request_too_large (32 MB Messages limit, Cloudflare answers
  before the API), 429 rate_limit_error, 500 api_error, 504 timeout_error, 529 overloaded_error.
  Every response carries a `request-id` header. The docs do not say whether an errored request is
  billed: UNVERIFIED.
- Rate limits are a token bucket; only uncached input (`input_tokens` + `cache_creation_input_tokens`)
  counts toward ITPM on current models; `max_tokens` does not count toward OTPM. Headers
  `anthropic-ratelimit-{requests,tokens,input-tokens,output-tokens}-{limit,remaining,reset}` and
  `retry-after` (not on every 429).
- Long requests: use streaming for long-running requests; the SDK refuses non-streaming requests
  expected to exceed 10 minutes; idle connections may be dropped by networks.

## 3. How proxies hold provider credentials

- LiteLLM proxy (https://docs.litellm.ai/docs/proxy/virtual_keys): callers hold LiteLLM virtual
  keys (`/key/generate`), each limited to a model list and carrying budget and rate limits; the
  provider key sits in the proxy's `config.yaml` `litellm_params.api_key` or an `os.environ/`
  reference. The page does not say callers can ever see provider keys, and does not describe
  at-rest encryption of provider keys (UNVERIFIED here; prior pass `6b496eb`
  `autonomous-harness-prior-art.md` section 2 has its DB requirement for budgets).
- OpenRouter BYOK (https://openrouter.ai/docs/guides/overview/auth/byok): provider keys are
  "securely encrypted" (method undocumented), several per provider, each scopable to models,
  OpenRouter API keys or workspace members, ordered as prioritized or fallback, with OpenRouter's
  own capacity as fallback unless disabled.
- Common shape: the caller never holds the provider credential; it holds a proxy-issued identity,
  and the proxy maps that identity to the provider credential and to its own budget. The kit
  already has this shape (per-caller connection key and windowed ceiling, rulings `connector-owner`,
  `connector-spend-caller-window-ceiling`); what neither proxy documents is OS-level isolation of
  the key from co-located processes, which is ours to supply (a unit-private credential file).

## 4. Thinking across tool use: where the in-flight exchange starts

Source: https://platform.claude.com/docs/en/build-with-claude/thinking ("Thinking with tool use",
"Preserving thinking blocks", "Thinking block preservation by model", "Thinking encryption",
"Controlling thinking display"); https://platform.claude.com/docs/en/agents-and-tools/tool-use/handle-tool-calls.

- "A tool-use loop is one assistant turn." An assistant turn ends only when Claude completes its full
  response, which may include several tool calls and results. Tool results appear as user messages
  but are part of one continuous reasoning flow.
- Required: within a tool-use turn, pass thinking blocks back complete and unmodified. Within the
  latest assistant message the consecutive thinking blocks must match what the model generated: no
  reorder, edit or partial drop, `redacted_thinking` included; a modified block is a 400.
- In the user message carrying tool results, "the `tool_result` blocks must come FIRST in the
  content array. Any text must come AFTER all tool results", and results must immediately follow
  their `tool_use` message. Text after the results is legal with client tools only.
- Mid-turn toggle of thinking does not error: the API silently disables thinking for that request.
  Adaptive mode does not require an assistant turn to start with a thinking block; manual mode does.
- Keep-all-prior-turns models: Opus 4.5 and later Opus, Sonnet 4.6 and later, Haiku 5.5, Fable and
  Mythos 5/5.1; earlier models strip earlier-turn thinking. Kept blocks bill as input. A block is
  readable only by its model and certain others; unreadable blocks are dropped without error.
- `signature` carries the full thinking encrypted; opaque, do not parse; compatible across Claude
  API, Bedrock and Google Cloud, except Sonnet 5.5 and Haiku 5.5 blocks, which "work only in the
  account that produced them or in an account linked to it".
- `display: "omitted"` is the default on Opus 5.5, Sonnet 5.5, Haiku 5.5, Fable 5.1 and others:
  empty `thinking` field, full thinking still billed, replay unchanged.
- Prefix binding (from `725f5a5` 1a, confirmed on the errors page): on Fable 5.1, Opus 5.5,
  Sonnet 5.5 and Haiku 5.5 a replayed block is accepted only while `system`, `tools` and the
  preceding messages are unchanged.

Answer to follow-up `claude-tool-exchange-boundary`: the kit's rule "in flight after the last user
text" is wrong for Claude whenever user text rides with tool results. In the kit transcript a
contiguous run of `UserText` and `ToolResult` items lowers to one user message; if that run holds a
`ToolResult`, the assistant turn before it is still in flight (the model has not yet answered those
results). So the in-flight exchange starts after the last user-side run that holds no `ToolResult`.
Probe P1 (section 8) shows the current kit drops, and executes past, foreign state in
`[user, foreign, call, result, user]`.

## 5. Counting tokens for the reservation

Source: https://platform.claude.com/docs/en/build-with-claude/token-counting;
https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools ("Tool use system prompt").

- `POST /v1/messages/count_tokens` takes the Messages inputs and returns `{input_tokens}`. It is free,
  has its own RPM limit (Start 5,000, Build 10,000, Scale 20,000), uses no caching logic, and is
  "an **estimate**": "the actual number of input tokens ... might differ by a small amount", and
  counts may include system-added tokens that are not billed. So it is neither an upper nor a lower
  bound, and it is one more network call per request with its own failure mode.
- Claude 4.7 and later (Fable, Mythos) use a newer tokenizer: about 30% more tokens for the same text.
  Still a byte-level tokenizer as far as the docs say; nothing documents a token shorter than one
  byte of text: UNVERIFIED.
- With `tools`, the API constructs a system prompt from the tool definitions and its own fixed
  instruction text. That text is input the caller did not send: a per-model constant the price
  table's `overhead_tokens` must cover.
- Prior-turn thinking on keep-all models bills as input; what it bills is the decrypted thinking,
  carried in the signature. Whether the signature's byte length is at least that thinking's token
  count is not documented and cannot be probed without a live call: the one premise of the kit's
  byte bound left open for Claude (follow-up `claude-byte-bound-live-check`).

## 6. Facts the Messages lowering must respect

- Tool names match `^[a-zA-Z0-9_-]{1,128}$`. `tool_choice`: `auto`, `any`, `tool`, `none`, plus
  `disable_parallel_tool_use`. Opus 5.5, Sonnet 5.5, Fable 5.1 and Mythos 5.1 reject `any` and `tool`
  with a 400 ("tool_choice: type \"tool\" and \"any\" are not supported for this model"); on models
  that accept forced tool use, the forced call skips thinking. Changing `tool_choice` invalidates
  cached message blocks.
- Claude 4.6 and later reject a prefilled final assistant message: the conversation must end with a
  user message.
- Thinking: `thinking: {type: "adaptive"}` with `output_config.effort`; `type: "enabled"` is a 400 on
  4.7 and later; `type: "disabled"` is a 400 on Fable 5.1, Mythos 5.1, Fable 5, Mythos 5, Opus 5.5;
  Sonnet 5.5 uses `between_tools` instead. Omitting `thinking` runs adaptive thinking.
- Structured outputs: `output_config.format: {type: "json_schema", schema}`; `minimum`, `maxLength`
  and similar are unsupported (400); a `refusal` or `max_tokens` stop may yield non-conforming JSON.
- Usage fields are in `725f5a5` 1a: `input_tokens` (after the last cache breakpoint),
  `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens`,
  `output_tokens_details.thinking_tokens`. Stop reasons include `model_context_window_exceeded`,
  `refusal` (HTTP 200, billed) and `pause_turn` (server tools only).

## 7. Prices

`725f5a5` 2e holds the current price table (per MTok: input, 5m write, 1h write, cache read, output;
Haiku 5.5's long-context tier above 100k prompt tokens). The kit's `Rates` and `LongContext` carry
every class it names; the connector's config document holds one `ModelPrice` per allowed model.

## 8. Probes (Imagination, Yggdrasil ygg-verify rust image, 2026-10-28)

Scratch commits `eb021f0` and `0c9a39d` on ModelConnector `e0c0832`, never pushed.

- P1 boundary: a kit lib test admitting `[UserText, ReasoningState(other-provider), ToolCall,
  ToolResult, UserText]` printed `executed, dropped=1`: the foreign item in an unanswered tool
  exchange is dropped and the request runs.
- P2 verbatim schemas: `serde_json::value::RawValue` holding `{"b":1,"a":2}` serialized through the
  `json!` macro came out `{"a":2,"b":1}` (the macro builds a `Value`, whose map sorts keys); through
  a typed `#[derive(Serialize)]` struct field it came out `{"b":1,"a":2}`. A lowering that must keep
  the caller's tool and output schemas byte-for-byte (prefix binding, section 4) serializes typed
  structs with `RawValue` fields, never `json!`.
- P3 client: `ureq 3.5.0` (`default-features = false`, `rustls`) with `serde_json` `raw_value`
  resolves and builds in the workspace (rust 1.95). Baseline at `e0c0832`: 138 kit lib tests with
  `daemon`, all green.

## 9. What the cut takes from this

- The credential is an operator-owned Console API key in a dedicated workspace with its own spend
  and rate limits (question `claude-connector-credential`, recommended), held only by the connector.
- The kit's in-flight boundary moves to the last user-side run with no tool result (section 4).
- The reservation keeps the kit's byte bound; the Claude price entries' `overhead_tokens` covers the
  tool system prompt; the receipt's `overrun_micro_usd` is the live instrument (section 5).
- The lowering is a pure, deterministic function over typed structs with `RawValue` for caller JSON;
  the response is read as a stream; failures that the docs do not say are free bill the reservation.
