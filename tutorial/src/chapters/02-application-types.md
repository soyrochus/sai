# Chapter 2 — Types for a growing application

## Product goal

Separate argument parsing from application behavior and return a typed summary of what happened.

## Rust concepts

- Modules and visibility.
- Struct construction and update syntax.
- `Option<T>` for an absent value.
- Owned outputs versus borrowed inputs.
- Equality derives for tests.

## Design first

`main` currently knows how to parse and how to run. Split those responsibilities:

```text
main → parse Cli → app::run(Cli) → RunSummary
```

The summary is not logging. It is a value that describes the outcome and will later become useful for tests and history.

## AI collaboration script

```text
Compare two designs for separating my Rust CLI parser from application behavior: a run function returning () and one returning a RunSummary. The next chapters will add generated commands and exit codes. Recommend the smallest useful design now. Do not add traits or async code.
```

Ask the assistant why every field exists. Remove fields justified only by vague future possibilities.

## Build

Create `src/cli.rs`:

```rust
use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "sai-course")]
pub struct Cli {
    /// Describe the command you want.
    pub request: String,
}
```

Create `src/app.rs`:

```rust
use crate::cli::Cli;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub request: String,
    pub generated_command: Option<String>,
    pub exit_code: i32,
}

pub fn run(cli: Cli) -> RunSummary {
    println!("Request: {}", cli.request);
    RunSummary {
        request: cli.request,
        generated_command: None,
        exit_code: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_records_the_request() {
        let summary = run(Cli {
            request: "list Rust files".into(),
        });
        assert_eq!(
            summary,
            RunSummary {
                request: "list Rust files".into(),
                generated_command: None,
                exit_code: 0,
            }
        );
    }
}
```

Replace `src/main.rs`:

```rust
mod app;
mod cli;

use clap::Parser;
use cli::Cli;

fn main() {
    let summary = app::run(Cli::parse());
    std::process::exit(summary.exit_code);
}
```

`pub` is used only where another module needs access. Modules are private by default. The crate root (`main.rs`) declares which source files participate in the crate.

## Compiler conversation

If `request` is private, constructing `Cli` in `app` tests fails with a privacy diagnostic. You have three choices:

1. Make the field `pub`.
2. Add a constructor.
3. Parse test arguments through Clap.

For this small data-only CLI type, a public field is acceptable. Later, types with invariants should prefer constructors that can reject invalid state.

Another common error appears if you print after moving:

```rust,compile_fail
let request = cli.request;
println!("{}", cli.request);
```

Move only when the old owner is finished, or clone when two independent owned values are truly required.

## Tests

Keep the Chapter 1 parser test in `cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_request() {
        let cli = Cli::try_parse_from(["sai-course", "show disk usage"]).unwrap();
        assert_eq!(cli.request, "show disk usage");
    }
}
```

The application test checks returned state; it should not need to capture terminal output. This is the first example of making important behavior observable through a value.

## Review checklist

- Does `main` only wire parsing, running, and process exit?
- Does `RunSummary` represent actual current behavior?
- Is absence represented by `Option`, not an empty string?
- Is visibility no broader than required?
- Can application behavior be called directly from a test?

## Checkpoint

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- "show disk usage"
```

Then:

```bash
git add src
git commit -m "Tutorial 02: application types"
git tag tutorial-02-application-types
```

## Stretch exercise

Add a private `RunSummary::from_request(String)` constructor. Decide whether it improves clarity or merely hides a struct literal.

## Reflection

The AI can propose module layouts; you decide which boundaries are justified now. Rust makes the chosen ownership and visibility enforceable across the crate.

## Further learning

- [The Rust Book — Defining and Instantiating Structs](https://doc.rust-lang.org/book/ch05-01-defining-structs.html)
- [The Rust Book — Control Scope and Privacy with Modules](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html)
- [Comprehensive Rust — Visibility](https://google.github.io/comprehensive-rust/modules/visibility.html) — the `pub` question this chapter asks of every field.
- [Rustlings — `07_structs`](https://github.com/rust-lang/rustlings/tree/main/exercises/07_structs)
- [Rustlings — `10_modules`](https://github.com/rust-lang/rustlings/tree/main/exercises/10_modules)

Next: [Errors are part of the design](03-errors.md).

