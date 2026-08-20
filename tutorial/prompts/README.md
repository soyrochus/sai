# Reusable AI prompts

These prompts are deliberately role-specific. Do not ask for planning, implementation, and review in one giant request.

## 1. Plan a bounded change

```text
We are building a Rust CLI incrementally. The current checkpoint passes all tests.

Goal: <one observable behavior>
Out of scope: <what must not change>
Relevant files: <small list>

Propose a plan of no more than five steps. Name the Rust concepts involved, the tests that should fail before implementation, and any design choice I must make. Do not edit code yet.
```

Use this to prevent the assistant from solving a larger problem than the chapter requires.

## 2. Implement one step

```text
Implement only step <N> of the agreed plan.

Before editing, restate the behavioral acceptance criterion. Keep the diff small. Add or update the narrowest relevant test. Do not add dependencies without explaining why the standard library and current dependencies are insufficient.
```

Limitation: a small diff can still be wrong. Read it before running it.

## 3. Explain Rust, not just the fix

```text
Explain this compiler diagnostic in terms of ownership, borrowing, and the values involved in this exact code. Show the point where the value moves or the borrow begins and ends. Give two possible repairs and compare their API trade-offs. Do not edit yet.
```

Use this before accepting a reflexive `.clone()`.

## 4. Debug from evidence

```text
Here is the failing command, complete diagnostic, and relevant code. Identify the smallest causal chain supported by this evidence. Distinguish facts from hypotheses. Suggest one diagnostic check before proposing a fix.
```

Do not paraphrase an error or omit the command that produced it.

## 5. Generate adversarial cases

```text
For this deterministic parser/validator, propose boundary and adversarial inputs. Organize them by invariant. Do not predict that the current code passes. Include quoted text, malformed input, Unicode, empty values, and platform differences where relevant.
```

AI is useful for breadth here; the test oracle must still be deterministic.

## 6. Review a diff

```text
Review this Rust diff against the stated requirement. Look for ownership/API problems, hidden I/O, accidental shell behavior, untested failure paths, backward-compatibility breaks, secret leakage, and scope expansion. Cite exact lines. Do not rewrite code unless I ask.
```

## 7. Review an AI boundary

```text
Classify every value crossing the model boundary as trusted configuration, user input, model output, or deterministic derived data. For each model-produced value, identify validation before it can cause a side effect. Flag any decision that is delegated to the model but should be deterministic.
```

## 8. Finish with evidence

```text
Before claiming completion, map every acceptance criterion to a test or manual check. Run formatting, tests, and Clippy. Report exact results and any work that remains. Do not treat an unchecked task or skipped platform check as complete.
```

## Prompt hygiene

- Include the requirement and relevant code, not the entire repository by default.
- Never paste credentials, personal history logs, or sensitive command output.
- Ask for alternatives before accepting a new dependency or broad abstraction.
- Preserve full diagnostics; they often contain the ownership fact that matters.
- Start a fresh request when the objective changes materially.

