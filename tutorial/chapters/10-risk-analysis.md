# Chapter 10 — Explain Risk Deterministically

Confirmation is more useful when the program can point to concrete risky syntax. The analysis must be deterministic: identical text should produce identical markers without another model call.

## Product goal

Scan command text and return stable, structured risk markers with byte positions and explanations.

## Rust concepts

This chapter applies enums, a single-pass state machine, byte-prefix comparison, UTF-8 boundaries, named lifetimes, ordered structured results, and adversarial table tests.

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
pub struct RiskMarker<'a> {
    pub kind: RiskKind,
    pub byte_offset: usize,
    pub token: &'a str,
}
```

`token` borrows directly from the text it was found in, so `RiskMarker` must say how long that borrow is allowed to last. A struct that stores a reference always names a lifetime parameter — nothing elides it away. Try writing the field as plain `&str` first and let the compiler explain what's missing before adding `<'a>`.

Build a single-pass scanner that tracks quote state:

```rust
pub fn analyze<'a>(source: &'a str) -> Vec<RiskMarker<'a>> {
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
            Some((RiskKind::DestructiveFlag, 7))
        } else if token_start && tail.starts_with(b"-rf") && boundary_after(3) {
            Some((RiskKind::DestructiveFlag, 3))
        } else if tail.starts_with(b"$(") {
            Some((RiskKind::CommandSubstitution, 2))
        } else if tail.starts_with(b"&&") {
            Some((RiskKind::Separator, 2))
        } else if tail.starts_with(b"||") {
            Some((RiskKind::Separator, 2))
        } else {
            match byte {
                b'|' => Some((RiskKind::Pipeline, 1)),
                b'>' | b'<' => Some((RiskKind::Redirect, 1)),
                b';' | b'\n' => Some((RiskKind::Separator, 1)),
                b'`' => Some((RiskKind::CommandSubstitution, 1)),
                b'*' => Some((RiskKind::BroadWildcard, 1)),
                _ => None,
            }
        };

        if let Some((kind, width)) = found {
            let token = &source[index..index + width];
            markers.push(RiskMarker { kind, byte_offset: index, token });
            index += width;
        } else {
            index += 1;
        }
    }
    markers
}
```

This deliberately handles ASCII shell metacharacters plus two conservative risk signals: a broad wildcard and the standalone destructive flags `-rf` and `--force`. The scanner still finds matches by comparing byte prefixes without slicing the UTF-8 string, exactly as before — but once a match is found, `token` is sliced directly from `source` instead of being copied from a hardcoded literal. That slice can never disagree with what was actually matched, and the earlier `"newline"` placeholder is gone: the token for a bare `\n` is now the literal newline character, because that is what was actually found. Byte offsets are appropriate because every reported marker begins on an ASCII character boundary, but any UI that moves a cursor through arbitrary text must handle UTF-8 boundaries separately. Markers are warnings, not a claim that every occurrence is harmful.

Use markers to render a deterministic warning. They inform confirmation; they do not make shell execution safe.

The production analyzer lives in [`src/safety.rs`](../../src/safety.rs).

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

`RiskKind` is `Copy` and owns nothing, so it needed no lifetime. `RiskMarker` is different: it stores a slice of the text it was built from, so the compiler needs to know how long that borrow is valid. Write the struct without a lifetime parameter first:

```rust,compile_fail
pub struct RiskMarker {
    pub kind: RiskKind,
    pub byte_offset: usize,
    pub token: &str,
}
```

The compiler rejects this with `missing lifetime specifier`. Every other reference you have written so far — a function parameter like `source: &str`, a local variable, a return value borrowed from a single input — has its lifetime worked out automatically by Rust's elision rules. A struct field is the one common place elision does not reach: nothing tells the compiler how long a `RiskMarker` is allowed to outlive the text it points into, so it insists you say so. Adding `<'a>` to both the struct and `token: &'a str` is that answer, not decoration.

`analyze`'s own signature, `fn analyze<'a>(source: &'a str) -> Vec<RiskMarker<'a>>`, is written out in full here because that is the point of this chapter, but it is worth knowing that elision *would* have covered it: with exactly one reference parameter, Rust already assumes every borrowed value in the return type owes its lifetime to that parameter, so `fn analyze(source: &str) -> Vec<RiskMarker<'_>>` compiles identically. Struct definitions never get that shortcut; function signatures sometimes do. Naming `'a` explicitly here makes the invariant readable even where the compiler would have inferred it for you.

The lifetime is not just paperwork — it is enforced. This does not compile:

```rust,compile_fail
let markers = {
    let source = String::from("rm -rf build");
    analyze(&source)
};
println!("{}", markers[0].token);
```

`source` is dropped at the end of the inner block, but `markers` borrows from it and was about to escape that block. The diagnostic will point at `source` and say it does not live long enough. That is exactly the property you want from a risk marker: it is structurally impossible to hold one whose token has outlived the command text it was found in.

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
    assert_eq!(markers[0].token, "|");
    assert_eq!(&"echo café | head"[markers[0].byte_offset..][..1], "|");
}
```

`markers[0].token` borrows from whatever was passed into `analyze`. Here that is a `'static` string literal, so nothing can drop it early — but if `source` had instead been an owned `String` built in a narrower scope, as in the compiler-conversation example above, reading `token` after that scope ended would fail to compile, not merely fail at runtime.

The Unicode regression protects the byte-prefix design from being replaced by unsafe string slicing later.

Add property tests later if this becomes a security-critical library. A valuable property is that `analyze` must never panic for arbitrary UTF-8 input.

## Review checklist

- Analysis is deterministic and offline.
- Operators inside quotes follow a documented policy.
- Escapes and adjacent multi-byte operators are tested.
- Unicode input cannot cause invalid string slicing.
- Markers preserve stable source order and byte offsets.
- Warnings never claim to prove a command safe.
- `token` is sliced from `source`, not duplicated as a separate literal, so it cannot drift from what was actually matched.

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
- Why does `RiskMarker` need an explicit `<'a>` when `analyze`'s own parameter and return type do not strictly need one written out?
