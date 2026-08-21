# Chapter 4 — Make the First Model Call

The CLI can now parse input and report errors. In this chapter it crosses the first important boundary: natural language goes to an AI model and a command comes back.

## Product goal

Given:

```text
list Rust files
```

the program should obtain a candidate such as:

```text
find . -name '*.rs'
```

It must only print the command. Execution comes later.

This separation is deliberate. Model output is untrusted data until the rest of the application has validated it.

## Rust concepts

This chapter introduces blocking HTTP at an I/O boundary, JSON values, iterator-based extraction, borrowed response text, and conversion to an owned `String`. The pure parser keeps network behavior separate from data-shape behavior.

## Build

Add the dependencies:

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
serde_json = "1"
```

Create `src/llm.rs`:

```rust
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub fn generate_command(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    request: &str,
) -> Result<String> {
    let response: Value = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "instructions": concat!(
                "Return exactly one command for the user's operating system. ",
                "Do not use Markdown fences or add an explanation."
            ),
            "input": request,
        }))
        .send()
        .context("failed to contact the model provider")?
        .error_for_status()
        .context("model provider returned an error")?
        .json()
        .context("model provider returned invalid JSON")?;

    extract_output_text(&response)
}

pub fn extract_output_text(response: &Value) -> Result<String> {
    let text = response["output"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .find_map(|part| {
            (part["type"] == "output_text")
                .then(|| part["text"].as_str())
                .flatten()
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);

    match text {
        Some(text) => Ok(text),
        None => bail!("model response did not contain output text"),
    }
}
```

Declare `mod llm;` in `main.rs`. Read credentials at the application boundary rather than inside the parser:

```rust
let api_key = std::env::var("OPENAI_API_KEY")
    .context("OPENAI_API_KEY is not set")?;
let client = reqwest::blocking::Client::new();
let command = llm::generate_command(
    &client,
    "https://api.openai.com/v1/responses",
    &api_key,
    "YOUR_MODEL",
    &cli.request,
)?;
println!("{command}");
```

This snippet prints the command directly instead of returning it through `RunSummary`. Treat `RunSummary` as scaffolding for Chapters 2–3: from here on, orchestration will grow into its own module and eventually return a plain `Result<i32>` once traits are introduced in Chapter 8. That change is deliberate, not an oversight.

Choose a model available to your account. The finished SAI implementation supports multiple provider shapes; examine [`src/llm.rs`](../../src/llm.rs) when you are ready to generalize this first client.

## AI collaboration script

Ask your coding assistant:

> Add a blocking HTTP client for the provider Responses endpoint. Keep JSON extraction in a pure function. Return `anyhow::Result<String>`, add context at each I/O boundary, and do not execute the generated command. Include a response-fixture unit test.

Then review the answer:

- Did it put the secret in source code, logs, or a test fixture?
- Does it call `error_for_status()` before decoding success JSON?
- Can response parsing be tested without making a network request?
- Does it reject empty or missing output?

AI is useful here because it can draft unfamiliar HTTP and Serde plumbing. You remain responsible for checking the provider schema and the security boundary.

## Compiler conversation

You may see an error saying `reqwest::blocking` cannot be found. The blocking module is feature-gated, so the dependency must include `features = ["blocking"]`.

The iterator chain in `extract_output_text` demonstrates several Rust ideas:

- `Option` represents a field that may be missing.
- `filter_map` discards values that do not have the expected shape.
- `&str` borrows text owned by the JSON value.
- `to_owned()` creates the returned `String` before `response` goes out of scope.

If you try to return `&str`, the compiler will correctly reject it: the caller needs an owned result whose lifetime is independent of the local response.

## Tests

Add fixture-driven tests beside the parser:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_output_text() {
        let response = json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "  find . -name '*.rs'  "
                }]
            }]
        });

        assert_eq!(
            extract_output_text(&response).unwrap(),
            "find . -name '*.rs'"
        );
    }

    #[test]
    fn rejects_missing_text() {
        let error = extract_output_text(&json!({"output": []})).unwrap_err();
        assert!(error.to_string().contains("did not contain"));
    }
}
```

Run:

```bash
cargo test
```

Keep live API tests out of the default unit suite. They are slow, cost money, require secrets, and can fail for reasons unrelated to your code.

## Review checklist

- The API key comes from the environment.
- The program prints but does not execute model output.
- HTTP, status, JSON, and schema failures have distinct context.
- Response extraction has offline tests.
- No test depends on a live provider.

## Checkpoint

Commit and tag the working state:

```bash
git add Cargo.toml Cargo.lock src
git commit -m "tutorial: call the first AI model"
git tag tutorial-04-first-model-call
```

Evidence: `cargo test` passes, and a manual invocation with valid credentials prints exactly one candidate command.

## Stretch exercise

Define a small request and response struct with `serde::{Serialize, Deserialize}` instead of building and reading a generic `Value`. Compare the improved type safety with the additional code required to tolerate provider schema variants.

## Reflection

- Why is generated text still data even when it looks like a shell command?
- Which failures belong to HTTP, JSON decoding, and application validation?
- Where did ownership force you to make the lifetime of returned data explicit?
