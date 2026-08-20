# Chapter 10 — Explain Risk Deterministically

Confirmation is more useful when the program can point to concrete risky syntax. The analysis must be deterministic: identical text should produce identical markers without another model call.

## Product goal

Scan command text and return stable, structured risk markers with byte positions and explanations.

## Rust concepts

This chapter applies enums, a single-pass state machine, byte-prefix comparison, UTF-8 boundaries, ordered structured results, and adversarial table tests.

## Build

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskKind {
    Pipeline,
    Redirect,
    CommandSubstitution,
    Separator,
    DestructiveFlag,
    BroadWildcard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskMarker {
    pub kind: RiskKind,
    pub byte_offset: usize,
    pub token: &'static str,
}
```

Build a single-pass scanner that tracks quote state:

```rust
pub fn analyze(source: &str) -> Vec<RiskMarker> {
    let bytes = source.as_bytes();
    let mut markers = Vec::new();
    let mut quote: Option<u8> = None;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte == b'\\' && quote != Some(b'\'') {
            index += 2;
            continue;
        }
        if matches!(byte, b'\'' | b'"') {
            quote = if quote == Some(byte) { None }
                else if quote.is_none() { Some(byte) }
                else { quote };
            index += 1;
            continue;
        }
        if quote.is_some() {
            index += 1;
            continue;
        }

        let tail = &bytes[index..];
        let token_start = index == 0 || bytes[index - 1].is_ascii_whitespace();
        let boundary_after = |width: usize| {
            bytes.get(index + width)
                .is_none_or(|byte| byte.is_ascii_whitespace())
        };
        let found = if token_start && tail.starts_with(b"--force") && boundary_after(7) {
            Some((RiskKind::DestructiveFlag, "--force", 7))
        } else if token_start && tail.starts_with(b"-rf") && boundary_after(3) {
            Some((RiskKind::DestructiveFlag, "-rf", 3))
        } else if tail.starts_with(b"$(") {
            Some((RiskKind::CommandSubstitution, "$(", 2))
        } else if tail.starts_with(b"&&") {
            Some((RiskKind::Separator, "&&", 2))
        } else if tail.starts_with(b"||") {
            Some((RiskKind::Separator, "||", 2))
        } else {
            match byte {
                b'|' => Some((RiskKind::Pipeline, "|", 1)),
                b'>' | b'<' => Some((RiskKind::Redirect, if byte == b'>' { ">" } else { "<" }, 1)),
                b';' | b'\n' => Some((RiskKind::Separator, if byte == b';' { ";" } else { "newline" }, 1)),
                b'`' => Some((RiskKind::CommandSubstitution, "`", 1)),
                b'*' => Some((RiskKind::BroadWildcard, "*", 1)),
                _ => None,
            }
        };

        if let Some((kind, token, width)) = found {
            markers.push(RiskMarker { kind, byte_offset: index, token });
            index += width;
        } else {
            index += 1;
        }
    }
    markers
}
```

This deliberately handles ASCII shell metacharacters plus two conservative risk signals: a broad wildcard and the standalone destructive flags `-rf` and `--force`. It compares byte prefixes without slicing the UTF-8 string. Byte offsets are appropriate because every reported marker begins on an ASCII character boundary, but any UI that moves a cursor through arbitrary text must handle UTF-8 boundaries separately. Markers are warnings, not a claim that every occurrence is harmful.

Use markers to render a deterministic warning. They inform confirmation; they do not make shell execution safe.

The production analyzer lives in [`src/risk.rs`](../../src/risk.rs).

## AI collaboration script

Begin with tests:

> Produce an adversarial test matrix for a quote-aware shell-risk scanner. Cover operators outside quotes, literal operators inside single and double quotes, escapes, adjacent operators, command substitution, multiline input, and Unicode before a marker. State the intended result for each case.

After agreeing on policy:

> Implement a deterministic single-pass scanner that returns ordered structured markers with byte offsets. Do not use regexes and do not call an AI model. Explain every state transition.

Finally ask the assistant to attack its own implementation. Treat any claimed bypass as a hypothesis until a test reproduces it.

## Compiler conversation

Rust strings are UTF-8 and cannot be indexed by arbitrary integers. This scanner examines `as_bytes()` because the syntax tokens are ASCII. It deliberately compares byte slices rather than creating `&source[index..]`; after advancing through a multi-byte character, an intermediate byte index would not be a valid string boundary.

For comparison, a safe byte-prefix check is:

```rust
bytes[index..].starts_with(b"&&")
```

This is the kind of subtle bug Rust makes visible once you reason precisely about bytes and strings. An alternative design can iterate with `char_indices()` and retain valid boundaries by construction.

`RiskKind` is `Copy`; `RiskMarker` owns no source text and uses static labels. If markers need to quote arbitrary substrings later, store a `Range<usize>` and borrow from the original text at rendering time.

## Tests

Start with these cases, then add the Unicode regression before accepting the implementation:

```rust
#[test]
fn risk_matrix() {
    let cases = [
        ("rg TODO | head", vec![RiskKind::Pipeline]),
        ("echo 'a|b'", vec![]),
        ("echo \"a>b\"", vec![]),
        ("echo $(whoami)", vec![RiskKind::CommandSubstitution]),
        ("one && two", vec![RiskKind::Separator]),
        ("one\ntwo", vec![RiskKind::Separator]),
        ("rm -rf build", vec![RiskKind::DestructiveFlag]),
        ("find *", vec![RiskKind::BroadWildcard]),
        ("echo '*'", vec![]),
    ];

    for (source, expected) in cases {
        let actual: Vec<_> = analyze(source)
            .into_iter()
            .map(|marker| marker.kind)
            .collect();
        assert_eq!(actual, expected, "{source:?}");
    }
}

#[test]
fn unicode_before_operator_does_not_panic() {
    let markers = analyze("echo café | head");
    assert_eq!(markers[0].kind, RiskKind::Pipeline);
    assert_eq!(&"echo café | head"[markers[0].byte_offset..][..1], "|");
}
```

The Unicode regression protects the byte-prefix design from being replaced by unsafe string slicing later.

Add property tests later if this becomes a security-critical library. A valuable property is that `analyze` must never panic for arbitrary UTF-8 input.

## Review checklist

- Analysis is deterministic and offline.
- Operators inside quotes follow a documented policy.
- Escapes and adjacent multi-byte operators are tested.
- Unicode input cannot cause invalid string slicing.
- Markers preserve stable source order and byte offsets.
- Warnings never claim to prove a command safe.

## Checkpoint

```bash
git add src
git commit -m "tutorial: add deterministic risk analysis"
git tag tutorial-10-risk-analysis
```

Evidence: the adversarial matrix and Unicode regression pass, and repeated analysis yields identical markers.

## Stretch exercise

Use `proptest` to generate arbitrary strings and assert that analysis never panics, offsets are monotonically increasing, and every offset is a valid UTF-8 character boundary.

## Reflection

- Why is deterministic analysis more trustworthy for enforcement than a second AI opinion?
- When are byte offsets preferable to character positions?
- What did the Unicode test reveal about the relationship between `String`, bytes, and indexing?
