# Guardrails for Deterministic Tests in This Repo

Audience: codegen/LLM agents contributing tests to `sai`.

- Prefer isolation over global mutation.
  - Avoid mutating process-wide state (env vars, current dir, global statics). If unavoidable, guard with a static `Mutex` or a scoped helper that restores state on drop.
  - Never assume tests run single-threaded; `cargo test` runs in parallel by default.
- Keep filesystem state local and predictable.
  - Use `tempfile::TempDir`/`tempdir` and write within it. Do not touch the real config dir except via test-specific overrides.
  - When assertions depend on directory listings or globbing, sort results before comparing.
- Treat environment variables carefully.
  - Wrap `env::set_var`/`remove_var` in a guard to avoid leaking to other tests (pattern: static mutex + drop guard).
  - Clean up even if assertions fail (scope guards, `drop` impls).
- Avoid timing and randomness.
  - Do not sleep-wait or rely on wall-clock; design pure functions or use deterministic inputs.
  - If randomness is needed, inject a seeded RNG; never use thread-local RNGs with implicit seeds.
- Stub external behavior explicitly.
  - Use lightweight stubs/mocks (as in `RecordingExecutor`, `StubGenerator`, `MockIo`) instead of real processes or network calls.
  - Keep stubs side-effect free and per-test scoped.
- Mind shared resources.
  - If changing CWD, serialize with a mutex and restore after the test.
  - Do not reuse temp directories across tests.
- Assertions: verify outcomes, not incidental details.
  - Check necessary fields, not full serialized blobs unless stable.
  - Avoid relying on nondeterministic ordering from maps/sets; normalize first.

Quick checklist before adding a test
1) Does it mutate global state? Guard it.
2) Does it touch the filesystem? Use temp dirs and deterministic ordering.
3) Any randomness/timing? Replace with deterministic inputs.
4) Are external interactions stubbed and local?
5) Will it still pass with `cargo test -- --test-threads=8`? If uncertain, add synchronization or make it pure.
