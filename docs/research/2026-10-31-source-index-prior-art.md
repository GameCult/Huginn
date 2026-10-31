# Huginn source index: prior art and probes

Date: 2026-10-31. Session `eureka-self-2026-10-31`, Imagination pass for
`eureka-body:follow_up:huginn-prose-index` under
`eureka-body:ruling:prose-retrieval-top-priority`. This note adds to
`gamecult-ops/docs/research/agent-knowledge-store-prior-art.md` (cited as PA)
on the forks that note does not cover: who may read which repos, how source
should be chunked, and what the embedders on the mesh can carry. Facts only.
Tags as in PA: [V] seen in a search result or measured this pass; [M] from
prior knowledge, not re-fetched.

## 1. Who may read which corpus (permission-aware retrieval)

- The common pattern is to tag every chunk with its access policy at index
  time and put a reader-specific filter inside the retrieval query, not after
  it. Filtering after retrieval is described as weaker, because unauthorised
  text has already reached the answerer's context. [V] (Tian Pan, "Permission-
  Aware Retrieval", 2026-05; letsdatascience "RAG system design permissions")
- Stale permissions are a stated failure: a repo made private, or a reader's
  access withdrawn, must reach the index within a bounded time, and some
  practitioners re-check against the source at answer time rather than trust
  index metadata alone. [V] (same sources)
- Database-level enforcement (Postgres row-level security under pgvector)
  appends the permission predicate to every similarity query. [V]
- GitHub code search returns only repositories the searcher can read;
  Sourcegraph syncs code-host permissions and enforces them at query time.
  [M] (not confirmed this pass; the search returned no Sourcegraph primary
  source)

## 2. Chunking source code

- cAST (Zhang et al., CMU and Augment Code, arXiv 2506.15655, EMNLP 2025
  Findings) chunks by merging AST siblings greedily up to a size bound and
  splitting oversized nodes. Against line-based chunking it reports +4.3
  Recall@5 on RepoEval retrieval and +2.67 Pass@1 on generation. Code:
  `astchunk`. [V]
- PA §3 already records that chunk size and overlap sensitivity vary by corpus
  with no universal optimum, and that heading-path prefixes are common for
  markdown. [M]
- VoidBot's live chunker is line windows of at most 48 lines or 1,400
  characters, no overlap, per file, with `lineStart`/`lineEnd` kept
  (`VoidBot/packages/rag/src/source-document-ingester.ts:6-7,93-140`). [V]

## 3. Probes on the mesh, 2026-10-31 (UTC)

All read-only except one scratch Qdrant collection, created and deleted.

- Embedder throughput, `qwen3-embedding:0.6b`, 1,400-character code chunks:
  - Yggdrasil `ollama-cpu` (`127.0.0.1:11434`), host load average 22 on 16
    CPUs (a verify container was running): 8 chunks in 26.8 s, 32 in 184.6 s,
    so about 0.2 chunks per second. [V]
  - Raven GPU (`10.77.0.4:11434`, same model, digest `ac6da0dfba84`), asked
    from Yggdrasil: 32 chunks in 1.3 s, 64 in 2.1 s, about 30 chunks per
    second after the first (cold) call. [V]
  - At those rates VoidBot's present corpus (136,381 points in
    `voidbot_repository_source_chunks`) is roughly 75 minutes on Raven and
    about a week on Yggdrasil's CPU under that load. [V, arithmetic]
  - In the same hour, a semantic query to the eureka mind timed out at the
    CLI's 15 s deadline, so the CPU embedder was not answering even single
    query embeddings promptly. [V]
- Qdrant 1.17.1 collection metadata: `PATCH /collections/{c}` with
  `{"metadata": {...}}` merges top-level keys and replaces a nested object
  whole (`{"a":1,"repos":{"X":..}}` patched with `{"repos":{"Y":..}}` read
  back `{"a":1,"repos":{"Y":..}}`). [V]
- The GameCult forge (`forgejo-eureka.service`) answers on Yggdrasil loopback
  at `http://127.0.0.1:3310` with no TLS; its public API lists 62 GameCult
  repositories, 15 of them archived. `gamecult-ops`, `cultcache-rs` and
  `cultnet-rs` are not among the public ones. [V]
- VoidBot's hourly refresh (`voidbot-source-refresh.service`) has failed every
  run since 2026-10-30 02:16 UTC with `Unexpected origin for Aetheria:
  https://eureka.gamecult.org/forge/GameCult/Aetheria.git`; its last success
  was 2026-10-30 01:14 UTC. Its catalog is the public GitHub org. Its MCP
  container still answers `search_sources` from the stale collection. [V]

## Primary sources

- https://arxiv.org/abs/2506.15655 (cAST)
- https://tianpan.co/blog/2026-05-04-permission-aware-retrieval-enterprise-rag-access-control
- https://letsdatascience.com/blog/rag-system-design-permissions
