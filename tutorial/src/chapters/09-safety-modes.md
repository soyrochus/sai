# Chapter 9 — Safety modes as explicit states

Some useful requests genuinely need pipes, redirection, or shell expansion. Instead of quietly weakening the default, represent stronger capabilities as explicit modes.

## Product goal

Add three modes with visibly different policy:

| Mode | Shell syntax | Tool allowlist | Confirmation |
|---|---:|---:|---:|
| Default | No | Yes | Normal prompt |
| Unsafe | Yes | Yes | Typed confirmation |
| Unrestricted | Yes | No | Typed confirmation |

## Rust concepts

You will model valid states with an enum, derive CLI parsing, use exhaustive policy methods, and take advantage of `Copy` for a small value with no owned resources.

## Build

Create `src/safety_mode.rs`:

```rust
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum SafetyMode {
    #[default]
    Default,
    Unsafe,
    Unrestricted,
}

impl SafetyMode {
    pub fn uses_shell(self) -> bool {
        matches!(self, Self::Unsafe | Self::Unrestricted)
    }

    pub fn enforces_allowlist(self) -> bool {
        !matches!(self, Self::Unrestricted)
    }

    pub fn requires_typed_confirmation(self) -> bool {
        !matches!(self, Self::Default)
    }
}
```

Expose one option:

```rust
#[arg(long, value_enum, default_value_t)]
mode: SafetyMode,
```

Update validation so it receives `SafetyMode`. Default mode rejects shell syntax. Unsafe mode accepts shell syntax but still checks the first tool. Unrestricted mode skips the tool allowlist.

Before shell execution, require a deliberate phrase:

```rust
pub fn confirm_high_risk(mode: SafetyMode, input: &str) -> bool {
    !mode.requires_typed_confirmation() || input.trim() == "run this command"
}
```

Keep the execution branches visibly separate:

```rust
if mode.uses_shell() {
    Command::new("sh").arg("-c").arg(source).status()?
} else {
    Command::new(program).args(args).status()?
}
```

Do not reconstruct a shell string from parsed tokens. In shell modes, the original source contains meaningful quoting and operators; in default mode, the token vector is the authority.

See the production policy type in [`src/safety_mode.rs`](../../../src/safety_mode.rs).

## AI collaboration script

Ask:

> Model default, unsafe, and unrestricted execution as a Rust enum with capability-query methods. Refactor validation and execution to match exhaustively on the mode. Preserve the original source only for shell execution. Add a policy truth-table test.

Then use AI as a critic:

> Look for paths where unrestricted behavior can occur without typed confirmation, or where default mode can reach a shell. Trace every branch from CLI parsing to process creation.

Require concrete paths and tests, not a general assurance.

## Compiler conversation

An enum is better than two booleans such as `unsafe_mode` and `unrestricted`. Two booleans represent four combinations, including ambiguous states. The enum represents exactly the three states the product supports.

Exhaustive `match` expressions are a maintenance tool. When a fourth mode is added, the compiler points to policy code that must be reconsidered.

Deriving `Copy` is appropriate because the enum is a tiny value with no owned resources. Passing it into multiple policy functions does not move an expensive object or require clones.

Clap’s `ValueEnum` derives a parser from the same set of variants, keeping accepted CLI values aligned with the type.

## Tests

Test the policy as data:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_matrix_is_explicit() {
        let cases = [
            (SafetyMode::Default, false, true, false),
            (SafetyMode::Unsafe, true, true, true),
            (SafetyMode::Unrestricted, true, false, true),
        ];

        for (mode, shell, allowlist, typed) in cases {
            assert_eq!(mode.uses_shell(), shell, "{mode:?}");
            assert_eq!(mode.enforces_allowlist(), allowlist, "{mode:?}");
            assert_eq!(mode.requires_typed_confirmation(), typed, "{mode:?}");
        }
    }

    #[test]
    fn high_risk_confirmation_is_exact_after_trimming() {
        assert!(confirm_high_risk(
            SafetyMode::Unsafe,
            " run this command\n"
        ));
        assert!(!confirm_high_risk(SafetyMode::Unsafe, "yes"));
        assert!(confirm_high_risk(SafetyMode::Default, ""));
    }
}
```

Add an orchestration test for every mode. The executor test double should record whether it was asked to use a shell; a default-mode test must assert `false`.

## Review checklist

- The type represents only valid safety states.
- Default mode cannot reach shell execution.
- Unrestricted mode always requires typed confirmation.
- Unsafe mode still enforces the allowlist.
- Original source and parsed tokens have clearly separated uses.
- The policy matrix is covered by tests.

## Checkpoint

```bash
git add src
git commit -m "tutorial: model explicit safety modes"
git tag tutorial-09-safety-modes
```

Evidence: the capability matrix passes and an orchestration test proves default mode never selects the shell executor branch.

## Stretch exercise

Add a `--dry-run` flag orthogonal to safety mode. Decide whether the combination belongs in a second enum or a separate boolean. Explain why dry-run changes execution behavior but not validation policy.

## Reflection

- How did the enum eliminate invalid combinations?
- Why should “unsafe” and “unrestricted” remain different concepts?
- What must a confirmation protect against, and what can it never guarantee?

## Further learning

- [The Rust Book — Defining an Enum](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)
- [Comprehensive Rust — Enums](https://google.github.io/comprehensive-rust/user-defined-types/enums.html)
- [Comprehensive Rust — Copy Types](https://google.github.io/comprehensive-rust/memory-management/copy-types.html) — why deriving `Copy` here is a real decision, not decoration.
- [Rust by Example — C-like enums](https://doc.rust-lang.org/rust-by-example/custom_types/enum/c_like.html)
- [Rustlings — `08_enums`](https://github.com/rust-lang/rustlings/tree/main/exercises/08_enums)

Next: [Local risk analysis](10-risk-analysis.md).
