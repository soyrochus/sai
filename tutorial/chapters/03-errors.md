# Chapter 3 — Errors are part of the design

## Product goal

Reject blank requests with a useful diagnostic and a non-zero process exit instead of pretending the run succeeded.

## Rust concepts

- `Result<T, E>` as an outcome type.
- The `?` operator.
- Early return and validation.
- Adding context to errors.
- Separating user errors from panics.

## Add the error dependency

This course uses `anyhow` at the application boundary because the CLI mainly needs contextual error reports rather than a public typed-error API:

```bash
cargo add anyhow
```

Libraries consumed by other crates often benefit from a dedicated error enum. That is not necessary yet.

## AI collaboration script

```text
Before editing, enumerate failure cases for the current one-argument Rust CLI. Then implement only blank-request validation using anyhow::Result. Explain why panic!, unwrap(), and process::exit inside app::run are worse boundaries for this case.
```

Do not let the assistant add model or configuration errors yet.

## Build

Change `app::run`:

```rust
use crate::cli::Cli;
use anyhow::{Result, bail};

pub fn run(cli: Cli) -> Result<RunSummary> {
    let request = cli.request.trim();
    if request.is_empty() {
        bail!("request must not be blank");
    }

    println!("Request: {request}");
    Ok(RunSummary {
        request: request.to_string(),
        generated_command: None,
        exit_code: 0,
    })
}
```

Change `main`:

```rust
fn main() {
    match app::run(Cli::parse()) {
        Ok(summary) => std::process::exit(summary.exit_code),
        Err(error) => {
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
    }
}
```

The application returns errors; the outermost process boundary chooses how to display them and which exit status to use.

When a fallible operation needs context, use:

```rust
use anyhow::Context;

let text = std::fs::read_to_string(path)
    .with_context(|| format!("failed to read {}", path.display()))?;
```

The low-level error is preserved. The context explains what the application was trying to do.

## Compiler conversation

Changing `run` to return `Result<RunSummary>` makes the previous caller fail:

```text
no field `exit_code` on type `Result<RunSummary, anyhow::Error>`
```

The compiler is saying the caller ignored one possible state. You must match, use `?` from another fallible function, or deliberately unwrap in a test where failure should panic.

Do not fix this by returning a summary with exit code 1 for every internal failure. That erases the error's causal information.

## Tests

Update the success test to unwrap the expected `Ok` value, then add:

```rust
#[test]
fn blank_request_is_rejected() {
    let error = run(Cli {
        request: "  \n\t".into(),
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "request must not be blank");
}
```

Also test that surrounding whitespace has an explicit policy:

```rust
#[test]
fn surrounding_whitespace_is_trimmed() {
    let summary = run(Cli {
        request: "  list files  ".into(),
    })
    .unwrap();
    assert_eq!(summary.request, "list files");
}
```

Whether trimming is correct is a product decision. The test records it.

## Review checklist

- Can a user-caused error occur without a panic?
- Does the error explain the failed operation?
- Is the original cause preserved when wrapping I/O errors?
- Does only the process boundary call `process::exit`?
- Are whitespace semantics deliberate and tested?

## Checkpoint

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- "list files"
cargo run -- "   "
```

The second run must print a clear error and exit non-zero.

```bash
git add Cargo.toml Cargo.lock src
git commit -m "Tutorial 03: explicit errors"
git tag tutorial-03-errors
```

## Stretch exercise

Create a `Prompt` newtype with a fallible constructor. Compare enforcing nonblank text at construction with checking inside `run`.

## Reflection

AI often writes only the happy path unless asked for failure cases. Rust's `Result` forces the application to represent failure, but humans still decide which failures matter and how much context users need.

## Further learning

- [The Rust Book — Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html) — panic vs. `Result`, the decision this chapter makes concrete.
- [The Rust Book — Recoverable Errors with `Result`](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- [Comprehensive Rust — `anyhow`](https://google.github.io/comprehensive-rust/error-handling/anyhow.html) — the exact crate this chapter adds, explained on its own.
- [Rust by Example — `Result`](https://doc.rust-lang.org/rust-by-example/error/result.html) — a quick reference once the pattern feels routine.
- [Rustlings — `13_error_handling`](https://github.com/rust-lang/rustlings/tree/main/exercises/13_error_handling)

Next: [The first model call](04-first-model-call.md).
