# Prior art: held read image over an append-only receipt log

Serves follow-up `eureka-substrate:follow_up:huginn-materialized-image` ("Prior-art pass first: MVCC read views, LMDB/redb read transactions, Datomic database values, event-sourced projections").
Served spec: `eureka-substrate:cut_spec:cut-mind-read-floor.r1` (Huginn branch `hands/eureka-substrate-mind-read-floor`).
Session: self-2026-11-02-eureka. Eyes pass, 2026-11-03. Facts with pointers, no recommendation.
Method note: 22 tool calls; WebFetch returns a small-model digest of each page, not the raw page. Items marked [not stated] were asked for and the page did not say. Kafka Streams primary docs would not load (redirect shell, 404 on raw site source); Kafka facts are from secondary pages plus KIP-1035 (primary). Datomic on-prem pages were fetched; Datomic Cloud architecture page too.

## Sources
- redb design doc: https://raw.githubusercontent.com/cberner/redb/master/docs/design.md
- redb README: https://raw.githubusercontent.com/cberner/redb/master/README.md
- redb ReadTransaction API: https://docs.rs/redb/latest/redb/struct.ReadTransaction.html
- redb src/transactions.rs (first 100,000 of 129,790 chars read; ReadTransaction/begin_read not in that range): https://raw.githubusercontent.com/cberner/redb/master/src/transactions.rs
- LMDB lmdb.h: https://raw.githubusercontent.com/LMDB/lmdb/mdb.master/libraries/liblmdb/lmdb.h
- Datomic caching/memory index: https://docs.datomic.com/operation/caching.html
- Datomic capacity planning: https://docs.datomic.com/operation/capacity.html
- Datomic filters (as-of/since/history): https://docs.datomic.com/reference/filters.html
- Datomic Cloud architecture: https://docs.datomic.com/whatis/architecture.html
- Fowler, Memory Image: https://martinfowler.com/bliki/MemoryImage.html
- Kafka KIP-1035: https://cwiki.apache.org/confluence/display/KAFKA/KIP-1035%3A+StateStore+managed+changelog+offsets
- Kafka Streams state restore (secondary): https://www.conduktor.io/kafka-streams/state-restore ; https://developer.confluent.io/courses/kafka-streams/stateful-fault-tolerance/ (search snippets only, not fetched)
- Kurrent/EventStoreDB persistent subscriptions (search snippet only): https://docs.kurrent.io/clients/grpc/persistent-subscriptions
- The spec itself, read via `view` (ordinal 5487).

## 1. Current-state reads without reading the log

- redb: current state is the committed B+tree root; the log is not a separate structure. A read txn "make[s] a private copy of the root of the B+tree" (design.md). Tables are copy-on-write B+trees (README). Reads walk the tree; nothing is replayed.
- LMDB: "The entire database is exposed in a memory map, and all data fetches return data directly from the mapped memory" (lmdb.h). Readers run lock-free on a multi-versioned structure.
- Datomic: four sort-order indexes over all datoms; queries pick one (Cloud page). The current db "does no filtering, so queries about 'now' pay no penalty for history size" (filters page). A query is answered from persistent-index segments plus the local memory index (capacity/caching pages; forum snippet). The log is read for time-only questions, not for entity reads.
- Fowler Memory Image: the application holds current state in RAM; events are the persistence; reads never touch the log.
- Kafka Streams: reads go to a local state store (RocksDB or in-memory) materialized from a changelog topic; the changelog is read only to restore (secondary sources; KIP-1035 confirms the store/changelog-offset pairing).
- EventStoreDB/Kurrent: read models live in a consumer's own storage; the docs' stated best practice is to store the checkpoint "in the same storage" as the projection and project plus checkpoint in one transaction (search snippet of docs.kurrent.io).

## 2. As-of / snapshot reads and their cost

- redb: a read txn is a snapshot of the latest committed state at the moment it begins. Writers never modify committed pages; freed pages are reused only "after all read transactions that could reference it have completed" (design.md). Cost of starting one: registers the snapshot; multi-process mode adds locks and cache invalidation on seeing a new txn id; no figure given [not stated]. Arbitrary past points: only via savepoints. Ephemeral savepoint registers as a read txn; persistent savepoint is stored in the system table, survives restart, must be deallocated, and "saves a copy of the page allocator state", estimated "about 64kB per 1GB of values (assuming 4kb pages)". Restore resets the root and queues later-allocated pages for freeing. ReadTransaction is Send and Sync and "may exist concurrently with writes" (docs.rs). The README and docs.rs say nothing on retention cost beyond that.
- LMDB: txn id identifies the snapshot. Long readers cost space: "Read transactions prevent reuse of pages freed by newer write transactions, thus the database can grow quickly"; stale readers from aborted programs "cause further writes to grow the database quickly"; reader table default 126 slots, MDB_READERS_FULL on exhaustion; mdb_txn_reset/renew exist to reuse a handle. Snapshots are only of the newest committed state at txn begin; no as-of-past facility in the doc.
- Datomic: as-of(t or instant), since, history, filter are filters over the same indexes (filters page, stated as "appear" for as-of). "Filtering is not free"; time-only questions via as-of "would require a filtering scan of the entire database"; entity-plus-time can jump to the entity's datoms in EAVT then drop later datoms. The database value is immutable; caches hold "only immutable data, so all caches are valid forever" (caching page).
- Fowler: no as-of facility described; replay from snapshot reconstructs state at the replayed position.
- Kafka Streams: stores keep current value per key; no as-of [not stated].

## 3. RAM vs paged; history outgrowing RAM

- redb: page cache exists, with a `cache_metrics` feature; size, eviction, and mmap-vs-pread are [not stated] in the docs fetched. Data size is not bounded by RAM; the file is paged. Old versions occupy file space while a reader or persistent savepoint pins them.
- LMDB: whole database is mmapped; the OS pages it. Long readers grow the file, not RAM.
- Datomic: indexes live in storage, segments (thousands of datoms) cached in a per-process LRU object cache in the app address space (caches page and "competes with your application for available memory"). Datasets can be "much larger than can fit in memory" (Cloud page). Only the memory index (recent, not-yet-indexed novelty) is held in full: transactor default threshold 32m, max 512m; peers may hold up to memory-index-max per connected database. Past the max the transactor "will radically slow the rate at which it processes transactions" (back pressure). A dataset that fits the peer's object cache gives "very fast reads". Whole-history-in-RAM beyond that is not what Datomic does.
- Fowler: "Available RAM must exceed the data the application needs in memory"; mitigation suggested is sending each image only the relevant subset of events.
- Kafka Streams: stores are RocksDB (disk) or in-memory; the changelog is on the brokers (secondary).

## 4. Startup / rebuild cost; checkpoints of derived state

- redb: quick repair on open reads allocator state from a table; if the recovery flag is set, full repair "walking the B+tree from all active roots" grows with database size (design.md); no timings. Read-only opens skip corruption detection.
- LMDB: open maps the file; no rebuild.
- Datomic: a peer "builds the memory index from the log before the call to connect returns" (caching page): it replays only the log tail since the last index job; the persistent index is the checkpoint. The transactor does the same before serving. Index jobs are background, "mostly serial", adaptive.
- Fowler: replay of all events is "too slow in practice"; the system periodically saves snapshots and recovery loads the latest then replays after it; "Writing and loading snapshots is code you have to build yourself." Restore speed must meet quality-of-service targets.
- Kafka Streams: restore replays the store's changelog; secondary sources say a checkpoint file records the changelog offset the local store is consistent up to, restore applies the delta from it and falls back to full replay if lost/inconsistent; the checkpoint is written on graceful shutdown under EOS so a hard crash may force full rebuild. KIP-1035: offsets moved from per-task `.checkpoint` files into the store (RocksDB column family, flushed atomically with data) because a separate file cannot be synced atomically with state.
- Kurrent: checkpoint stored with the read model in one transaction.

## 5. Known divergence failure modes of a held projection or index

- Kafka KIP-1035: offsets in a file separate from the state cannot be atomically synced with the state; under EOS the task state is wiped when flushing is involved; fallback is full changelog replay. Secondary sources: hard crash forces full rebuild.
- Datomic: caches are immutable and "valid forever", so object cache cannot diverge; the memory index is kept current by transactor-to-peer notifications, not invalidation. A missing cache layer segment is reloaded from the next layer (Cloud page). Peer failure to receive a notification mode is [not stated].
- Kurrent: a checkpoint stored apart from the projection storage cannot give exactly-once; hence the same-transaction advice.
- Fowler: events must be processed reliably and the event store must be reliable; concurrency control is yours; event schema changes may require migrating the log; keep events decoupled from model structure.
- LMDB/redb: divergence of a derived structure is not their concern; their hazards are pinned old pages from long or stale readers (LMDB explicit; redb by the freeing rule) and, for redb, compaction refused while a read txn is open.

## Spec versus the prior art

Same:
- RAM-held fold of the whole log, rebuilt by replay at open, serves current-state reads without touching the log: Fowler Memory Image. Spec's "nothing derived persisted" matches Fowler's base form before snapshots are added.
- A single writer with a serial loop and no locks: Fowler cites LMAX single writer thread; the spec cites the daemon's serial loop.
- Advance per commit in place, rebuild only after a lost swap or store error: Datomic updates its memory index per transaction on transactor and peers and drops it on index completion; Kafka restores only on state loss.
- Index structures narrowing candidates, with the real evaluator deciding: Datomic query chooses among four indexes and filters. The spec's postings are narrowing-only (superset) and `select` decides.
- Held-versus-fresh equality as a check: Kafka falls back to full replay; the spec's read_back gate compares held image to a fresh fold. No cited system has an equivalent gate in the pages read.
- As-of as a filter over current structures rather than a separate stored view: Datomic as-of filters the same indexes and says filtering is not free; the spec keeps rows with ordinals and re-derives status for as_of when status_changed > as_of.

Differ:
- Persistence of derived state: Fowler (snapshots), Datomic (persistent index plus log tail), Kafka (checkpoint plus changelog delta), Kurrent (checkpoint with read model) all persist a checkpoint or index to bound startup. Spec persists none and re-folds all receipts at open (853 ms at her size per the spec's probe). It states a snapshot file would be "a second truth".
- Paging: Datomic and the on-disk stores page segments; Fowler requires RAM above data size. Spec holds all decoded rows, postings and closers in RAM (RSS 637 MB at base; accepted bound is "at or below base"); no paging path in the spec, and 10x tests are in the acceptance table.
- Snapshot reads: redb and LMDB give a snapshot of the newest committed state at txn begin and pin pages while open; the spec has one image at head and an `as_of` ordinal filter, and the daemon is serial so no concurrent reader holds a snapshot. Datomic's as-of is a filter over indexes; the spec's is a visibility filter plus status rebuild for rows whose status changed after as_of.
- Status at as_of: Datomic as-of filters datoms uniformly. The spec's rows carry derived status that depends on later resolutions, so it carries `status_changed` and a patched arena; no cited system's page describes this mechanism.
- Postings narrow only when as_of == head for status aliases (InForce, Ran); Datomic indexes are time-agnostic and filtered after.
- Atomicity of derived state and its position: Kafka's KIP-1035 and Kurrent put checkpoint offset and state in one atomic unit. In the spec the image's head is derived in the same fold as its rows and nothing is persisted, so the question does not arise on disk; the in-memory advance after a landed CAS that fails mid-fold calls refresh.
- Whole-image rebuild on lost swap: the spec rebuilds fully (126-145 ms per base probe); Datomic and Kafka apply deltas or tail.
- Back pressure: Datomic throttles the writer when the memory index outgrows its max; the spec has no such bound on held state.

## redb read transactions: does MVCC already give the snapshot the spec builds?

Facts only.
- redb's read txn gives a consistent point-in-time view of the stored tables at the newest commit when begun, usable concurrently with writes, with pages pinned until dropped (design.md; docs.rs).
- What it hands out is the stored tables: receipts and document envelopes as bytes. It holds no decoded rows, status, postings, closers or citers; the spec's costs (351 ms receipt decode, 807 ms Reader::at, 512 ms rows at base) are derivation over those bytes, none of which redb's snapshot stores [from the spec's probe figures].
- A redb txn is a snapshot at one commit, the newest at begin time; reads at earlier commits need savepoints (persistent ones are stored in the file with an allocator-state copy, about 64 kB per 1 GB of values). The spec's `as_of` is a receipt ordinal; redb's snapshots are not addressed by receipt ordinal.
- The spec's store is accessed via a CAS store trait (`store.rs`, MemoryStore CAS, compare_and_swap_batch); whether redb's read txn is reachable through that trait was not examined here (Huginn checkout not touched).
- The spec's stated reason for no MVCC layer is the daemon's serial loop (`keeps_moves`: "no RwLock, Arc or MVCC layer is added"). Whether the daemon ever holds a read during a write was not examined.
- Not stated in sources read: redb read-txn start cost, page-cache size/eviction, whether open readers affect file growth the way LMDB documents.
