# Test Determinism and Threading Analysis

Scope: reviewed all Rust unit tests under `src/` (`app.rs`, `config.rs`, `executor.rs`, `history.rs`, `ops.rs`, `peek.rs`, `safety.rs`, `scope.rs`).

- `executor.rs`, `safety.rs`, `peek.rs`, `history.rs`, `ops.rs`, `app.rs`: tests use temp dirs, stub executors/LLM clients, and local data only. Assertions avoid ordering assumptions (e.g., glob results) and do not share mutable global state. No threading or timing, so they are deterministic.
- `scope.rs`: tests change the process current directory but wrap changes in a static `Mutex`, preventing races when tests run in parallel. Directory listings are sorted, so ordering is deterministic.
- `config.rs:223 env_override_takes_precedence`: now wrapped in a static `Mutex` to serialize env var mutation, removing the prior nondeterminism risk when tests run in parallel.

Implications for the app
- Tests no longer exhibit threading race risks. Runtime CLI reads env vars in a single-threaded path, so previous test concern did not imply an app bug; concurrent callers of `resolve_ai_config` in the same process would still contend on global environment state, which is expected for process-wide env access.

Suggestions to address identified issue
1) Keep using the env-var mutex guard in `config.rs` tests (or adopt a scoped env helper) to ensure future tests that touch env vars remain deterministic when run in parallel.
