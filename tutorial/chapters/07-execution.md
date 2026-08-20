# Chapter 7 — Execute Without a Shell

The application now has a validated program and argument vector. It can finally run a command while preserving the boundary established in Chapter 6.

## Product goal

Execute the validated program directly, return its exit code, and never pass default-mode text through `sh -c` or another shell.

## Rust concepts

You will use `std::process::Command`, borrowed slices from `split_first`, child exit-status modeling, platform-specific tests, and contextual I/O errors.

## Build

Create `src/executor.rs`:

```rust
use anyhow::{Context, Result, bail};
use std::process::Command;

use crate::validation::ValidatedCommand;

pub fn execute(command: &ValidatedCommand) -> Result<i32> {
    let Some((program, args)) = command.tokens.split_first() else {
        bail!("validated command unexpectedly had no program");
    };

    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to start {program:?}"))?;

    Ok(status.code().unwrap_or(1))
}
```

Connect the stages in the application layer:

```rust
let generated = llm::generate_command(/* ... */)?;
let validated = validation::validate(&generated, &allowed)?;

eprintln!("Command: {}", validated.source);
let exit_code = executor::execute(&validated)?;
std::process::exit(exit_code);
```

There is an important semantic difference:

```rust
Command::new("find").args([".", "-name", "*.rs"])
```

passes `*.rs` literally to `find`, which is what `find -name` expects. A shell would expand a glob before starting the program. Direct execution avoids shell control syntax, but it also means your application must define glob behavior deliberately for tools that expect expanded filenames.

The finished application handles this boundary in [`src/executor.rs`](../../src/executor.rs). Read it after completing the simple direct version.

## AI collaboration script

Ask:

> Implement direct process execution from a previously validated token vector using `std::process::Command`. Preserve stdout and stderr inheritance, return a normalized exit code, and attach context if the program cannot start. Do not invoke a shell.

Then ask for a review:

> Compare direct argument execution with `sh -c`. List concrete differences for spaces, quotes, globbing, pipes, redirects, environment expansion, and command substitution. Identify which differences are security properties and which are user-experience tradeoffs.

This is a productive use of AI: it can enumerate edge cases quickly. Verify the important claims with focused integration tests.

## Compiler conversation

`split_first()` returns an `Option<(&String, &[String])>`. The first element is borrowed as the executable name; the rest is borrowed as a slice. No token needs to be cloned.

`Command::new` accepts many path-like borrowed values, including `&String`. `.args(args)` accepts an iterator over borrowed arguments. Rust’s trait system makes the efficient version look nearly identical to an allocating version.

An exit status may have no numeric code when a process is terminated by a signal. `status.code()` therefore returns `Option<i32>`. This tutorial maps that case to `1`; a production Unix application might inspect `ExitStatusExt::signal()` and choose a richer policy.

Do not confuse these two errors:

- `Command::status()` returning `Err`: the child could not be started.
- a successful `status()` containing a nonzero code: the child ran and reported failure.

The first is a Rust `Result` error. The second is ordinary program output represented as data.

## Tests

Keep unit tests platform-neutral where possible, and mark platform-specific integration tests clearly:

```rust
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::validation::ValidatedCommand;

    fn command(tokens: &[&str]) -> ValidatedCommand {
        ValidatedCommand {
            source: tokens.join(" "),
            tokens: tokens.iter().map(|value| (*value).to_owned()).collect(),
        }
    }

    #[test]
    fn returns_success_exit_code() {
        assert_eq!(execute(&command(&["true"])).unwrap(), 0);
    }

    #[test]
    fn returns_child_failure_code() {
        assert_ne!(execute(&command(&["false"])).unwrap(), 0);
    }

    #[test]
    fn missing_program_is_a_start_error() {
        let error = execute(&command(&["definitely-not-a-real-tutorial-tool"]))
            .unwrap_err();
        assert!(error.to_string().contains("failed to start"));
    }
}
```

If you made `ValidatedCommand` fields private in the stretch exercise, add a test-only constructor inside its module or obtain instances through `validate`. Do not weaken the production boundary solely to simplify a test.

For an end-to-end manual check, temporarily allow `printf` and request a harmless command whose single argument contains spaces. Confirm that it remains one argument.

## Review checklist

- Default execution uses `Command::new(program).args(args)`.
- Generated text never reaches a shell in this mode.
- Start failures and child exit codes remain distinct.
- Signal termination has an explicit fallback policy.
- Tests exercise success, nonzero status, and a missing executable.

## Checkpoint

```bash
git add src
git commit -m "tutorial: execute validated commands directly"
git tag tutorial-07-execution
```

Evidence: unit and Unix integration tests pass, and a manual command with a spaced argument is delivered to the child as one argument.

## Stretch exercise

Add explicit glob expansion with the `glob` crate, but only for unquoted arguments and only after validation. First write down the desired behavior for no matches, hidden files, and malformed patterns. Then implement those decisions as tests.

## Reflection

- Why does direct execution remove an entire class of shell-injection behavior?
- Which conveniences disappear when there is no shell?
- Should a child’s nonzero exit code become an application error, or should the CLI propagate it unchanged?
