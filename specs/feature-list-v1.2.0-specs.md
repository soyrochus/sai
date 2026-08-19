# SAI v1.2.0 Feature List Specs

Purpose: define candidate v1.2.0 features in descending user impact while preserving SAI's product boundary (natural-language command assistant, not a full shell).

## Prioritization Method

Ordering is based on:
- Frequency of user benefit
- Time saved per interaction
- Reduction of friction/errors
- Implementation risk vs. payoff

---

## SPEC-01 (Highest Impact): Interactive Mini Editor Prompt Mode

Status: Proposed
Priority: P0

### Problem
Single-line prompt entry is fast for simple requests but limiting for iterative, complex, or corrected prompts.

### User Value
- Faster prompt authoring with fewer retries
- Better control before model generation
- Lower cognitive load for complex requests

### Scope
Make interactive mini-editor input the default behavior for terminal/CLI usage when prompting in natural language.

CLI:
- Default path: interactive mini-editor is used automatically in terminal sessions.
- Optional compatibility flag: `sai --no-interactive` for legacy single-line input.
- Optional explicit flag (still supported): `sai --interactive`.

Editor capabilities:
- Cursor movement: left/right, home/end
- Basic editing: insert/delete/backspace
- Shortcuts: Ctrl+A, Ctrl+E, Ctrl+K, Ctrl+U, Ctrl+L (clear/redraw prompt area)
- Submit: Enter
- Cancel: Esc or Ctrl+C

Boundary:
- Editor is for NL prompt composition only
- No shell scripting language, pipes, job control, or shell runtime semantics

### Acceptance Criteria
- User can fully edit a prompt before submission
- Interactive mini-editor is the default prompt input behavior in terminal sessions
- Submission routes through existing safety/generation flow unchanged
- Cancel exits cleanly with no side effects
- Works on macOS/Linux/Windows terminals

### Out of Scope
- Full-screen IDE behavior
- Multiple panes or shell-like command execution inside editor

---

## SPEC-02: Prompt History + Reverse Search

Status: Proposed
Priority: P0

### Problem
Users frequently repeat or refine earlier prompts and currently retype them.

### User Value
- Major time savings on repeated tasks
- Faster iteration on previous requests

### Scope
Persistent prompt history separate from execution history.

Capabilities:
- Up/down navigation through prior prompts
- Reverse search (Ctrl+R style)
- Select and edit a historical prompt before submit

Storage:
- Dedicated prompt history file under app config directory
- Rotation/size limit to prevent unbounded growth

### Acceptance Criteria
- History persists across sessions
- Reverse search returns relevant prior prompts quickly
- Selected history entry is editable before submit

### Out of Scope
- Cross-device sync
- Semantic clustering of prompts

---

## SPEC-03: Multi-line Prompt Composition

Status: Proposed
Priority: P1

### Problem
Complex intent is hard to express in one line without losing clarity.

### User Value
- Better prompt precision
- Fewer regeneration cycles and fewer ambiguous outputs

### Scope
Support multi-line NL input in interactive mode.

Capabilities:
- Insert line breaks
- Display prompt line count/length indicator
- Submit as one consolidated prompt payload

### Acceptance Criteria
- Multi-line text is preserved and sent correctly to model
- UI indicates current line and total size
- Existing non-interactive mode behavior remains unchanged

### Out of Scope
- Markdown rendering features
- Rich text formatting

---

## SPEC-04: Preflight Command Card (Before Confirm)

Status: Proposed
Priority: P1

### Problem
Users need a faster way to assess command intent/risk than reading full explanations every time.

### User Value
- Faster trust calibration
- Better decision quality before execution

### Scope
Display a compact preflight summary before execution.

Card fields:
- Generated command
- Primary tool detected
- Scope hint used (if any)
- Safety mode (safe/unsafe)
- Explain requirement source (flag vs force_explain)
- Risk markers (operators, destructive flags, wildcard breadth)

### Acceptance Criteria
- Card appears consistently before confirmation step
- Data accurately reflects actual command/safety evaluation
- Card does not alter execution semantics

### Out of Scope
- Interactive dashboard
- Runtime policy editing from the card

---

## SPEC-05: Scope Picker Shortcuts

Status: Proposed
Priority: P2

### Problem
Users often know the target area but not exact scope syntax.

### User Value
- Better first-try command generation
- Reduced prompt ambiguity

### Scope
Interactive scope selector that can prefill `--scope`.

Initial shortcuts:
- Current directory (`.`)
- Git changed files
- Tracked files only
- Custom glob input

### Acceptance Criteria
- Selected scope is injected into normal generation flow
- Scope choice is visible to user before submit
- Invalid scope input returns clear correction prompt

### Out of Scope
- Deep VCS analytics
- Project graph visualizations

---

## SPEC-06: Inline Explain Delta (Regeneration Comparison)

Status: Proposed
Priority: P2

### Problem
When command output changes between attempts, users may not know why.

### User Value
- Better transparency and learning
- Faster debugging of prompt/config interactions

### Scope
When a command is regenerated in same session, show a concise delta:
- Previous command
- New command
- Plain-language reason summary (heuristic)

### Acceptance Criteria
- Delta appears only when previous command exists
- No effect on final safety validations
- Clear labeling of old vs new command

### Out of Scope
- Full provenance tracing from model internals
- Token-level explanation guarantees

---

## SPEC-07: Safe Dry-Run Validation Hints

Status: Proposed
Priority: P3

### Problem
Some generated commands fail for trivial local reasons (missing path, invalid glob).

### User Value
- Fewer avoidable execution failures
- Better confidence before running

### Scope
Run lightweight local validations before execution for known-safe checks:
- Path existence checks
- Glob expansion sanity
- Obvious flag/argument contradictions (heuristic)

Output:
- Non-blocking warnings by default
- Optional strict mode can block execution on validation failures

### Acceptance Criteria
- Validation is fast and deterministic
- Warnings are clear and actionable
- Does not execute user command during validation phase

### Out of Scope
- Full static command verifier
- Tool-specific semantic emulation

---

## SPEC-08 (Lower Impact): User Profiles (fast/safe/expert)

Status: Proposed
Priority: P3

### Problem
Different users want different default interaction styles.

### User Value
- Faster onboarding
- Less repetitive flag usage

### Scope
Named profile presets that toggle defaults:
- `fast`: minimal prompts, lightweight confirmation
- `safe`: explain + confirm defaults
- `expert`: reduced friction with explicit warnings

Configuration:
- Stored in global config
- Overridable by CLI flags per invocation

### Acceptance Criteria
- Profile selection updates runtime defaults predictably
- Per-command flags always take precedence
- Help/docs clearly show active profile behavior

### Out of Scope
- Org-level policy management
- Remote profile synchronization

---

## Product Boundary Guardrail

All features in this document must preserve SAI's intent:
- SAI is not a full shell replacement
- SAI remains a natural-language to command assistant with explicit safety controls

Disallowed direction for v1.2.0:
- Shell runtime features (pipelines as language primitives, job control UI, script interpreter mode)
- General-purpose REPL shell emulation

---

## Suggested Rollout Order

1. SPEC-01 and SPEC-02
2. SPEC-03 and SPEC-04
3. SPEC-05 and SPEC-06
4. SPEC-07 and SPEC-08
