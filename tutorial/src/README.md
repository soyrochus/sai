# Rust in the Loop

**Learn Rust. Build with AI. Build AI-powered apps.**

This course builds a small version of [SAI](../../README.md) from an empty Cargo project into a safe, tested AI-powered command-line application. You will write the application yourself. The production code in `src/` is the reference destination, not code to copy at the start.

## The three tracks

Every chapter advances three skills together:

- **Rust:** ownership, types, traits, files, processes, terminal state, and tests.
- **Building with AI:** planning bounded changes, reviewing suggestions, debugging with evidence, and keeping a decision trail.
- **Building AI-powered apps:** model APIs, untrusted output, deterministic validation, human review, and offline artifacts.

## Before you begin

Install a current stable Rust toolchain and confirm:

```bash
rustc --version
cargo --version
```

You also need Git, a terminal, and an AI coding assistant that can inspect and edit a local repository. If you don't already have one set up, see [Connect an AI coding assistant](assistant-setup.md) for concrete steps with three widely used options. A model API key is not needed until Chapter 4. Never commit an API key.

Create the course project beside this repository:

```bash
cargo new sai-course
cd sai-course
git init
git add .
git commit -m "Start Rust in the Loop"
```

All chapter commands assume you are inside `sai-course`, not this reference repository.

## How to study

For each chapter:

1. Read the product goal and acceptance criteria.
2. Ask your AI assistant for a plan using the provided prompt.
3. Make one small change at a time.
4. Read every diff before compiling.
5. Run the chapter tests.
6. Explain the design aloud or in writing.
7. Commit and tag the checkpoint only after it passes.

Use this quality gate throughout:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Curriculum

### Part I — Learn Rust by building a CLI

1. [A program that understands arguments](chapters/01-minimal-cli.md)
2. [Types for a growing application](chapters/02-application-types.md)
3. [Errors are part of the design](chapters/03-errors.md)

### Part II — Build an app with AI inside it

4. [The first model call](chapters/04-first-model-call.md)
5. [Configuration instead of hard-coded behavior](chapters/05-configuration.md)
6. [Treat model output as untrusted](chapters/06-validation.md)
7. [Execute the safe path](chapters/07-execution.md)

### Part III — Use AI without surrendering judgment

8. [Traits make external systems replaceable](chapters/08-traits-and-tests.md)
9. [Safety modes as explicit states](chapters/09-safety-modes.md)
10. [Local risk analysis](chapters/10-risk-analysis.md)

### Part IV — Grow into a durable application

11. [History and backward-compatible data](chapters/11-history.md)
12. [An interactive terminal editor](chapters/12-terminal-editor.md)
13. [Deterministic commands](chapters/13-deterministic-commands.md)
14. [Specification-driven development with AI](chapters/14-spec-driven-development.md)

## Supporting material

- [Connect an AI coding assistant](assistant-setup.md) — concrete setup for three widely used options.
- [Checkpoint matrix](checkpoints.md) — acceptance commands and expected evidence.
- [Reusable AI prompts](prompts/README.md) — planning, implementation, debugging, and review prompts.
- [Exercises and capstone](exercises/README.md) — design work that is intentionally not solved for you.
- [Troubleshooting](troubleshooting.md) — compiler, test, API, shell, and terminal symptoms.
- [Divergences from the reference application](divergences.md) — chapter-by-chapter, where `src/` differs from what you build and why.
- [Instructor's guide](instructor-guide.md) — chapter-by-chapter, why each one is built the way it is: what's delegated to AI, what's authored, and what the compiler-conversation centerpiece proves.
- [Course proposal](proposal.md) — goals, scope, and teaching philosophy.

## The rule that matters most

AI output is a proposal. Rust code that compiles is a stronger proposal. Tested behavior that satisfies an explicit requirement is evidence.

Never skip directly from the first to the third.

## License

This tutorial's text — chapters, prose, exercises, and supporting documentation — is licensed under [CC BY 4.0](LICENSE.md). The SAI application source code it references, under `src/` at the root of this repository, remains separately licensed under the MIT License.

