# Chapter 5 — Configuration Without Surprises

Hard-coded endpoints and model names were fine for one request. A real application needs configuration that is typed, explainable, and predictable.

## Product goal

Load optional YAML configuration, then allow environment variables to override it. Keep secrets out of the file by default.

## Rust concepts

You will use Serde-derived types, `Default`, `Option`, `Path` versus `PathBuf`, ownership during configuration resolution, and explicit source precedence.

## Build

Add:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
```

Create `src/config.rs`:

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ai: AiConfig,
    pub allowed_tools: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            allowed_tools: ["find", "rg", "ls"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key_env: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1/responses".into(),
            model: "YOUR_MODEL".into(),
            api_key_env: "OPENAI_API_KEY".into(),
        }
    }
}

pub fn load(path: Option<&Path>) -> Result<Config> {
    let Some(path) = path else {
        return Ok(Config::default());
    };

    let yaml = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    serde_yaml::from_str(&yaml)
        .with_context(|| format!("failed to parse config {}", path.display()))
}

#[derive(Debug)]
pub struct EffectiveAiConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub allowed_tools: Vec<String>,
}

pub fn resolve(mut config: Config) -> Result<EffectiveAiConfig> {
    if let Ok(value) = std::env::var("SAI_AI_ENDPOINT") {
        config.ai.endpoint = value;
    }
    if let Ok(value) = std::env::var("SAI_AI_MODEL") {
        config.ai.model = value;
    }
    if let Ok(value) = std::env::var("SAI_ALLOWED_TOOLS") {
        config.allowed_tools = value
            .split(',')
            .map(str::trim)
            .filter(|tool| !tool.is_empty())
            .map(str::to_owned)
            .collect();
    }

    let api_key = std::env::var(&config.ai.api_key_env)
        .with_context(|| format!("{} is not set", config.ai.api_key_env))?;

    Ok(EffectiveAiConfig {
        endpoint: config.ai.endpoint,
        model: config.ai.model,
        api_key,
        allowed_tools: config.allowed_tools,
    })
}
```

Add a CLI flag:

```rust
#[arg(long, value_name = "PATH")]
config: Option<PathBuf>,
```

A minimal YAML file now looks like:

```yaml
ai:
  model: YOUR_MODEL
  api_key_env: OPENAI_API_KEY
allowed_tools:
  - find
  - rg
```

The precedence rule is:

```text
compiled defaults < YAML file < environment overrides
```

That rule should appear in `--help` or project documentation. Invisible precedence is a common source of configuration bugs.

The production project has a broader configuration model in [`src/config.rs`](../../src/config.rs). Grow toward it only when the product needs another setting.

## AI collaboration script

Ask:

> Design typed YAML configuration for endpoint, model, and API-key environment-variable name. Use defaults for omitted fields, accept an optional `PathBuf` from Clap, and make the precedence defaults < file < environment explicit. Never deserialize the secret itself from YAML.

Follow with:

> Identify every place this design can panic or accidentally expose a secret. Suggest tests that do not mutate process-wide environment variables in parallel.

The second prompt matters. Generating configuration code is easy; reasoning about global process state and secret handling is where review earns its keep.

## Compiler conversation

`Path` and `PathBuf` have different jobs:

- `PathBuf` owns a path, so it is suitable inside `Cli`.
- `&Path` borrows a path, so it is suitable for `load` while reading it.

The `let Some(path) = path else` construct unwraps the option for the rest of the function while returning early for the no-file case.

Be alert to partial moves. This compiles:

```rust
let model = config.ai.model.clone();
inspect_config(&config);
```

This may not:

```rust
let model = config.ai.model;
inspect_config(&config);
```

Moving the owned `String` out leaves `config.ai` partially moved. Borrow it, clone it when ownership is genuinely needed, or resolve all fields in one place before reusing the parent value.

## Tests

Test defaults and parsing without touching the real environment:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_applied_to_empty_yaml() {
        let config: Config = serde_yaml::from_str("{}").unwrap();
        assert_eq!(config.ai.api_key_env, "OPENAI_API_KEY");
        assert!(config.ai.endpoint.starts_with("https://"));
        assert!(config.allowed_tools.contains(&"find".to_owned()));
    }

    #[test]
    fn yaml_overrides_selected_fields() {
        let config: Config = serde_yaml::from_str(
            "ai:\n  model: tutorial-model\nallowed_tools:\n  - rg\n"
        ).unwrap();
        assert_eq!(config.ai.model, "tutorial-model");
        assert_eq!(config.ai.api_key_env, "OPENAI_API_KEY");
        assert_eq!(config.allowed_tools, ["rg"]);
    }

    #[test]
    fn malformed_yaml_is_rejected() {
        assert!(serde_yaml::from_str::<Config>("ai: [").is_err());
    }
}
```

For precedence tests, prefer refactoring environment lookup behind a function that accepts a map:

```rust
fn override_from(config: &mut Config, env: &std::collections::HashMap<String, String>)
```

That makes tests deterministic and parallel-safe. The application can build the map from `std::env::vars()` once.

## Review checklist

- Defaults, file values, and environment overrides have documented precedence.
- Secrets are resolved from a named environment variable.
- Owned and borrowed path types are used intentionally.
- Missing files and malformed YAML include the path in their errors.
- Tests do not race by repeatedly changing process-wide environment variables.

## Checkpoint

```bash
git add Cargo.toml Cargo.lock src
git commit -m "tutorial: add typed configuration"
git tag tutorial-05-configuration
```

Evidence: parsing tests pass, omitted YAML fields receive defaults, and a missing API-key variable produces a useful error without printing a secret.

## Stretch exercise

Support a provider enum:

```rust
enum Provider {
    OpenAi,
    Azure,
}
```

Use Serde naming attributes so YAML accepts `openai` and `azure`. Make invalid provider names fail during deserialization instead of much later during an HTTP request.

## Reflection

- Why is a type error during configuration loading better than a string comparison deep in the application?
- When is cloning a configuration field the clearest solution, and when is borrowing better?
- Which configuration values are safe to print in diagnostics?
