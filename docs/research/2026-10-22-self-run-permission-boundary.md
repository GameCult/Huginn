# Prior art: permission boundary for an unattended Self run

Serves fork `eureka-body:question:self-run-permissions` (campaign eureka-body, session self-2026-10-22-eureka-c2a). Eyes pass, 2026-10-22. Facts and evidence pointers only; no recommendation.

Source status key: FETCHED = page read in full or in the relevant part via WebFetch. SEARCH-ONLY = seen only as a WebSearch summary, page not read. NOT FETCHED = not obtained.

## Claims in the fork's prior-art line, checked

| Fork claim | Result |
|---|---|
| Managed settings at `/etc/claude-code/managed-settings.json` on Linux | Confirmed (FETCHED, managed-settings). |
| Managed outranks user and project settings | Confirmed (FETCHED, settings). |
| Docs advise bypassPermissions only in an isolated container or VM | Confirmed, wording stronger: see 1.3 (FETCHED). |
| `--permission-prompts none` exists | Confirmed, needs Claude Code v2.1.259+ (FETCHED, cli-reference, headless). |
| Codex "full-auto" runs approval never inside workspace-write | Not supported by the official page: see 2 (FETCHED). The current official page does not define `--full-auto`. A search summary cited third-party blogs that disagree with each other (`on-request` vs `never`); those pages were NOT FETCHED. |

## 1. Claude Code

### 1.1 Managed settings and precedence
Source: https://code.claude.com/docs/en/managed-settings (FETCHED), https://code.claude.com/docs/en/settings (FETCHED).
- Linux and WSL file: `/etc/claude-code/managed-settings.json`; optional `/etc/claude-code/managed-settings.d/*.json` drop-ins merged with it; `managed-mcp.json` same directory. macOS `/Library/Application Support/ClaudeCode/`, Windows `C:\Program Files\ClaudeCode\`.
- Precedence, highest first: managed, command line (`--settings`), project local, shared project, user. "Nothing in your own settings files or `--settings` overrides a managed key."
- List keys such as `permissions.allow` merge across scopes, unless `allowManagedPermissionRulesOnly` is set. Deny rules from any scope are evaluated before allow rules.
- Exceptions where a stricter lower-scope value still wins over managed exist for a short list (e.g. `permissions.blockReadsOutsideWorkingDirectories: true` from any scope); none loosens a managed deny.
- Keys that exist in managed settings for policy (FETCHED, managed-settings table): `allowManagedPermissionRulesOnly`, `allowManagedHooksOnly`, `allowManagedMcpServersOnly`, `permissions.disableBypassPermissionsMode`, `permissions.disableAutoMode`, `sandbox.network.allowManagedDomainsOnly`, `sandbox.filesystem.allowManagedReadPathsOnly`. `disableBypassPermissionsMode` "works from any scope".
- `permissions.defaultMode` values `auto` and `bypassPermissions` "don't take effect from project or local settings; set them in user or managed settings instead, or pass `--permission-mode`". Before v2.1.257 bypassPermissions took effect from any file.
- What a developer can change: "a developer who is an administrator on the machine can edit the managed source itself".
- Unreadable managed file: "If the operating system denied the read, for example on a root-only file, every session starts without that source's policies." A present-but-unparseable managed file makes Claude Code refuse to start. Invalid single-value restrictive keys read as the restrictive value.
- On WSL, `/etc/claude-code` is user-writable (not relevant to a native Linux unit).

### 1.2 Permission rules
Source: https://code.claude.com/docs/en/permissions (FETCHED; large page, read by targeted extraction).
- Evaluation order: deny, then ask, then allow; first match decides; specificity does not reorder.
- Bash rules match the whole command text; `*` wildcard; `Bash(git *)` and legacy `Bash(git:*)` are equivalent when the colon-star is a trailing suffix. A space before `*` matters (`Bash(git diff*)` also matches `git diff-index`).
- Edit/Write/Read take path rules (`Read(./.env)`, `Read(./secrets/**)`); `WebFetch(domain:example.com)`, `WebFetch(domain:*.example.com)`.
- `Bash(command:rm *)`-style constraint on the primary content field is ignored with a startup warning because it "would be bypassable by a compound command".
- Compound commands (`&&`, `;`, pipes) are split and each subcommand must match. Built-in wrapper stripping: `timeout`, `time`, `nice`, `nohup`, `stdbuf`, `command`, `builtin`, `noglob`; not stripped: `direnv exec`, `devbox run`, `mise exec`, `npx`, `docker exec`, `watch`, `setsid`, `ionice`, `flock`; so `Bash(devbox run *)` matches `devbox run rm -rf .`.

### 1.3 Documented limits of Bash patterns
Same source (FETCHED).
- "A deny or ask rule covers the invocation Claude usually produces and isn't a security boundary around the program." Table of non-matches: `Bash(curl *)` does not stop `/usr/bin/curl ...` or `sh -c 'curl ...'`; `Bash(rm *)` does not stop `/bin/rm -rf` or `bash -c`; `Bash(git push *)` does not stop `git -C . push origin main`, `git -c push.default=current push ...`, `git 'push' origin main`.
- "Bash permission patterns that try to constrain command arguments are fragile" (curl-URL example: option reordering, protocol change, redirects, variables).
- Docs' pointers for reliable enforcement: sandboxing ("filesystem and network enforcement that doesn't depend on the command text"), PreToolUse hooks, WebFetch domain rules plus sandbox network allowlist.
- A blocking PreToolUse hook (exit code 2) stops the call before rules; hooks cannot override a matching deny rule.
- Read/Edit deny rules apply to built-in file tools and recognised file commands (`cat`, `head`, `sed`, `tee`, redirects); not to commands that read without naming a file (e.g. `grep -r pattern .`).

### 1.4 Permission modes and bypassPermissions
Sources: https://code.claude.com/docs/en/permission-modes (FETCHED, last 812 chars unread), permissions page (FETCHED).
- Modes: `default`/manual, `acceptEdits`, `plan`, `auto` (classifier reviews actions), `dontAsk`, `bypassPermissions`. Table row for bypassPermissions: "Isolated containers and VMs only".
- permission-modes page text on bypassPermissions (via WebFetch summarizer, near-verbatim): use only "in isolated environments like containers, VMs, or dev containers without internet access, where Claude Code cannot damage your host system"; "offers no protection against prompt injection or unintended actions". The permissions page says "Only use this mode in isolated environments like containers or VMs where Claude Code can't cause damage."
- On Linux/macOS Claude Code refuses bypassPermissions when running as root or under sudo unless it detects a recognised sandbox; the docs recommend a non-root user (dev container).
- Even in bypassPermissions these are not auto-approved: explicit `ask` rules; connector tools set to `ask` by an org; tools requiring user interaction; recognised `rm`/`rmdir` critical-path removals ("no allow rule or PreToolUse hook approves"); cross-session messaging safeguards; reads outside working dirs when `blockReadsOutsideWorkingDirectories` is on. Writes to protected paths (`.git`, `.claude`, `.bashrc`, `.gitconfig`, `.mcp.json` etc.) are allowed in bypass.
- `dontAsk`: auto-denies everything that would prompt; reads in working directories, read-only commands, and `permissions.allow` / `--allowedTools` matches still run. "Locked-down CI and scripts".
- Flags: `--permission-mode`, `--dangerously-skip-permissions` (equivalent to bypassPermissions), `--allow-dangerously-skip-permissions`, `--restricted` (v2.1.248+: loads only managed settings and `--settings`, refuses bypassPermissions, removes command-running tools unless named in `--tools`).

### 1.5 `--permission-prompts`
Sources: https://code.claude.com/docs/en/cli-reference (FETCHED), https://code.claude.com/docs/en/headless (FETCHED).
- `--permission-prompts host|none`, print mode, v2.1.259+. With `none`: "Anything that would prompt is denied unless a `PermissionRequest` hook allows it, Claude is told that nobody can approve the request and not to retry it". Rules, `PermissionRequest` hooks and the permission mode "still decide every call first". Removes `AskUserQuestion`; unanswered MCP elicitations are cancelled. Denials appear as `permission_denied` system messages and in `permission_denials` of the result with `stream-json`.
- `--permission-prompt-tool mcp__<server>__<tool>` handles prompts in non-interactive mode.
- Headless page: from v2.1.302 `claude -p` starts in Manual mode when nothing sets a mode; a call needing approval is denied unless a hook or prompt tool approves. `--bare` is recommended for scripted calls and skips CLAUDE.md, hooks, plugins, MCP, auto memory discovery (relevant: a bare run would not load the Self's own CLAUDE.md or skills; managed settings are still read per `--restricted`/safe-mode wording, but `--bare` and managed settings were not explicitly cross-checked on the fetched page).
- Without `--bare`, a `-p` session runs hooks from the project's `.claude/settings.json` and connects its `.mcp.json` servers, with no trust dialog.

### 1.6 Claude Code's own sandbox (relevant to "OS boundary" options)
Source: https://code.claude.com/docs/en/sandboxing (FETCHED, targeted grep of the saved page).
- Linux/WSL2/macOS only; uses `bubblewrap` and `socat`; optional seccomp filter adds Unix-socket blocking. Native Windows runs commands unsandboxed.
- Covers Bash, PowerShell, Monitor commands and children only. "Claude's file tools, MCP servers, and hooks run outside it."
- Network: no direct route out; connections go through a proxy on the machine that checks each host against `sandbox.network.allowedDomains` (starts empty); hostname-level.
- Escape hatch: `dangerouslyDisableSandbox` retry; disabled with `sandbox.allowUnsandboxedCommands: false`; `failIfUnavailable` setting exists. A matching allow rule such as `Bash(curl *)` also approves the unsandboxed retry.
- Inside containers bubblewrap can fail with `Operation not permitted`; Ubuntu 24.04+ AppArmor setting `kernel.apparmor_restrict_unprivileged_userns` can block bubblewrap.
- Permissions and sandbox are described as complementary: "sandbox restrictions still apply even if a prompt injection bypasses Claude's decision-making".

## 2. OpenAI Codex CLI
Source: https://learn.chatgpt.com/docs/agent-approvals-security.md (FETCHED; redirect target of developers.openai.com/codex/agent-approvals-security.md). The GitHub `docs/sandbox.md` in openai/codex is a pointer only (FETCHED).
- Approval policies: `on-request`, `never` ("Codex doesn't prompt for approval. Sandbox modes still apply."), granular table form; `untrusted` retired.
- Sandbox modes: `read-only`, `workspace-write` ("Network access is off by default"; enable with `[sandbox_workspace_write] network_access = true`), `danger-full-access` (no sandbox).
- `features.network_proxy` constrains enabled traffic by domain rules, allowlist-first, deny wins; does not grant network by itself; local/private destinations blocked by default.
- "Auto" preset is `--sandbox workspace-write --ask-for-approval on-request`. The page does not define `--full-auto`.
- OS sandbox: macOS Seatbelt; Linux `bwrap` + `seccomp`; Windows MXC or legacy modes; WSL2 uses the Linux implementation.
- `--dangerously-bypass-approvals-and-sandbox` (`--yolo`): removes sandbox and approvals, "not recommended".
- Externally sandboxed environments: if a container blocks namespace/setuid bwrap/seccomp operations, "configure the container for the isolation you need, then run Codex with `danger-full-access` inside it". Warns Dev Containers "do not prevent every attack"; with full access inside a container "a malicious project could exfiltrate data available there, including Codex credentials".
- Third-party summaries about `-a never -s workspace-write` for CI came from a WebSearch summary only; NOT FETCHED.

## 3. Other unattended coding agents

| Agent | Boundary | Network egress | Status |
|---|---|---|---|
| GitHub Copilot cloud/coding agent | Ephemeral GitHub Actions environment plus a firewall for processes started via its Bash tool | Firewall on by default; always-allowed GitHub hosts plus a recommended allowlist (OS package repos, container registries, language package registries, CAs, Playwright browser hosts); org/repo allowlist additions; blocked requests are reported in the PR. Not covered: MCP server processes and processes started in setup steps. Docs: "should not be considered a comprehensive security solution." | FETCHED https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/customize-the-agent-firewall ("runs on Actions infrastructure" statement was SEARCH-ONLY) |
| OpenHands | Docker sandbox container "recommended"; "process" sandbox is "unsafe, but fast", "No container isolation"; remote option | Overview page does not describe egress; Docker page does not describe egress restrictions, names read-write mounts as the main risk (`SANDBOX_VOLUMES`, `--mount-cwd`). Search summary: FAQ says agent can reach the internet from the container (NOT FETCHED); confirmation policies `AlwaysConfirm`/`ConfirmRisky` with pluggable security analyzers exist in the SDK (SEARCH-ONLY) | FETCHED https://docs.openhands.dev/openhands/usage/sandboxes/overview and .../docker; SDK security page SEARCH-ONLY |
| SWE-agent | Docker container per run via SWE-ReX (local Docker, or remote such as Modal/AWS); shell session inside the container | Architecture page silent on egress; no command allow/blocklist described on that page (a tool-bundle `blocklist` config exists in the project, NOT FETCHED) | FETCHED https://swe-agent.com/latest/background/architecture/ |
| Devin | Per-session VM ("Devbox"); enterprise option runs Devbox in customer VPC; Brain in Cognition tenant; Outposts: customer machines dial out only | VPC option needs outbound HTTPS 443 only; docs "strongly recommend" granting internet access to in-VPC instances | SEARCH-ONLY (docs.devin.ai/enterprise/vpc/overview, deployment pages, devin.ai/blog/introducing-devin-outposts); no page fetched |

Pattern observed across the five agents fetched or searched: in each the unattended boundary is an OS/container/VM isolation unit; command allowlists appear as a secondary layer (Claude Code rules, OpenHands confirmation policies), not as the sole boundary. Egress handling differs: Codex default off; Copilot allowlist firewall scoped to the Bash tool; Claude Code proxy allowlist scoped to Bash; OpenHands and SWE-agent pages read do not state egress controls.

## 4. systemd sandbox directives
Sources: https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html (FETCHED, chars 0-200000 of 244377; last 44377 unread), https://www.freedesktop.org/software/systemd/man/latest/systemd.resource-control.html (FETCHED).
- `ProtectSystem=strict`: whole file-system hierarchy read-only except `/dev`, `/proc`, `/sys`; `ReadWritePaths=` re-opens paths; `StateDirectory=`, `LogsDirectory=` etc. stay writable; with `PrivateTmp=` `/tmp` and `/var/tmp` writable. Caveat: "This setting cannot ensure protection in all cases".
- `ProtectHome=true`: `/home`, `/root`, `/run/user` inaccessible and empty. No protection if homes are at non-standard locations.
- `PrivateTmp=true`: private `/tmp` and `/var/tmp`, removed on stop.
- `ReadWritePaths=` / `ReadOnlyPaths=`: refused "even if the usual file access controls would permit"; `-` prefix ignores missing paths. Caveats: writable host mounts that propagate in remain writable; "Privileged processes can undo these settings", pair with `CapabilityBoundingSet=~CAP_SYS_ADMIN` or `SystemCallFilter=~@mount`.
- `StateDirectory=`: created under `/var/lib/`, owned by `User=`/`Group=`, not removed on stop.
- `NoNewPrivileges=true`: no privilege gain through `execve()` (setuid/setgid bits, file capabilities). It "does not affect" processes invoked on request through `at`, `crontab`, `systemd-run`, "or arbitrary IPC services" (page does not name polkit or D-Bus; the generic IPC statement is the evidence).
- `DynamicUser=` implies `ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp=disconnected`, `NoNewPrivileges`, `RestrictSUIDSGID`; "currently incompatible with D-Bus policies".
- `RestrictAddressFamilies=`: allow-list (or `~` deny-list) of `AF_*`; applies only to `socket(2)`; not to sockets passed in (socket activation), `socketpair()`, or `io_uring`; no effect on 32-bit x86, s390, s390x and some others; recommends `SystemCallArchitectures=native`; keep `AF_UNIX` for local IPC and syslog. It selects families (e.g. AF_INET/AF_INET6 vs AF_UNIX), not destinations.
- `IPAddressAllow=` / `IPAddressDeny=`: lists of IPv4/IPv6 addresses with optional prefix length, plus `any`, `localhost`, `link-local`, `multicast`; apply to IPv4/IPv6 sockets of the unit; filter ingress (source) and egress (destination). Order: allow match grants; else deny match denies; else granted. Both empty by default. Recommended allow-list pattern: `IPAddressDeny=any` on a higher slice, per-service `IPAddressAllow=`. The page does not mention hostnames, DNS, or SNI; the directives take IP literals only. May have no effect where eBPF cgroup support is missing; the page advises against relying on them alone. Socket-unit lists do not cover the activated service's sockets.
- Consequence stated by the directives' own definitions (no separate source): hostname-based egress filtering is not expressed by any of these directives.

## Gaps
- Codex `--full-auto` current meaning not stated in any fetched official page.
- Devin, OpenHands V1 egress/security pages and SWE-agent tool blocklist docs not read.
- systemd.exec last 44377 characters unread (covers later sections, not the directives above).
- Claude Code permissions page (84 KB) and sandboxing page (97 KB) read by targeted extraction, not end to end; the `--bare` interaction with managed-settings loading is not confirmed from a fetched sentence.
- No test was run of `--permission-prompts none` with a managed `bypassPermissions` file on the target Claude Code version.
