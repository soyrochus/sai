# Instructor's Guide

This is a companion document, not a fifteenth chapter of the course. It's for whoever is teaching, facilitating, or adapting "Rust in the Loop" — including a future version of yourself returning to a chapter after months away. It explains, chapter by chapter, *why* each one is built the way it is: what its real teaching payload is beneath its surface topic, why certain explanations live in an AI prompt instead of the chapter's own prose, what the staged compiler failure is actually proving, and what an instructor should watch for when running it live.

Nothing here is required reading for a learner working through the course solo — everything a learner needs is already in the chapters. This document exists because the design decisions behind those chapters are mostly invisible from inside them, and an instructor adapting or extending the course benefits from seeing the scaffolding, not just the finished room.

## The shared frame

Read this section once. Every chapter below assumes it.

### The fixed template is the point

All fourteen chapters use the same ten headings, in the same order, mechanically enforced by `tutorial/scripts/check-course.sh` — a chapter that's missing "Compiler conversation" or "Reflection" fails the build. This isn't a formatting convenience. The template *is* the curriculum's actual spine: `proposal.md` names a seven-step loop — Define, Ask, Inspect, Compile, Test, Refactor, Reflect — as the course's real product, more durable than any specific Rust syntax. The headings are that loop made mechanical:

| Loop step | Heading(s) |
|---|---|
| Define | Product goal, Rust concepts |
| Ask | AI collaboration script |
| Inspect | Build (the human narration around the code) |
| Compile | Compiler conversation |
| Test | Tests |
| Refactor | Review checklist, Stretch exercise |
| Reflect | Reflection |

Because the shape never changes, a learner (or a cohort) stops noticing it by Chapter 3 or 4 — which is exactly when it starts working unconsciously. If you add a fifteenth chapter of your own, keep the template. Breaking it breaks the thing that makes the earlier chapters transferable.

### Where an explanation lives is itself a lesson

Every chapter makes a deliberate choice about *where* a given piece of knowledge is explained, and that placement is not interchangeable:

- **Inside the AI collaboration script** — mechanical, fast-changing, or purely a matter of recall (library syntax, what a derive macro expands to, enumerating edge cases). This is explicitly the category the course tells learners to delegate; see the Reflection lines that say things like "AI can recall Clap syntax quickly" (Ch. 1) or "AI is useful here because it can draft unfamiliar HTTP and Serde plumbing" (Ch. 4).
- **In the chapter's own prose, narrated right after a code block** — a judgment call, almost always about ownership, that recurs at every scale for the rest of the course (own vs. borrow, when to clone, when a type should make an invalid state unconstructable).
- **Staged as a Compiler Conversation failure** — something the course wants the learner to *experience* going wrong before being told why, per `proposal.md`'s explicit stance that the tutorial should "preserve useful failures rather than editing history into a flawless sequence."
- **Named once, at the very end, in Reflection** — the chapter's thesis stated as confirmation of something already lived through, not as new information front-loaded at the top.

When you're deciding how to explain something new in your own material, this is the rubric: if it's lookup, delegate it to the AI prompt; if it's judgment, put it in your own voice; if it's a trap, stage the failure and let the compiler say it.

### The four parts, in one paragraph each

**Part I** (Ch. 1–3) deliberately uses the smallest possible Rust surface — one CLI argument, one struct split, one validation rule — because the actual content being rehearsed is the loop itself, not CLI parsing. **Part II** (Ch. 4–7) crosses into AI-application territory and immediately establishes the course's central discipline: model output is data, not a command, until something deterministic has validated it — every chapter in this part hardens one more link in that chain (generate → configure → validate → execute). **Part III** (Ch. 8–10) turns "trust but verify" into compiler-enforced structure: traits make dependencies substitutable, an enum makes safety states exhaustive, and a lifetime-carrying type makes a risk marker's provenance unforgeable. **Part IV** (Ch. 11–14) scales the same disciplines to a durable application — data that must outlive the process (history), state that must survive real terminals (the editor), artifacts that must survive the model being unavailable (frozen commands) — and closes by handing the whole loop to the learner unsupervised in Chapter 14, which teaches process, not syntax.

---

## Part I — Learn Rust by building a CLI

### Chapter 1 — A program that understands arguments

**Payload:** rehearsing the loop itself, not CLI parsing — the Rust content is deliberately minimal so the loop's mechanics are the entire lesson.

**Delegated vs. authored:** "Explain what the derive macro generates" is asked *of the AI*, inside the prompt (line 35) — it's mechanical recall. The chapter's own prose is spent entirely on one thing: why `render_request` borrows `&str` instead of owning `String` (line 80). That split is the chapter enacting its own thesis before stating it.

**Centerpiece:** a deliberately wrong signature (`request: String`) that compiles fine called once, then fails on a second call — "used after move." This is the first time the learner sees ownership enforced by a diagnostic rather than described in prose.

**Answer key:** "AI can recall Clap syntax quickly. Rust determines whether ownership is coherent. You decide the product contract." — the whole course's division of labor, stated once, after being lived.

**Watch for:** learners who accept the AI's first draft without running the Review checklist's last item — "Did AI add anything not required by the contract?" — tend to carry unnecessary abstractions into Chapter 2, where they become harder to justify removing.

### Chapter 2 — Types for a growing application

**Payload:** making an outcome *observable as a value* (`RunSummary`) instead of only as printed side effects — the first instance of a pattern every later chapter's tests depend on.

**Delegated vs. authored:** the AI prompt explicitly foreshadows the future ("the next chapters will add generated commands and exit codes") and asks the assistant to justify a design against that trajectory — delegating the *comparison of two designs*, not the decision. The instruction to "ask the assistant why every field exists" is itself teaching a review habit, not outsourcing one.

**Centerpiece:** a privacy diagnostic when `request` is private and a test in another module tries to construct `Cli` directly — three named fixes (public field / constructor / parse through Clap), with an explicit rule for choosing: public fields are fine for data-only types, constructors matter once a type has invariants to protect.

**Answer key:** "The AI can propose module layouts; you decide which boundaries are justified now." Visibility is framed as a decision with a shelf life, not a fixed property.

**Watch for:** `RunSummary` is introduced here with real weight and *quietly retired* in Chapter 8. Nothing in Chapters 3–7 signals that transition — it's Chapter 8 itself that explains it (see below). If you're facilitating live, mention this now so it lands as "this will change on purpose" rather than "I noticed something disappear."

### Chapter 3 — Errors are part of the design

**Payload:** treating error-case design as a first-class product decision, not a defensive afterthought bolted on at the end.

**Delegated vs. authored:** the AI prompt's first instruction is "enumerate failure cases... before implementing," with an explicit ban on the assistant adding model or configuration errors early — bounding scope is authored by the instructor, enumeration is delegated.

**Centerpiece:** changing `run`'s return type to `Result<RunSummary>` breaks the caller with `no field 'exit_code' on type Result<RunSummary, anyhow::Error>` — the compiler surfacing that the caller was silently ignoring a possible state. The chapter explicitly warns against the tempting wrong fix (returning exit code 1 for every internal failure), which erases causal information.

**Answer key:** "AI often writes only the happy path unless asked for failure cases." This names a specific, generalizable AI failure mode the learner has just watched happen (or watched themselves prevent, if they used the bounded prompt correctly).

**Watch for:** the whitespace-trimming test (`surrounding_whitespace_is_trimmed`) is deliberately framed as "a product decision" rather than an obviously correct behavior — a good discussion prompt if running this as a cohort: what should trimming policy actually be, and who decides?

---

## Part II — Build an app with AI inside it

### Chapter 4 — The first model call

**Payload:** establishing, at the exact moment AI output enters the program, that it is *data* — "generated text is still data even when it looks like a shell command" is the Reflection's first line, and everything downstream (Ch. 6 onward) depends on the learner internalizing this before it becomes an abstract security policy.

**Delegated vs. authored:** the AI prompt asks for "unfamiliar HTTP and Serde plumbing," explicitly scoped as the AI's strength; the chapter's own prose is reserved for the boundary the AI can't be trusted to get right unprompted — "you remain responsible for checking the provider schema and the security boundary."

**Centerpiece:** two things, deliberately paired — a feature-gate compile error (`reqwest::blocking` needs `features = ["blocking"]`) as a mechanical fix, immediately followed by the chapter's real point: why `extract_output_text` must return an *owned* `String`, not `&str`, because the caller needs a result independent of the local `response`'s lifetime.

**Watch for:** this chapter contains the course's most consequential architectural pivot, flagged explicitly in its own text: "`RunSummary` served its purpose while the application had one linear path to observe... orchestration will grow into its own module and eventually return a plain `Result<i32>` once traits are introduced in Chapter 8. That change is deliberate, not an oversight." Read that note aloud if facilitating live — it's easy to skim past as a throwaway line when it's actually the chapter warning the reader about Chapter 8 three chapters early.

### Chapter 5 — Configuration instead of hard-coded behavior

**Payload:** making configuration *precedence* (defaults < file < environment) an explicit, testable rule rather than an emergent property of code order.

**Delegated vs. authored:** the second AI prompt is the interesting one — "identify every place this design can panic or accidentally expose a secret" — and the chapter states outright why it matters: "generating configuration code is easy; reasoning about global process state and secret handling is where review earns its keep." That's the course naming, explicitly, a case where AI's draft needs a second, differently-framed prompt rather than a first read-through.

**Centerpiece:** the partial-move example (`config.ai.model` moves the field, leaving `config.ai` partially moved even though the rest of `config` is untouched). This is not a one-off illustration — the *identical* example recurs in `troubleshooting.md`, `proposal.md`'s "teaching real mistakes" section, and Chapter 14's Compiler Conversation. It's the course's canonical ownership example, deliberately reused rather than varied, so a learner who struggled with it once has a second and third chance to consolidate it in different contexts.

**Watch for:** this is a good chapter to point learners at [Brown's "Fixing Ownership Errors" page](https://rust-book.cs.brown.edu/ch04-03-fixing-ownership-errors.html) (in Further learning) proactively — it's a page with no equivalent in the official Book, built for exactly this failure mode, and most learners won't think to look for an "experimental" fork mid-chapter unless pointed there.

### Chapter 6 — Treat model output as untrusted

**Payload:** the chapter's key invariant, stated as a pull-quote — "Default mode never asks a shell to interpret model output" — and a worked example of a validated domain type (`ValidatedCommand`) that's harder to construct wrong than right.

**Delegated vs. authored:** notably, the second AI prompt asks the assistant to attack its *own* prior draft ("find bypasses involving whitespace, quoted operators, newlines..."), with an explicit rule that "AI is useful for breadth here; the test oracle must still be deterministic" — the course modeling adversarial self-review as a distinct step from initial generation.

**Centerpiece:** the tempting-but-wrong `starts_with` check for tool matching — "if `git` is allowed, that would also accept `git-malware`" — a concrete, memorable illustration of why exact matching beats prefix matching for an allowlist, stated as a rule ("do not") rather than left as a subtlety to discover later.

**Watch for:** the chapter explicitly, honestly documents a known limitation — the scanner isn't quote-aware and will reject `rg 'a|b'` as a false positive — and explicitly defers fixing it to Chapter 10. This is worth calling out to learners as a pattern in its own right: shipping an honestly-scoped, documented gap is a legitimate engineering choice, not a failure to finish.

### Chapter 7 — Execute the safe path

**Payload:** the precise semantic gap between direct process execution and a shell — what you gain (an entire class of injection becomes structurally impossible) and what you lose (glob expansion, which the application must now define deliberately).

**Delegated vs. authored:** the second AI prompt ("compare direct argument execution with `sh -c`... identify which differences are security properties and which are user-experience tradeoffs") is framed as "a productive use of AI: it can enumerate edge cases quickly" — but paired immediately with "verify the important claims with focused integration tests," refusing to let enumeration substitute for verification.

**Centerpiece:** the distinction between `Command::status()` returning `Err` (the child never started — a Rust `Result` error) and a successful `status()` containing a nonzero code (the child ran and reported failure — ordinary data). This Result-vs-data distinction is a recurring shape in the course and worth naming explicitly if a learner conflates the two.

**Watch for:** `Command::new("find").args([".", "-name", "*.rs"])` passing `*.rs` *literally* (no shell to expand it) is the seed of Chapter 13's much larger discussion about glob semantics differing between direct execution and a frozen script — mention that this isn't the last time glob behavior matters.

---

## Part III — Use AI without surrendering judgment

### Chapter 8 — Traits make external systems replaceable

**Payload:** the biggest structural pivot in the course, and the chapter says so directly: `RunSummary` from Chapter 2 is explicitly retired here, replaced by orchestration returning a plain `Result<i32>`, with the reasoning given in-line ("now that generation, validation, and execution are independently testable behind traits, the exit code is the only outcome orchestration itself needs to report").

**Delegated vs. authored:** the second AI prompt — "explain the tradeoff between generics and trait objects... which choice keeps this tutorial simplest, and when would dynamic dispatch help?" — is a genuine open question the course doesn't answer for the learner; the Stretch exercise asks them to build the `dyn` version themselves and compare.

**Centerpiece:** object safety, introduced not as an abstract rule but as a concrete consequence: the two traits here qualify for `Box<dyn Trait>` specifically because their methods don't return `Self`, use generic parameters, or need compile-time type knowledge. Object safety is taught as "why this works," not "here's a rule to memorize."

**Watch for:** if a learner asks "wait, what happened to `RunSummary`?", the answer is already in the chapter (see above) — point them back to it rather than re-explaining from scratch; the chapter's own framing ("if your own project still wants a richer summary... build it explicitly from this point rather than reviving the old struct") is deliberately the final word on the subject.

### Chapter 9 — Safety modes as explicit states

**Payload:** the sharpest, most classic "make illegal states unrepresentable" moment in the course — two booleans (`unsafe_mode`, `unrestricted`) would represent four combinations, including nonsensical ones; the three-variant enum represents exactly the three states the product actually supports.

**Delegated vs. authored:** the second AI prompt uses the assistant as an adversarial critic against the *learner's own implementation* ("look for paths where unrestricted behavior can occur without typed confirmation... trace every branch from CLI parsing to process creation") — explicitly requiring "concrete paths and tests, not a general assurance," refusing to accept AI reassurance as evidence.

**Centerpiece:** less a single failure and more a structural argument — exhaustive `match` as a maintenance tool ("when a fourth mode is added, the compiler points to policy code that must be reconsidered") and `Copy` justified by what the type *doesn't* own, not by convention.

**Watch for:** this chapter is the direct sequel to Chapter 6's runtime-checked allowlist — same theme (make wrong states hard to construct), now enforced by the type system instead of by a runtime check. Drawing that parallel explicitly helps learners see the course's design philosophy as one idea applied twice, not two unrelated lessons.

### Chapter 10 — Local risk analysis

**Payload:** the course's dedicated lifetimes chapter, added specifically to close a gap identified relative to standard Rust-learning resources (see `divergences.md` and this repository's own design history) — `RiskMarker<'a>` borrows its `token` field directly from the scanned text instead of duplicating it as a hardcoded literal.

**Delegated vs. authored:** notably restrained — the AI prompts here are about adversarial test generation and scanner implementation, not about lifetimes at all. The lifetime explanation is entirely hand-authored in Compiler Conversation, because it's the chapter's actual point and not something to delegate.

**Centerpiece:** two staged `compile_fail` examples, in sequence — first, the struct written *without* a lifetime parameter, producing `E0106: missing lifetime specifier`, establishing that struct fields never get elision the way function signatures sometimes do; second, a value trying to outlive the text it borrows from, producing `E0597`, proving the lifetime is enforced, not decorative. The chapter is unusually candid that `analyze`'s own function signature would actually be *covered* by elision — it's written out explicitly for pedagogical clarity, not because the compiler demands it. That's a rare, deliberately honest moment worth pointing out to learners: not every explicit annotation in idiomatic Rust is compiler-required.

**Watch for:** this is the most conceptually dense chapter in the course. If running a cohort, budget more time here than the uniform template suggests — the two-`compile_fail`-example structure is deliberately paced (missing-lifetime error, then a working fix, then the outlives error) and shouldn't be compressed.

---

## Part IV — Grow into a durable application

### Chapter 11 — History and backward-compatible data

**Payload:** `#[serde(default)]` as a compatibility mechanism with an explicit boundary condition — the chapter states plainly when *not* to use it: "do not put `#[serde(default)]` on a field whose absence cannot be interpreted safely. In that case, version the record or write a custom migration." Defaults are framed as a decision with a scope, not a universal escape hatch.

**Delegated vs. authored:** the second AI prompt asks for a *privacy* review — classifying every history field as necessary, sensitive, secret, or unnecessary — with an explicit reminder that "AI can help enumerate concerns, but product owners must decide what the application retains." Retention policy is named as a human decision the course won't let AI make by default.

**Centerpiece:** the deliberate choice of newline-delimited JSON over a single rewritten array, justified structurally: "one damaged record does not make the rest of the file structurally unreadable." Paired with an honest scope statement — "it is not a database: concurrent writers, durability, indexing, and unbounded growth need additional design" — modeling how to describe a design's limits without apologizing for them.

**Watch for:** the multiline-prompt round-trip test is easy to skim past but is doing real work — it's verifying that NDJSON with embedded newlines in string values still round-trips correctly through Serde's escaping, a subtlety worth surfacing explicitly if a learner is confused why that specific test exists.

### Chapter 12 — An interactive terminal editor

**Payload:** proving that terminal UI logic can be fully unit-tested by refusing to let it touch the terminal — the entire `EditorState` is a pure state machine driven by `EditorInput` values, with rendering and raw-mode kept in "a thin driver."

**Delegated vs. authored:** the AI collaboration script is split into three sequential, individually bounded prompts (state and invariants only → pure transitions and tests only → terminal adapter last), with an explicit rationale: "this sequence keeps the complex logic reviewable and prevents terminal escape codes from dominating every test." This is the clearest example in the course of *sequencing* prompts as a technique, not just bounding a single one.

**Centerpiece:** `RawModeGuard`'s `Drop` implementation, introduced with "cleanup happens as stack values leave scope, including most `?` error paths" — `Drop` taught through a real failure-safety guarantee (a hung terminal is a genuinely bad outcome) rather than as an abstract trait.

**Watch for:** this chapter has by far the largest gap between what's built here and the production implementation (documented in `divergences.md`): reverse incremental search and a help overlay are core, load-bearing features in `src/editor.rs` but appear only as this chapter's optional Stretch exercise. If a learner reads the production source expecting a close match, point them at `divergences.md`'s Chapter 12 section first.

### Chapter 13 — Deterministic commands

**Payload:** "freezing moves AI from runtime to authoring time" — the chapter's opening line is its thesis. A frozen command is reviewed once, then runs forever without a model, without SAI, without even network access.

**Delegated vs. authored:** the AI prompts are deliberately sequenced by *risk order*, not by build order — semantics and edge cases first, header round-trips second, body quoting third, atomic writer with all refusals last — with an explicit instructor note: "ask the AI to review the order of effects. A plausible implementation that creates a temp file before confirmation has already violated the product contract." Ordering of side effects is treated as a reviewable property, not an implementation detail.

**Centerpiece:** the glob-quoting exception — every ordinary token gets shell-quoted except ones containing glob metacharacters, because "your default executor expanded glob-bearing arguments itself... quoting every token in the script would change that observed behavior." This is the payoff of Chapter 7's glob discussion: a script must preserve *observed* execution semantics, not just look correct.

**Watch for:** the Review checklist's ordering is deliberate and worth pointing out explicitly — "confirmation and every refusal occur before filesystem mutation" is listed *first*, ahead of correctness checks, because in this chapter's threat model, writing an unconfirmed file to disk is a worse failure than a rendering bug.

### Chapter 14 — Specification-driven development with AI

**Payload:** unlike every other chapter, this one introduces no new Rust syntax — the "Rust concepts" list is explicitly consolidation ("cross-module ownership, exhaustive enum changes, serialization compatibility, trait boundaries, regression tests, compiler-guided API design"). The chapter's actual subject is process: how to run an AI-assisted change large enough to touch several modules without the chat transcript becoming the only record of why decisions were made.

**Delegated vs. authored:** structured as five sequential, role-bounded prompts (explore only, then propose artifacts only, then challenge the artifacts, then implement in task order, then verify against every scenario) — the most explicit, most fully worked example in the course of "one role at a time," a technique every earlier chapter used partially but never spelled out as a named sequence.

**Centerpiece:** the same partial-move ownership example from Chapter 5 and `proposal.md`, reused verbatim — deliberately, not by oversight. By this point the learner has seen it at least twice; seeing it a third time, now embedded in a cross-module refactor the learner designed themselves, is meant to consolidate it as reflex rather than as a memorized special case.

**Watch for:** the Stretch exercise is unusual — it asks the learner to hand a *fresh* AI session only the archived change and repository, with no access to the original conversation, and see whether it can explain the feature's safety model and compatibility guarantees. This is testing documentation completeness, not implementation correctness, and it's the course's final, sharpest statement of its own thesis: if a fresh reader (human or AI) can't reconstruct your reasoning from what you wrote down, the reasoning wasn't actually captured. If facilitating a cohort, this exercise works well as a closing group exercise even for learners who chose different capstone features — swap archived changes between pairs and see what survives the handoff.
