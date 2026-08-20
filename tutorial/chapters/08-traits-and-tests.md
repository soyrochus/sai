# Chapter 8 — Put Side Effects Behind Traits

The program works, but its application flow is tied to a live model and the operating system. Traits let us test orchestration without either dependency.

## Product goal

Represent command generation and execution as capabilities. Test the application with small in-memory implementations.

## Rust concepts

This refactor introduces traits, implementations, generic bounds, static dispatch, dependency injection, and narrow interior mutability for recording test doubles.

## Build

Define the boundaries:

```rust
use anyhow::Result;

pub trait CommandGenerator {
    fn generate(&self, request: &str) -> Result<String>;
}

pub trait CommandExecutor {
    fn execute(&self, command: &ValidatedCommand) -> Result<i32>;
}
```

Wrap the concrete implementations:

```rust
pub struct HttpGenerator {
    pub client: reqwest::blocking::Client,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

impl CommandGenerator for HttpGenerator {
    fn generate(&self, request: &str) -> Result<String> {
        crate::llm::generate_command(
            &self.client,
            &self.endpoint,
            &self.api_key,
            &self.model,
            request,
        )
    }
}

pub struct DirectExecutor;

impl CommandExecutor for DirectExecutor {
    fn execute(&self, command: &ValidatedCommand) -> Result<i32> {
        crate::executor::execute(command)
    }
}
```

Move orchestration into a generic function:

```rust
pub fn run<G, E>(
    request: &str,
    allowed: &HashSet<String>,
    generator: &G,
    executor: &E,
) -> Result<i32>
where
    G: CommandGenerator,
    E: CommandExecutor,
{
    let generated = generator.generate(request)?;
    let validated = validate(&generated, allowed)?;
    executor.execute(&validated)
}
```

`main` now assembles concrete dependencies; `run` describes policy and workflow. This is dependency injection using ordinary Rust types, not a framework.

The finished project uses the same idea in [`src/app.rs`](../../src/app.rs).

## AI collaboration script

Ask:

> Refactor this application so model generation and command execution are traits. Keep `main` responsible for constructing real implementations. Make orchestration generic over borrowed trait implementors and add recording test doubles. Avoid global mutable state and mocking libraries.

Review the result for unnecessary abstraction. A trait is justified when it defines a real boundary you need to substitute—not merely because a type has methods.

Then ask:

> Explain the tradeoff between generics (`G: CommandGenerator`) and trait objects (`&dyn CommandGenerator`) for this function. Which choice keeps this tutorial simplest, and when would dynamic dispatch help?

## Compiler conversation

The `where` clause says `run` is valid for any concrete `G` and `E` that implement the required traits. The compiler specializes calls through static dispatch.

The function borrows both dependencies. It does not need ownership, and it does not constrain how long they live beyond the call.

Object safety becomes relevant if you store mixed implementations:

```rust
let generator: Box<dyn CommandGenerator> = Box::new(HttpGenerator { /* ... */ });
```

The simple methods above are object-safe because they do not return `Self`, use generic method parameters, or require compile-time knowledge of the implementing type.

Keep errors as `anyhow::Result` at this application boundary. Later, a library intended for other crates might expose a typed error enum instead.

## Tests

Write tiny test doubles:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct StubGenerator(&'static str);

    impl CommandGenerator for StubGenerator {
        fn generate(&self, _request: &str) -> Result<String> {
            Ok(self.0.to_owned())
        }
    }

    #[derive(Default)]
    struct RecordingExecutor {
        seen: RefCell<Vec<Vec<String>>>,
    }

    impl CommandExecutor for RecordingExecutor {
        fn execute(&self, command: &ValidatedCommand) -> Result<i32> {
            self.seen.borrow_mut().push(command.tokens.clone());
            Ok(0)
        }
    }

    #[test]
    fn orchestration_validates_before_execution() {
        let generator = StubGenerator("find . -name '*.rs'");
        let executor = RecordingExecutor::default();
        let allowed = ["find"].into_iter().map(str::to_owned).collect();

        assert_eq!(run("find Rust files", &allowed, &generator, &executor).unwrap(), 0);
        assert_eq!(executor.seen.borrow().len(), 1);
        assert_eq!(executor.seen.borrow()[0][0], "find");
    }

    #[test]
    fn invalid_generation_never_reaches_executor() {
        let generator = StubGenerator("rm -rf something");
        let executor = RecordingExecutor::default();
        let allowed = ["find"].into_iter().map(str::to_owned).collect();

        assert!(run("bad request", &allowed, &generator, &executor).is_err());
        assert!(executor.seen.borrow().is_empty());
    }
}
```

`RefCell` provides checked interior mutability. The trait method only receives `&self`, but the recorder needs to retain observations. Borrow-rule violations become runtime panics, so keep the mutable region small.

## Review checklist

- Traits correspond to external side-effect boundaries.
- `main` constructs production dependencies.
- Orchestration borrows its dependencies.
- Unit tests need neither network nor child processes.
- A validation failure proves the executor was not called.

## Checkpoint

```bash
git add src
git commit -m "tutorial: isolate side effects behind traits"
git tag tutorial-08-traits-and-tests
```

Evidence: orchestration tests run offline and record exactly what would have been executed.

## Stretch exercise

Replace the generic parameters with `&dyn CommandGenerator` and `&dyn CommandExecutor`. Compare compiler errors, call sites, and the ability to choose an implementation at runtime from configuration.

## Reflection

- Which behavior belongs in orchestration rather than in the generator or executor?
- Why can a trait improve testing even when there is only one production implementation?
- What risks does `RefCell` move from compile time to runtime?
