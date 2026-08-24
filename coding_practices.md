```xml
<?xml version="1.0" encoding="UTF-8"?>
<rust_engineering_practices version="2.0">
  <metadata>
    <scope>Generic production Rust: services, systems software, command-line tools, embedded targets, protocols, and performance-sensitive paths.</scope>
    <objective>Correct, secure, deterministic, reviewable, resource-bounded software with measured high performance and low operational cost.</objective>
    <authority>Engineering evidence outranks organization names, folklore, cleverness, and benchmark-free claims.</authority>
    <priorities>
      <P0>Required for correctness, safety, security, or operational survival.</P0>
      <P1>Production default; exceptions require documented evidence.</P1>
      <P2>Apply only after profiling identifies a relevant bottleneck.</P2>
    </priorities>
  </metadata>

  <practices>
    <practice id="RP-01" priority="P0" name="Correctness before speed">
      <rule>Preserve observable behavior, memory safety, security invariants, and failure semantics before reducing latency, memory, instructions, allocations, or binary size.</rule>
      <requirements>
        <requirement>State invariants, ownership, bounds, units, and failure behavior at trust boundaries.</requirement>
        <requirement>Prefer the simplest implementation that meets measured requirements.</requirement>
        <requirement>Never remove validation, overflow checks, synchronization, or error handling without proving it redundant.</requirement>
      </requirements>
    </practice>

    <practice id="RP-02" priority="P0" name="Measure before optimizing">
      <rule>Optimize measured bottlenecks against a written workload and budget.</rule>
      <requirements>
        <requirement>Define relevant targets: latency percentiles, throughput, CPU, resident memory, allocations, startup, binary size, power, or wakeups.</requirement>
        <requirement>Benchmark production-equivalent release binaries with representative inputs, concurrency, duration, and hardware.</requirement>
        <requirement>Record toolchain, target, flags, hardware, data, warm-up, samples, variance, and baseline.</requirement>
        <requirement>Use profiles, flame graphs, hardware counters, allocation tracing, and syscall tracing as appropriate.</requirement>
        <requirement>Reject improvements within noise or changes that move cost to an unmeasured resource.</requirement>
      </requirements>
      <verification>A reproducible report accompanies every performance claim.</verification>
    </practice>

    <practice id="RP-03" priority="P1" name="Architecture before instructions">
      <rule>Fix asymptotic complexity, data flow, batching, duplicated work, and contention before micro-optimizing instructions.</rule>
      <requirements>
        <requirement>Document time and space complexity for operations that scale with input or load.</requirement>
        <requirement>Avoid repeated scans, accidental quadratic work, redundant parsing, and needless serialization.</requirement>
        <requirement>Move invariant work out of loops; cache only with explicit bounds, invalidation, and memory cost.</requirement>
      </requirements>
    </practice>

    <practice id="RP-04" priority="P0" name="Bound every resource">
      <rule>Memory, input, queues, tasks, threads, descriptors, retries, recursion, caches, batches, and work per request have explicit limits.</rule>
      <requirements>
        <requirement>Validate externally controlled sizes before allocation or expensive work.</requirement>
        <requirement>Use bounded channels, pools, collections, and caches with explicit overflow behavior.</requirement>
        <requirement>Use monotonic deadlines, retry ceilings, backpressure, and load shedding instead of unbounded growth.</requirement>
      </requirements>
      <verification>Test boundaries, saturation, timeouts, overload, and recovery.</verification>
    </practice>

    <practice id="RP-05" priority="P1" name="Ownership, copies, and allocation">
      <rule>Express ownership through types; borrow instead of copying and allocate deliberately.</rule>
      <requirements>
        <requirement>Prefer references, slices, iterators, and views when ownership need not transfer.</requirement>
        <requirement>Use RAII for files, locks, mappings, temporary state, and cleanup.</requirement>
        <requirement>Do not use Clone to avoid resolving ownership; justify hot-path clones semantically and with measurements.</requirement>
        <requirement>Pre-size from validated bounds and reuse buffers when measured cost falls.</requirement>
        <requirement>Use stack storage only for small fixed values; never place large or attacker-sized data on the stack.</requirement>
        <requirement>Use arenas, slabs, pools, or small-vector techniques only when lifecycle and benchmarks justify complexity.</requirement>
      </requirements>
      <verification>Measure allocations, copies, peak RSS, fragmentation, and retained capacity.</verification>
    </practice>

    <practice id="RP-06" priority="P2" name="Layout, locality, dispatch, and inlining">
      <rule>Change representation and dispatch only after measuring cache, branch, bandwidth, call, or code-size cost.</rule>
      <requirements>
        <requirement>Separate hot and cold fields and reduce pointer chasing only when access profiles justify it.</requirement>
        <requirement>Choose array-of-structures or structure-of-arrays from traversal patterns.</requirement>
        <requirement>Use static dispatch for small stable hot paths; use trait objects where flexibility or reduced monomorphization wins.</requirement>
        <requirement>Do not force inlining globally; check runtime and text size because excessive inlining harms instruction-cache locality.</requirement>
        <requirement>Use repr(C), alignment, packing, or transmutation only for a specified ABI or proven requirement.</requirement>
        <requirement>Consider false sharing for contended state; pad only after measurement.</requirement>
      </requirements>
      <verification>Use generated-code inspection and hardware counters only after source-level profiling identifies the issue.</verification>
    </practice>

    <practice id="RP-07" priority="P0" name="Integer and conversion safety">
      <rule>Lengths, offsets, counters, indexes, units, and narrowing conversions have explicit ranges and overflow semantics.</rule>
      <requirements>
        <requirement>Use checked arithmetic and TryFrom at trust, indexing, and allocation boundaries.</requirement>
        <requirement>Use wrapping, saturating, or overflowing arithmetic only when deliberately required.</requirement>
        <requirement>Use newtypes where identifiers or units could be mixed.</requirement>
        <requirement>Avoid as for potentially lossy integer conversion unless truncation is local, obvious, intended, and tested.</requirement>
      </requirements>
    </practice>

    <practice id="RP-08" priority="P0" name="Errors, panic, and safe state">
      <rule>Expected failures are values; panic and process termination are explicit deployment policies.</rule>
      <requirements>
        <requirement>Use Result and Option with precise context at subsystem boundaries.</requirement>
        <requirement>Avoid unwrap, expect, unchecked indexing, and unreachable assumptions on externally influenced paths.</requirement>
        <requirement>Use assertions for programmer invariants, not input validation.</requirement>
        <requirement>Define unwind, abort, supervisor restart, shutdown, and safe-state restoration behavior.</requirement>
        <requirement>Do not rely only on Drop where abort, SIGKILL, power loss, or kernel failure matters.</requirement>
      </requirements>
      <verification>Inject I/O errors, short operations, corruption, exhaustion, cancellation, and restart.</verification>
    </practice>

    <practice id="RP-09" priority="P0" name="Contain unsafe Rust">
      <rule>Safe Rust is default; unsafe code is minimal, isolated, documented, reviewed, and tested as a separate correctness boundary.</rule>
      <requirements>
        <requirement>Enable unsafe_op_in_unsafe_fn so every unsafe operation is lexically explicit.</requirement>
        <requirement>Every unsafe block has a SAFETY comment listing required invariants and why they hold.</requirement>
        <requirement>Expose a narrow safe interface that validates preconditions.</requirement>
        <requirement>Audit provenance, alignment, initialization, aliasing, lifetimes, layout, concurrency, unwinding, and ownership.</requirement>
        <requirement>Do not use unsafe to remove checks without profile and generated-code evidence.</requirement>
      </requirements>
      <verification>Use Miri, sanitizers, fuzzing, architecture tests, and expert review where applicable.</verification>
    </practice>

    <practice id="RP-10" priority="P0" name="FFI, syscalls, assembly, and hardware">
      <rule>Foreign boundaries are untrusted contracts with explicit ABI, encoding, alignment, lifetime, partial-I/O, and error rules.</rule>
      <requirements>
        <requirement>Prefer maintained platform bindings; hand-written constants require authoritative source and target documentation.</requirement>
        <requirement>Handle errno, interruption, short reads and writes, partial state, termination, and device ranges.</requirement>
        <requirement>Use the representation required by the ABI; binary is not faster or correct when a boundary requires text.</requirement>
        <requirement>Confine raw pointers and assembly to the smallest architecture-specific module.</requirement>
        <requirement>Assembly declares inputs, outputs, clobbers, stack alignment, and control flow correctly.</requirement>
      </requirements>
      <verification>Test every supported architecture and inspect entry/ABI code for freestanding programs.</verification>
    </practice>

    <practice id="RP-11" priority="P0" name="Concurrency and atomics">
      <rule>Shared state has a written ownership and synchronization invariant; lock-free is not a default optimization.</rule>
      <requirements>
        <requirement>Prefer immutability, partitioning, single ownership, or message passing before shared mutation.</requirement>
        <requirement>Bound workers and channels; avoid thread-per-connection designs at high connection counts.</requirement>
        <requirement>Keep critical sections short and perform no blocking I/O or await while holding a synchronous lock.</requirement>
        <requirement>Justify every atomic ordering by the relation it establishes; Relaxed is for truly independent state.</requirement>
        <requirement>Use established primitives unless lock-free code has measured value and a reviewed proof.</requirement>
      </requirements>
      <verification>Use stress, overload, shutdown, race, and model tests such as Loom where practical.</verification>
    </practice>

    <practice id="RP-12" priority="P0" name="Async and cancellation">
      <rule>Async operations are bounded, cancellable, and safe if dropped at every await point.</rule>
      <requirements>
        <requirement>Do not block executor threads; use bounded blocking workers.</requirement>
        <requirement>Define cancellation semantics for partial writes, transactions, and state transitions.</requirement>
        <requirement>Apply deadlines to network, storage, locks, and downstream calls.</requirement>
        <requirement>No detached task lacks ownership, limits, completion observation, or shutdown behavior.</requirement>
      </requirements>
    </practice>

    <practice id="RP-13" priority="P1" name="I/O and observability economy">
      <rule>Reduce copies, syscalls, wakeups, and diagnostics overhead without creating unbounded latency or memory.</rule>
      <requirements>
        <requirement>Batch and buffer only to explicit size and time limits.</requirement>
        <requirement>Correctly handle short I/O and backpressure before adding vectored, asynchronous, or zero-copy I/O.</requirement>
        <requirement>Do not poll faster than the controlled system can change or requirements demand.</requirement>
        <requirement>Use structured, rate-limited diagnostics; do not allocate or format disabled hot-path logs.</requirement>
      </requirements>
      <verification>Measure syscalls, bytes copied, wakeups, CPU per operation, tail latency, and telemetry observer effect.</verification>
    </practice>

    <practice id="RP-14" priority="P1" name="Dependencies and abstractions earn their cost">
      <rule>Each dependency and abstraction must add more correctness, portability, maintenance value, or measured speed than its total cost.</rule>
      <requirements>
        <requirement>Avoid speculative traits, wrappers, factories, macros, and feature flags.</requirement>
        <requirement>Review transitive dependencies, default features, build scripts, proc macros, native code, licenses, and advisories.</requirement>
        <requirement>Prefer audited ecosystem primitives over bespoke cryptography, parsers, synchronization, and allocators.</requirement>
        <requirement>Use no_std or a freestanding runtime only when deployment constraints justify lost facilities and added unsafe surface.</requirement>
      </requirements>
    </practice>

    <practice id="RP-15" priority="P0" name="Verification and regression prevention">
      <rule>Compilation is one check, not proof; combine tools that detect different defect classes.</rule>
      <requirements>
        <requirement>CI runs rustfmt, compiler warnings, reviewed Clippy lints, unit, integration, and documentation tests.</requirement>
        <requirement>Use property tests and fuzzing for parsers, codecs, state machines, and untrusted input.</requirement>
        <requirement>Use Miri for supported unsafe/aliasing checks and sanitizers for native integration.</requirement>
        <requirement>Test supported architectures, endianness, features, and minimum Rust version when contractual.</requirement>
        <requirement>Add a regression test for each production defect, especially startup, shutdown, recovery, and ABI defects.</requirement>
      </requirements>
    </practice>

    <practice id="RP-16" priority="P0" name="Reproducible builds and supply chain">
      <rule>Every deployed artifact is traceable to reviewed source, dependencies, toolchain, target, profile, and configuration.</rule>
      <requirements>
        <requirement>Pin toolchains where reproducibility matters; commit Cargo.lock for deployed applications and build with --locked.</requirement>
        <requirement>Audit advisories, licenses, sources, duplicates, build scripts, proc macros, linkers, and native libraries.</requirement>
        <requirement>Record artifact hashes and retain external debug symbols when stripping production binaries.</requirement>
      </requirements>
    </practice>

    <practice id="RP-17" priority="P1" name="Build profiles are workload decisions">
      <rule>No compiler configuration is universally fastest, smallest, and safest.</rule>
      <requirements>
        <requirement>Use release builds for performance conclusions.</requirement>
        <requirement>Measure opt-level, thin/fat LTO, codegen units, panic strategy, stripping, overflow checks, debug information, and CPU target.</requirement>
        <requirement>Use target-cpu=native only for artifacts guaranteed to run on compatible CPUs.</requirement>
        <requirement>Retain overflow checks where ranges are unproven or failure is security-relevant; measure their cost.</requirement>
        <requirement>Optimize for size when instruction-cache, storage, startup, or embedded constraints make size a performance property.</requirement>
      </requirements>
    </practice>

    <practice id="RP-18" priority="P1" name="Minimal and reviewable code">
      <rule>Another engineer must be able to verify invariants and cost without reverse engineering.</rule>
      <requirements>
        <requirement>Keep functions focused, control flow shallow, names precise, and mutation local.</requirement>
        <requirement>Comments explain why, safety, units, hardware behavior, and tradeoffs, not syntax.</requirement>
        <requirement>Delete dead code, speculative generality, stale compatibility, and unused configuration.</requirement>
        <requirement>Minimize conceptual and runtime cost, not source lines; low-level or clever is not automatically efficient.</requirement>
      </requirements>
    </practice>
  </practices>

  <recommended_baseline>
    <lint_policy>
      <requirement>Enable unsafe_op_in_unsafe_fn and require safety documentation for public unsafe APIs.</requirement>
      <requirement>Deny warnings in controlled application CI; libraries avoid lint policy that unnecessarily breaks downstream builds.</requirement>
      <requirement>Adopt Clippy lints through review; never apply machine suggestions blindly.</requirement>
    </lint_policy>
    <application_release_profile>
      <setting name="opt-level">3 for throughput; s or z only when measured size/cache results win.</setting>
      <setting name="lto">thin baseline; fat only after runtime and build-cost measurement.</setting>
      <setting name="codegen-units">1 when cross-module optimization justifies slower builds.</setting>
      <setting name="panic">abort only when unwinding is unnecessary and process recovery is defined.</setting>
      <setting name="strip">strip deployed symbols while retaining separate diagnostics symbols when needed.</setting>
      <setting name="overflow-checks">true for safety-sensitive arithmetic unless proofs and measurements justify otherwise.</setting>
    </application_release_profile>
    <minimum_ci>
      <command>cargo fmt --all -- --check</command>
      <command>cargo clippy --workspace --all-targets --all-features --locked -- -D warnings</command>
      <command>cargo test --workspace --all-features --locked</command>
      <command>cargo doc --workspace --all-features --no-deps --locked</command>
      <command>cargo audit or cargo deny check under project policy</command>
    </minimum_ci>
  </recommended_baseline>

  <abhigram_profile baseline_commit="e17d8ec99bbb025ed1b281994aae6c10e263b9b5">
    <scope>DCP/BAP, DCP-PQ E2EE, realtime server, persistence, native clients, and browser/WASM clients.</scope>
    <requirements>
      <requirement id="AG-01" priority="P0">Every frame, decompressed output, queue, batch, retry/replay set, skipped-key set, handshake set, and in-flight operation has a hard bound checked before allocation or cryptographic work.</requirement>
      <requirement id="AG-02" priority="P0">Wire encoding has fixed field order, explicit width and endianness, canonical identifiers, exact lengths, versioning, and cross-implementation golden vectors.</requirement>
      <requirement id="AG-03" priority="P0">Encrypt at the sending edge; routing, persistence, logs, queues, and metrics remain ciphertext-only; reveal authenticated plaintext only at the receiving edge.</requirement>
      <requirement id="AG-04" priority="P0">Use audited constant-time primitives for secret comparisons; avoid secret-dependent branches and indexes.</requirement>
      <requirement id="AG-05" priority="P0">Secret-bearing types restrict Clone and Debug and wipe material where required; temporary extractable browser key bytes are wiped after import.</requirement>
      <requirement id="AG-06" priority="P0">Nonce, epoch, sequence, retry, rollback, persistence, skipped-key, and replay rules are explicit, durable, transactional, and tested.</requirement>
      <requirement id="AG-07" priority="P0">Version, suite, session, ratchet state, counters, nonce metadata, and security-relevant lengths are canonically authenticated as AEAD AAD.</requirement>
      <requirement id="AG-08" priority="P1">Measured sealed paths parse borrowed bytes, use in-place AEAD when safe, pre-size validated output, and avoid redundant serialization.</requirement>
      <requirement id="AG-09" priority="P0">Key-bearing state has a single-owner or explicitly synchronized model; atomic orderings document their invariant.</requirement>
      <requirement id="AG-10" priority="P0">Handshake, authentication, PQ, persistence, delivery, reconnect, media, and signaling work has deadlines, caps, backpressure, and non-amplifying retries.</requirement>
      <requirement id="AG-11" priority="P0">Provider, algorithm, wire-visible dependency, build script, proc macro, and native dependency changes receive security and interoperability review.</requirement>
    </requirements>
    <verification>Golden vectors, tamper/replay/rollback tests, malformed-input fuzzing, overload, reconnect storms, persistence backpressure, native/WASM interoperability, and production-equivalent benchmarks are release gates.</verification>
  </abhigram_profile>

  <optimization_workflow>
    <step index="1">Specify behavior, invariants, workload, limits, and budgets.</step>
    <step index="2">Build a correct, simple, bounded implementation in safe Rust.</step>
    <step index="3">Create correctness tests and a reproducible release benchmark.</step>
    <step index="4">Profile CPU, memory, allocation, locks, I/O, syscalls, cache, and tail latency relevant to the workload.</step>
    <step index="5">Fix architecture and algorithms before allocation, layout, dispatch, instruction, or unsafe tuning.</step>
    <step index="6">Change one cost center, compare to baseline, and retain only statistically meaningful wins.</step>
    <step index="7">Run correctness, safety, overload, recovery, portability, and performance-regression gates.</step>
    <step index="8">Document the tradeoff and remove experiments that did not win.</step>
  </optimization_workflow>

  <release_gate>
    <gate priority="P0">Externally influenced resources and expensive operations are bounded.</gate>
    <gate priority="P0">No unexplained unsafe block, unchecked narrowing, overflow assumption, FFI contract, or atomic ordering remains.</gate>
    <gate priority="P0">Error, cancellation, startup, shutdown, partial-I/O, exhaustion, and recovery behavior is defined and tested.</gate>
    <gate priority="P0">Security and protocol invariants pass adversarial and interoperability tests.</gate>
    <gate priority="P1">Performance meets written budgets in reproducible production-equivalent benchmarks.</gate>
    <gate priority="P1">Build profile, target CPU, dependencies, and artifact provenance are recorded.</gate>
    <gate priority="P1">No optimization remains only because it appears clever or low-level.</gate>
  </release_gate>
</rust_engineering_practices>
```
