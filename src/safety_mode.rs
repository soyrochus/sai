//! How much of the safety model applies to a single invocation.
//!
//! The three modes form a ladder rather than independent switches. `--unsafe`
//! relaxes operator blocking; `--unrestricted` relaxes that *and* the tool
//! restriction, and pays for it with inspection that cannot be turned off.
//! Modelling them as one enum makes the combination "unrestricted but still
//! operator-blocked" unrepresentable instead of merely avoided.

use crate::cli::Cli;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SafetyMode {
    /// Tool whitelist and operator blocking both enforced.
    #[default]
    Default,
    /// Operator blocking lifted; the tool whitelist still applies.
    Unsafe,
    /// Tool whitelist and operator blocking both lifted; inspection forced.
    Unrestricted,
}

impl SafetyMode {
    /// Which mode this invocation runs in. `--unrestricted` wins over
    /// `--unsafe`, since it is strictly the broader of the two.
    pub fn from_cli(cli: &Cli) -> Self {
        if cli.unrestricted {
            Self::Unrestricted
        } else if cli.unsafe_mode {
            Self::Unsafe
        } else {
            Self::Default
        }
    }

    /// Whether pipes, redirects, chaining and substitution are permitted.
    pub fn allows_operators(self) -> bool {
        !matches!(self, Self::Default)
    }

    /// Whether the generated command may name a tool that is not configured.
    /// This governs the system prompt as well as validation: restricting only
    /// validation would leave the model producing whitelisted tools anyway.
    pub fn lifts_tool_restriction(self) -> bool {
        matches!(self, Self::Unrestricted)
    }

    /// Whether explanation and confirmation are mandatory and unsuppressible.
    pub fn forces_inspection(self) -> bool {
        matches!(self, Self::Unrestricted)
    }

    /// Whether the command runs through the shell rather than being exec'd
    /// directly. Anything that permits operators has to.
    pub fn uses_shell(self) -> bool {
        self.allows_operators()
    }

    pub fn is_unrestricted(self) -> bool {
        matches!(self, Self::Unrestricted)
    }

    /// Whether this invocation should be recorded as unsafe in the history log.
    /// Unrestricted implies it: the command ran through the shell with
    /// operators permitted, which is what the field has always meant.
    pub fn is_unsafe_for_history(self) -> bool {
        self.allows_operators()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(unsafe_mode: bool, unrestricted: bool) -> Cli {
        Cli {
            unsafe_mode,
            unrestricted,
            ..Default::default()
        }
    }

    #[test]
    fn plain_invocation_is_default_mode() {
        let mode = SafetyMode::from_cli(&cli(false, false));
        assert_eq!(mode, SafetyMode::Default);
        assert!(!mode.allows_operators());
        assert!(!mode.lifts_tool_restriction());
        assert!(!mode.forces_inspection());
        assert!(!mode.uses_shell());
    }

    #[test]
    fn unsafe_lifts_operators_but_keeps_the_whitelist() {
        let mode = SafetyMode::from_cli(&cli(true, false));
        assert_eq!(mode, SafetyMode::Unsafe);
        assert!(mode.allows_operators());
        assert!(!mode.lifts_tool_restriction());
        assert!(!mode.forces_inspection());
        assert!(mode.uses_shell());
    }

    #[test]
    fn unrestricted_lifts_everything_and_forces_inspection() {
        let mode = SafetyMode::from_cli(&cli(false, true));
        assert_eq!(mode, SafetyMode::Unrestricted);
        assert!(mode.allows_operators());
        assert!(mode.lifts_tool_restriction());
        assert!(mode.forces_inspection());
        assert!(mode.uses_shell());
    }

    #[test]
    fn unrestricted_alone_implies_shell_execution_and_no_operator_blocking() {
        // The flag needs no companion: --unrestricted on its own is enough.
        let mode = SafetyMode::from_cli(&cli(false, true));
        assert!(mode.uses_shell(), "--unrestricted must execute through the shell");
        assert!(
            mode.allows_operators(),
            "--unrestricted must skip operator blocking"
        );
    }

    #[test]
    fn unrestricted_wins_when_both_flags_are_given() {
        assert_eq!(
            SafetyMode::from_cli(&cli(true, true)),
            SafetyMode::Unrestricted
        );
    }

    #[test]
    fn unrestricted_without_unsafe_is_unrepresentable() {
        // The enum has no state meaning "lifts the whitelist but still blocks
        // operators", so this holds for every mode by construction.
        for mode in [
            SafetyMode::Default,
            SafetyMode::Unsafe,
            SafetyMode::Unrestricted,
        ] {
            if mode.lifts_tool_restriction() {
                assert!(mode.allows_operators());
                assert!(mode.uses_shell());
            }
        }
    }

    #[test]
    fn history_records_both_relaxed_modes_as_unsafe() {
        assert!(!SafetyMode::Default.is_unsafe_for_history());
        assert!(SafetyMode::Unsafe.is_unsafe_for_history());
        assert!(SafetyMode::Unrestricted.is_unsafe_for_history());
    }
}
