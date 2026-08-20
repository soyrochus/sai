# Rust in the Loop

**Learn Rust. Build with AI. Build AI-powered apps.**

## Proposal

`Rust in the Loop` is a progressive, project-based tutorial in which learners build SAI, a real Rust command-line application that turns natural-language requests into safe, executable commands.

The tutorial teaches three connected skills at the same time:

1. **Learn Rust** by building a useful application from an empty Cargo project into a tested, production-minded CLI.
2. **Learn to build with AI** by using an AI coding assistant as a collaborator for planning, implementation, debugging, testing, and review.
3. **Learn to build apps that use AI** by integrating a language model into the application itself and designing around uncertainty, safety, cost, and failure.

The application begins with a few lines of synchronous Rust and grows one deliberate step at a time. Every chapter produces a working program, introduces a small set of Rust concepts, and adds one meaningful product capability. Complexity is earned rather than presented up front.

## Why this tutorial

Most Rust tutorials teach language features through isolated exercises. Most AI coding tutorials focus on prompts and generated code without teaching how to evaluate the result. Most AI application tutorials stop after the first successful API call.

This tutorial joins those subjects around one evolving product.

SAI is a strong teaching project because its requirements expose the qualities that make Rust valuable:

- Generated commands must be represented and validated precisely.
- Filesystem and process operations make errors and side effects concrete.
- Configuration and history introduce durable data and backward compatibility.
- Safety modes benefit from enums and explicit invariants.
- Traits let model calls and command execution be replaced in tests.
- Shell quoting and cross-platform behavior demand careful boundary design.
- The compiler turns AI mistakes into teachable feedback.

AI is not presented as an oracle that writes the application. It is presented as a fast but fallible engineering partner. Learners remain responsible for requirements, design decisions, verification, and the final behavior of the software.

## The central learning loop

Every chapter follows the same repeatable loop:

1. **Define** one observable product outcome.
2. **Ask** AI to explain unfamiliar concepts and propose a small implementation.
3. **Inspect** the proposed code before accepting it.
4. **Compile** and use Rust's diagnostics to expose incorrect assumptions.
5. **Test** the behavior, including failure and boundary cases.
6. **Refactor** toward clearer types and smaller responsibilities.
7. **Reflect** on what the human decided, what AI accelerated, and what Rust verified.

This loop is itself a course outcome. By the end, learners should be able to repeat it on applications of their own.

## Audience

The primary audience is a developer who can read basic code but is new to Rust, AI-assisted development, or both.

The tutorial assumes:

- Basic command-line familiarity.
- Familiarity with variables, functions, and simple control flow in any language.
- No previous Rust experience.
- No previous language-model API experience.

The tutorial does not assume knowledge of lifetimes, async Rust, shell internals, prompt engineering, or machine-learning theory.

## Learning outcomes

By completing the tutorial, learners will be able to:

### Rust

- Create and organize a Cargo project.
- Work confidently with ownership, borrowing, references, and cloning.
- Model application state with structs, enums, `Option`, and `Result`.
- Use pattern matching, iterators, slices, generics, and traits.
- Parse and serialize JSON and YAML with Serde.
- Work with paths, files, processes, environment variables, and terminal input.
- Design platform-specific behavior with conditional compilation.
- Write unit, integration, regression, and end-to-end tests.
- Use `cargo fmt`, `cargo test`, and Clippy as part of normal development.
- Read compiler diagnostics and use them to improve a design rather than merely silence an error.

### Building with AI

- Turn an idea into bounded requirements before requesting code.
- Give an AI assistant enough context without handing it the entire problem at once.
- Ask for explanations, alternatives, tests, and reviews—not only implementations.
- Separate generated suggestions from verified facts.
- Review changes for correctness, security, scope, and maintainability.
- Use compiler errors and failing tests as structured feedback for the next AI interaction.
- Keep changes small enough to understand and reverse.
- Maintain a decision trail through specifications, commits, and chapter checkpoints.

### Building AI-powered applications

- Call a language-model API and parse its response.
- Separate model-provider code from application logic.
- Construct prompts from configuration and user input.
- Treat model output as untrusted input.
- Validate generated commands before execution.
- Design explicit safety modes and human confirmation flows.
- Handle API errors, malformed responses, missing configuration, and unavailable models.
- Test AI-integrated software without contacting a real model.
- Reason about determinism, cost, latency, privacy, and observability.
- Decide which behavior belongs in the model and which must remain deterministic application code.

## The project learners will build

Learners will progressively create a version of SAI with the following journey:

```text
natural-language request
        ↓
Rust CLI and configuration
        ↓
language-model request
        ↓
untrusted generated command
        ↓
parsing, validation, and risk analysis
        ↓
human review when required
        ↓
execution or deterministic script emission
        ↓
history, diagnostics, and tests
```

The finished application is substantial enough to demonstrate real engineering, but compact enough for one learner to understand end to end.

## Progressive curriculum

### Part I — Learn Rust by building a CLI

#### 1. A program that understands arguments

Create a new Cargo project and accept a natural-language request from the command line.

Rust focus: `main`, variables, `String`, `&str`, functions, `Result`, Cargo, and basic `clap` usage.

AI collaboration focus: ask AI for a minimal plan and require it to explain every dependency it proposes.

Checkpoint: `sai "list Rust files"` prints the request exactly as received.

#### 2. Types for a growing application

Introduce a CLI struct and a run summary rather than passing loose values through `main`.

Rust focus: structs, derives, associated functions, modules, visibility, and the difference between owned and borrowed data.

AI collaboration focus: compare two designs and ask the assistant to identify trade-offs before choosing one.

Checkpoint: parsing and application behavior are separated and independently testable.

#### 3. Errors are part of the design

Reject empty requests and report useful context when something fails.

Rust focus: `Result`, the `?` operator, error context, early returns, and recoverable versus unrecoverable failures.

AI collaboration focus: ask AI to enumerate failure cases before implementing the happy path.

Checkpoint: invalid input produces a clear error and non-zero exit status.

### Part II — Build an app with AI inside it

#### 4. The first model call

Send the user's request to a language model and print the generated command without executing it.

Rust focus: HTTP clients, request and response structs, Serde JSON, environment variables, and dependency boundaries.

AI application focus: model selection, system instructions, temperature, authentication, latency, and API failure.

Checkpoint: the application turns natural language into a displayed command.

#### 5. Configuration instead of hard-coded behavior

Load provider settings and permitted tools from YAML.

Rust focus: `Option<T>`, `Default`, nested structs, Serde attributes, `Path` and `PathBuf`, and precedence rules.

AI application focus: separate operational configuration from prompt configuration and credentials.

Checkpoint: behavior changes through configuration without recompiling.

#### 6. Treat model output as untrusted

Parse the generated command and reject tools that are not configured.

Rust focus: enums, pattern matching, iterators, slices, pure functions, and domain-specific errors.

AI application focus: why prompting is not validation and why deterministic checks belong outside the model.

Checkpoint: allowed commands proceed; unconfigured commands are rejected before execution.

#### 7. Execute the safe path

Run a validated command directly as a process without invoking a shell.

Rust focus: `std::process::Command`, argument vectors, exit codes, filesystem globs, and operating-system boundaries.

AI application focus: least privilege and minimizing capabilities granted to generated output.

Checkpoint: a safe generated command executes and returns its real exit status.

### Part III — Use AI without surrendering engineering judgment

#### 8. Traits make external systems replaceable

Introduce `CommandGenerator` and `CommandExecutor` traits and replace both with test doubles.

Rust focus: traits, implementations, generics, static dispatch, interior mutability in tests, and dependency injection.

AI collaboration focus: ask AI to write tests against contracts rather than implementation details.

Checkpoint: the full application flow can be tested without a network or real command execution.

#### 9. Safety modes as explicit states

Add confirmation, unsafe, and unrestricted modes without reducing them to scattered booleans.

Rust focus: enums as state models, exhaustive matching, invariants, precedence, and conversion between representations.

AI application focus: human-in-the-loop design, risk communication, and why an AI-generated explanation is not an independent safety check.

Checkpoint: each safety mode has defined, tested behavior.

#### 10. Local risk analysis

Detect operators, destructive flags, and broad wildcards using deterministic Rust code.

Rust focus: scanners, state transitions, character iteration, borrowed input, and table-driven tests.

AI collaboration focus: use AI to generate adversarial cases, then verify each case independently.

Checkpoint: confirmations include reproducible risk markers computed without a model.

### Part IV — Grow into a durable application

#### 11. History and backward-compatible data

Write invocation history as newline-delimited JSON and tolerate records produced by older versions.

Rust focus: file I/O, append-only data, Serde defaults, rotation, corrupt-record recovery, and compatibility tests.

AI application focus: observability, privacy, provenance, and deciding what AI-related data should be retained.

Checkpoint: runs can be inspected later and old history remains readable.

#### 12. An interactive terminal editor

Add multiline prompt composition, cursor movement, history recall, and cancellation.

Rust focus: event loops, state machines, Unicode text, terminal modes, mutable state, and deterministic UI tests.

AI collaboration focus: break a complex feature into independently testable transitions before generating code.

Checkpoint: users can compose and reuse rich prompts without leaving the terminal.

#### 13. Deterministic commands

Freeze a reviewed generated command into an executable script that runs without SAI or a model.

Rust focus: Unix permissions, atomic writes, renames, conditional compilation, shell quoting, metadata parsing, and end-to-end tests.

AI application focus: move the model from a runtime dependency to an authoring-time dependency.

Checkpoint: a frozen command produces the same observable result as SAI's executor, including glob behavior.

#### 14. Specification-driven development with AI

Plan one final feature through proposal, behavioral specification, design, tasks, implementation, verification, and archive.

Rust focus: maintaining correctness across modules and protecting behavior with regression tests.

AI collaboration focus: persistent context, bounded tasks, reviewable diffs, and evidence-based completion.

Checkpoint: learners can apply the complete workflow to a feature of their own design.

## Chapter design

Each chapter will contain:

- **Product goal** — one sentence describing what users gain.
- **Rust concepts** — the language ideas introduced and why the feature needs them.
- **AI collaboration script** — example prompts for planning, implementation, explanation, debugging, and review.
- **Build steps** — small changes that keep the application compiling frequently.
- **Compiler conversation** — at least one diagnostic explained in context.
- **Test strategy** — happy path, failure path, and one adversarial case.
- **Review checklist** — questions the learner must answer before accepting AI-generated code.
- **Checkpoint** — commands and expected behavior proving the chapter is complete.
- **Stretch exercise** — an optional variation without a provided final implementation.
- **Reflection** — what AI contributed, what Rust guaranteed, and what remained a human decision.

## Teaching ownership through real mistakes

The tutorial should preserve useful failures rather than editing history into a flawless sequence. For example, passing an owned configuration field into a resolver and later borrowing the containing struct produces a partial-move error:

```rust
let effective_ai = resolve_ai_config(global_cfg.ai)?;
// Later: borrow global_cfg again.
```

That failure creates a concrete lesson about ownership. Possible repairs—borrowing, cloning, or changing the resolver's API—can be compared instead of presenting `clone()` as magic.

Similar teaching moments should include:

- A model returning a disallowed command.
- Shell text that is valid text but unsafe execution input.
- A test that accidentally contacts a real API.
- A serialized struct that breaks older history entries.
- A script that changes semantics because an argument was quoted incorrectly.
- A confirmation flow that writes a file before the user approves it.

The learner should see that AI can produce plausible code while Rust and a good test strategy reveal whether it is actually correct.

## Repository and release structure

The tutorial will live under `tutorial/` while the current application remains the reference implementation.

Proposed structure:

```text
tutorial/
  proposal.md
  README.md
  chapters/
    01-minimal-cli.md
    02-application-types.md
    03-errors.md
    04-first-model-call.md
    05-configuration.md
    06-validation.md
    07-execution.md
    08-traits-and-tests.md
    09-safety-modes.md
    10-risk-analysis.md
    11-history.md
    12-terminal-editor.md
    13-deterministic-commands.md
    14-spec-driven-development.md
  prompts/
  exercises/
  checkpoints.md
```

Each chapter will correspond to a Git tag such as `tutorial-01-minimal-cli`. A learner can start from the previous tag, complete the chapter, and compare the result with the next tag. The finished SAI codebase remains the destination, not the starting point.

## Responsible use of AI

The tutorial will establish the following rules from the beginning:

- Never execute generated commands without understanding and validating them.
- Never place secrets in prompts, source files, fixtures, or captured logs.
- Never accept a dependency solely because AI suggested it.
- Require tests for behavior that matters, especially safety boundaries.
- Prefer authoritative documentation when an API or library may have changed.
- Keep AI-generated changes small enough for a human to review.
- Treat confident explanations as hypotheses until code, tests, or documentation support them.
- Record important product and safety decisions outside the chat transcript.

## Scope

The first edition will cover:

- A synchronous command-line application.
- OpenAI-compatible and Azure OpenAI-style provider configuration.
- Unix-oriented command execution and script emission.
- Unit and end-to-end testing without live model calls.
- AI-assisted planning, coding, debugging, testing, and review.
- The complete path from beginner Rust syntax to a modular application.

## Non-goals

The first edition will not attempt to teach:

- Machine-learning model training or fine-tuning.
- Advanced async Rust or a web-server framework.
- A graphical user interface.
- Production multi-user service operation.
- Exhaustive shell-language parsing.
- Full Windows script emission.
- Blind "vibe coding" where generated changes are accepted without review.

These can become follow-on projects after the core learning path is stable.

## Deliverables

The project will produce:

- Fourteen progressive tutorial chapters.
- A working checkpoint for every chapter.
- Reusable AI prompts annotated with their purpose and limitations.
- Exercises that require learners to make design decisions rather than copy code.
- Automated tests for each checkpoint.
- A troubleshooting guide organized around compiler and runtime symptoms.
- A final capstone in which learners specify and implement their own AI-assisted feature.

## Success criteria

The tutorial succeeds when a learner can:

1. Explain the ownership and type choices in the finished application.
2. Modify the application without relying on AI to understand the existing code for them.
3. Use AI to plan and implement a bounded feature through small, reviewable changes.
4. Write tests that detect an incorrect AI-generated implementation.
5. Integrate a model while keeping validation and safety decisions deterministic.
6. Build a second Rust application that uses AI, applying the same architecture and workflow.

## Project identity

**Name:** Rust in the Loop

**Tagline:** Learn Rust. Build with AI. Build AI-powered apps.

The name captures both sides of the course. Rust stays in the execution and verification loop, providing explicit types and deterministic boundaries. The learner stays in the decision loop, directing and reviewing AI rather than handing responsibility to it.
