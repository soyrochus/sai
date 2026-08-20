# Troubleshooting

Start with the complete command and diagnostic. Do not ask AI to debug a paraphrase.

## Cargo cannot find a crate or feature

Check `Cargo.toml`, then run:

```bash
cargo metadata --no-deps
cargo check
```

Ask whether the feature is needed. For example, Serde derives and Reqwest blocking support are opt-in features.

## Borrow of moved or partially moved value

Look for a field passed by value:

```rust
let effective = resolve(config.ai)?;
use_config_again(&config);
```

`config.ai` moved even though the rest of `config` did not. Possible designs are:

- Make the resolver borrow `&AiConfig`.
- Clone the optional subsection when configuration data is small and intentionally shared.
- Destructure and stop using the parent value.

Choose based on ownership semantics, not merely the shortest compiler fix.

## A closure cannot mutate captured state

Check whether it implements `Fn`, `FnMut`, or `FnOnce`, and whether the caller bound it as mutable. In tests, `Cell` and `RefCell` can record calls through a shared reference, but they move borrow checking to runtime. Keep their scope narrow.

## Tests accidentally contact the model

Search for direct construction of the HTTP generator. Application tests should receive a fake through a trait. Remove the API key and rerun tests; a correct offline suite still passes.

## The model returns Markdown instead of a command

Do not strip arbitrary formatting and execute what remains. Tighten the system instruction, parse the response contract, and reject output that is not one valid command.

## A command works in a shell but fails in default mode

Default mode spawns a program with an argument vector. Pipes, redirects, substitutions, and chaining belong to the shell language and are intentionally unavailable. Print the tokens in a dry run to see what the program actually receives.

## A safe-looking string becomes dangerous when emitted as a script

A direct process argument and shell source code have different semantics. Quote every ordinary token in default mode. Preserve only deliberate glob expansion. Test the emitted script by executing it in a temporary directory and comparing observable output.

## A new history field breaks old records

Optional additive fields need a compatibility default:

```rust
#[serde(default)]
pub prompt: Option<String>,
```

Keep a literal legacy JSON fixture with the field absent.

## Terminal tests hang or corrupt the screen

Move editing behavior into a pure state object. Feed key events directly to that object in tests. Keep raw terminal mode and rendering in a thin driver, and always restore terminal state on errors.

## Tests pass alone but fail together

Suspect shared process state: environment variables, current directory, global files, terminal state, or a shared temp filename. Prefer dependency injection and per-test temporary directories. If global state is unavoidable, serialize only the affected tests and document why.

## Clippy rejects code that tests accept

Treat Clippy as design feedback. Read the lint documentation, decide whether the suggested shape improves the code, and use `#[allow]` only with a local explanation of why the lint does not apply.

## An AI assistant keeps expanding scope

Restate one product outcome, name out-of-scope behavior, and request one plan step. If the change needs a new architectural decision, stop implementation and record that decision first.

