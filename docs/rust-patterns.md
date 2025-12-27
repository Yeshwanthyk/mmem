# Rust Patterns (from ripgrep + parking_lot + crossbeam)

This document distills Rust design and implementation patterns observed in `/tmp/ripgrep`, `/tmp/parking_lot`, and `/tmp/crossbeam`. It is intended as a blueprint for agentic coding tools that need proven, production-grade patterns.

**Architecture**
- Use a facade crate to expose a stable API while delegating to focused sub-crates (e.g., `grep` over matcher/searcher/printer). Refs: `crates/grep/README.md`, `crates/searcher/README.md`, `crates/printer/README.md`, `crates/matcher/README.md`, `crates/regex/README.md`.
- Keep CLI glue separate from reusable libraries; treat core as integration, not public API. Refs: `crates/core/README.md`.
- Enforce single-responsibility crates (searching, printing, matching, walking, globbing, CLI utilities). Refs: `crates/searcher/README.md`, `crates/printer/README.md`, `crates/matcher/README.md`, `crates/regex/README.md`, `crates/ignore/README.md`, `crates/globset/README.md`.
- Use Cargo feature flags to keep optional dependencies and engines off the hot path by default. Refs: `crates/grep/README.md`, `crates/globset/README.md`.
- Explicitly warn about unstable internal APIs to steer users toward the facade crate. Refs: `crates/searcher/README.md`, `crates/printer/README.md`, `crates/matcher/README.md`, `crates/regex/README.md`.
- Split core synchronization engine from high-level primitives to isolate breaking changes and keep a reusable low-level API. Refs: `/tmp/parking_lot/README.md`, `/tmp/parking_lot/core/src/lib.rs`.
- Provide a type-safe lock API layer (`no_std` friendly) that wraps raw locks. Refs: `/tmp/parking_lot/lock_api/src/lib.rs`.
- Re-export subcrates through a lightweight umbrella crate to keep modularity with a cohesive surface. Refs: `/tmp/crossbeam/README.md`, `/tmp/crossbeam/src/lib.rs`.
- Mark `no_std`/`alloc` support explicitly in feature lists to clarify deployment constraints. Refs: `/tmp/crossbeam/README.md`, `/tmp/crossbeam/crossbeam-utils/README.md`.

**API Shape**
- Prefer trait-based abstraction for pluggable engines (e.g., `Matcher` trait). Refs: `crates/matcher/src/lib.rs`.
- Use internal iteration (push model) when external iteration is too restrictive for performance or ergonomics. Refs: `crates/matcher/src/lib.rs`, `crates/searcher/src/sink.rs`.
- Introduce newtypes to enforce invariants and hide representation details (`Match`, `LineTerminator`, `BinaryDetection`, `Encoding`). Refs: `crates/matcher/src/lib.rs`, `crates/searcher/src/searcher/mod.rs`.
- Represent strategy choices as enums to keep behavior explicit and optimizable (`MatchStrategy`, `MmapChoice`, `BinaryDetection`). Refs: `crates/globset/src/glob.rs`, `crates/searcher/src/searcher/mmap.rs`, `crates/searcher/src/searcher/mod.rs`.
- Provide explicit control signals for traversal/iteration (`WalkState`, visitor traits). Refs: `crates/ignore/src/walk.rs`.
- Parameterize lock wrappers by raw lock traits and expose optional capabilities via extension traits. Refs: `/tmp/parking_lot/lock_api/src/lib.rs`, `/tmp/parking_lot/lock_api/src/mutex.rs`, `/tmp/parking_lot/lock_api/src/rwlock.rs`.
- Encode guard Send-ness in marker types and forbid incompatible feature combos at compile time. Refs: `/tmp/parking_lot/lock_api/src/lib.rs`, `/tmp/parking_lot/src/lib.rs`.

**Builders & Config**
- Use builder + internal `Config` to keep construction explicit and freeze config at build time. Refs: `crates/searcher/src/searcher/mod.rs`, `crates/printer/src/json.rs`.
- Provide `Default` for safe, conservative settings; make high-risk options opt-in. Refs: `crates/searcher/src/searcher/mod.rs`, `crates/searcher/src/searcher/mmap.rs`.
- Expose builder APIs for complex domain config (`WalkBuilder`, `GlobBuilder`, `GlobSetBuilder`, JSON printer, decompression). Refs: `crates/ignore/src/walk.rs`, `crates/globset/src/lib.rs`, `crates/printer/src/json.rs`, `crates/cli/src/decompress.rs`.
- Use `Arc` inside config to enable cheap clones while keeping immutability. Refs: `crates/printer/src/json.rs`, `crates/ignore/src/walk.rs`.
- Support incremental setup on builders before `build` (e.g., `CommandReaderBuilder`, `DecompressionMatcherBuilder`). Refs: `crates/cli/src/process.rs`, `crates/cli/src/decompress.rs`.

**Error Handling**
- Model errors as rich enums with context wrappers (`WithPath`, `WithLineNumber`, `WithDepth`). Refs: `crates/ignore/src/lib.rs`.
- Aggregate partial failures to keep operating when possible (`PartialErrorBuilder`). Refs: `crates/ignore/src/lib.rs`.
- Use `#[non_exhaustive]` on error kinds to allow future expansion without breaking APIs. Refs: `crates/regex/src/error.rs`, `crates/globset/src/lib.rs`.
- Implement `From` conversions for I/O and domain errors to reduce boilerplate. Refs: `crates/regex/src/error.rs`, `crates/cli/src/process.rs`.
- Make error type generic for callback APIs via an error trait (`SinkError`). Refs: `crates/searcher/src/sink.rs`.

**Streaming & Iteration**
- Push results to callers via callback traits to keep control in the engine (`Sink`). Refs: `crates/searcher/src/sink.rs`.
- Favor internal iteration for matcher/searcher integration when external iteration is too limiting. Refs: `crates/matcher/src/lib.rs`, `crates/searcher/src/sink.rs`.
- Build streaming process readers that avoid pipe deadlocks by reading stderr asynchronously. Refs: `crates/cli/src/process.rs`.
- Reuse buffers with interior mutability to minimize allocations during repeated searches. Refs: `crates/searcher/src/searcher/mod.rs`.

**Concurrency & Parallelism**
- Use work-stealing deques for scalable parallel tree walks. Refs: `crates/ignore/src/walk.rs`.
- Track worker lifecycle with atomics to avoid lock contention in hot paths. Refs: `crates/ignore/src/walk.rs`.
- Construct per-thread visitors via builder traits to avoid sharing non-Send state. Refs: `crates/ignore/src/walk.rs`.
- Provide explicit `Skip`/`Quit` semantics with documented async quit behavior. Refs: `crates/ignore/src/walk.rs`.
- Abstract thread parking behind a platform-specific trait with cfg-driven implementations. Refs: `/tmp/parking_lot/core/src/thread_parker/mod.rs`, `/tmp/parking_lot/core/src/thread_parker/linux.rs`, `/tmp/parking_lot/core/src/thread_parker/windows/mod.rs`.
- Provide work-stealing queues with Worker/Stealer/Injector roles and batch stealing. Refs: `/tmp/crossbeam/crossbeam-deque/src/deque.rs`, `/tmp/crossbeam/crossbeam-deque/src/lib.rs`.
- Use scoped threads to borrow stack data safely without Arc or static lifetimes. Refs: `/tmp/crossbeam/crossbeam-utils/src/thread.rs`, `/tmp/crossbeam/README.md`.
- Use sharded locks and wait groups as reusable building blocks for read-heavy and join-style concurrency. Refs: `/tmp/crossbeam/crossbeam-utils/src/sync/sharded_lock.rs`, `/tmp/crossbeam/crossbeam-utils/src/sync/wait_group.rs`.

**Locking & Synchronization**
- Use a hash table keyed by lock address with per-bucket locks for parked threads; never free old tables to allow lock-free reads. Refs: `/tmp/parking_lot/core/src/parking_lot.rs`.
- Use park/unpark tokens for direct handoff to avoid unlock-park-lock cycles. Refs: `/tmp/parking_lot/core/src/parking_lot.rs`.
- Implement a two-queue RwLock (writers at `addr + 1`) to avoid writer starvation. Refs: `/tmp/parking_lot/src/raw_rwlock.rs`.
- Provide explicit fairness hooks (`bump`, `unlock_fair`) to enforce eventual fairness. Refs: `/tmp/parking_lot/src/raw_mutex.rs`, `/tmp/parking_lot/src/raw_rwlock.rs`.
- Pack `Once` state into bit flags with poison handling on panic. Refs: `/tmp/parking_lot/src/once.rs`.
- Requeue `Condvar::notify_all` waiters onto the mutex to avoid thundering herd. Refs: `/tmp/parking_lot/src/condvar.rs`.
- Expose a minimal Parker/Unparker primitive for building custom blocking abstractions. Refs: `/tmp/crossbeam/crossbeam-utils/src/sync/parker.rs`.

**Lock-Free & Memory Reclamation**
- Use epoch-based GC with pinning guards and global epoch advancement to defer destruction safely. Refs: `/tmp/crossbeam/crossbeam-epoch/src/lib.rs`, `/tmp/crossbeam/crossbeam-epoch/src/guard.rs`, `/tmp/crossbeam/crossbeam-epoch/src/epoch.rs`, `/tmp/crossbeam/crossbeam-epoch/src/internal.rs`.
- Use tagged pointers to store metadata in alignment bits with helper compose/decompose APIs. Refs: `/tmp/crossbeam/crossbeam-epoch/src/atomic.rs`.
- Collect deferred reclamation in per-thread bags with small inline storage to avoid allocations. Refs: `/tmp/crossbeam/crossbeam-epoch/src/deferred.rs`, `/tmp/crossbeam/crossbeam-epoch/src/internal.rs`.
- Prevent ABA in bounded queues via stamped indices combining lap and slot. Refs: `/tmp/crossbeam/crossbeam-queue/src/array_queue.rs`.
- Implement unbounded queues with segmented blocks and state flags for safe reuse/destruction. Refs: `/tmp/crossbeam/crossbeam-queue/src/seg_queue.rs`.
- Use lock-free skip lists with mark-and-help deletion and probabilistic tower heights. Refs: `/tmp/crossbeam/crossbeam-skiplist/src/base.rs`, `/tmp/crossbeam/crossbeam-skiplist/src/map.rs`, `/tmp/crossbeam/crossbeam-skiplist/src/set.rs`.

**Channels & Coordination**
- Provide MPMC channels with bounded, unbounded, and rendezvous flavors behind a unified API. Refs: `/tmp/crossbeam/crossbeam-channel/src/channel.rs`, `/tmp/crossbeam/crossbeam-channel/src/flavors/mod.rs`.
- Implement `select!` with a trait-based handle, per-thread context, and explicit registration/unregistration. Refs: `/tmp/crossbeam/crossbeam-channel/src/select.rs`, `/tmp/crossbeam/crossbeam-channel/src/select_macro.rs`, `/tmp/crossbeam/crossbeam-channel/src/context.rs`.
- Use wakers to register blocked operations and notify on readiness; wrap with SyncWaker for shared use. Refs: `/tmp/crossbeam/crossbeam-channel/src/waker.rs`.
- Add timer/utility channels (`tick`, `after`, `never`) for scheduling and control flow. Refs: `/tmp/crossbeam/crossbeam-channel/src/flavors/tick.rs`, `/tmp/crossbeam/crossbeam-channel/src/flavors/at.rs`, `/tmp/crossbeam/crossbeam-channel/src/flavors/never.rs`.

**Performance & Memory**
- Precompute fast-path match strategies (literal, prefix, suffix, extension) before regex. Refs: `crates/globset/src/glob.rs`.
- Batch-compile patterns into regex sets for multi-glob matching. Refs: `crates/globset/src/lib.rs`.
- Use byte-oriented `Cow<[u8]>` for zero-copy path handling and non-UTF8 support. Refs: `crates/globset/src/lib.rs`.
- Allow configurable heap limits and strategy selection for large inputs (mmap vs buffer). Refs: `crates/searcher/src/searcher/mod.rs`, `crates/searcher/src/searcher/mmap.rs`.
- Compile out logging when disabled via feature-gated macros. Refs: `crates/globset/src/lib.rs`.
- Optimize uncontended paths to a single atomic op and keep lock state compact (1 byte or 1 word). Refs: `/tmp/parking_lot/README.md`.
- Use adaptive spinning with exponential backoff before parking. Refs: `/tmp/parking_lot/core/src/spinwait.rs`, `/tmp/parking_lot/README.md`.
- Use cache-line padding to prevent false sharing in hot data structures. Refs: `/tmp/crossbeam/crossbeam-utils/src/cache_padded.rs`, `/tmp/crossbeam/crossbeam-utils/src/sync/sharded_lock.rs`.
- Use exponential backoff helpers in CAS loops to balance latency and contention. Refs: `/tmp/crossbeam/crossbeam-utils/src/backoff.rs`, `/tmp/crossbeam/crossbeam-queue/src/array_queue.rs`.

**Safety & Unsafe**
- Localize unsafe code and document the safety contract explicitly. Refs: `crates/searcher/src/searcher/mmap.rs`.
- Keep unsafe features opt-in and guarded by safe defaults (e.g., mmap disabled). Refs: `crates/searcher/src/searcher/mmap.rs`, `crates/searcher/src/searcher/mod.rs`.
- Gate platform-specific APIs with `cfg` and expose safe, portable defaults. Refs: `crates/ignore/src/walk.rs`.
- Use type wrappers to keep invariants enforced at construction time. Refs: `crates/matcher/src/lib.rs`, `crates/searcher/src/searcher/mod.rs`.
- Encapsulate unsafe fast paths with debug-mode fallbacks to keep failures visible during development. Refs: `/tmp/parking_lot/src/util.rs`, `/tmp/parking_lot/core/src/util.rs`.

**Encoding & Bytes**
- Make encoding a first-class type with validation and explicit error paths. Refs: `crates/searcher/src/searcher/mod.rs`.
- Treat paths and matched data as bytes to avoid UTF-8 assumptions. Refs: `crates/globset/src/lib.rs`.
- Preserve non-UTF8 data in JSON output by encoding bytes when needed. Refs: `crates/printer/src/json.rs`.
- Separate binary detection policy from search mechanics (`BinaryDetection`). Refs: `crates/searcher/src/searcher/mod.rs`.

**Security & OS Behavior**
- Resolve executables to absolute paths before spawning to avoid CWD hijacks (esp. Windows). Refs: `crates/cli/src/decompress.rs`.
- Detect and document platform-specific behavior (e.g., mmap disabled on macOS). Refs: `crates/searcher/src/searcher/mmap.rs`.
- Use `OnceLock` for lazy, thread-safe global configuration. Refs: `crates/ignore/src/walk.rs`, `crates/ignore/src/gitignore.rs`.
- Favor `Path`/`OsStr` over `String` for cross-platform path handling. Refs: `crates/cli/src/decompress.rs`, `crates/ignore/src/walk.rs`.

**Documentation Discipline**
- Enforce docs on public APIs with `#![deny(missing_docs)]`. Refs: `crates/matcher/src/lib.rs`, `crates/globset/src/lib.rs`, `crates/ignore/src/lib.rs`.
- Provide crate-level docs with runnable examples for core entry points. Refs: `crates/ignore/src/lib.rs`, `crates/globset/src/lib.rs`.
- Use READMEs to set expectations about API stability and intended usage. Refs: `crates/searcher/README.md`, `crates/printer/README.md`, `crates/matcher/README.md`, `crates/regex/README.md`, `crates/grep/README.md`.
- Keep docs close to code to explain invariants and performance trade-offs. Refs: `crates/matcher/src/lib.rs`, `crates/searcher/src/searcher/mod.rs`.
- Document MSRV policy and bump strategy in README compatibility sections. Refs: `/tmp/crossbeam/README.md`, `/tmp/crossbeam/crossbeam-utils/README.md`.
