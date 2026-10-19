//! The whole command-line surface, plus the exit-code contract.
//!
//! There is **no** `--port`: the port is the single-instance lock and is
//! hard-coded by the agent hooks, so it must not be overridable.

use std::fmt;

/// Normal exit: served and shut down cleanly, or a hub was already running.
pub const EXIT_OK: u8 = 0;
/// A real failure (could not bind for a reason other than "already taken",
/// listener died mid-flight).
pub const EXIT_FAILURE: u8 = 1;
/// The command line could not be understood.
pub const EXIT_USAGE: u8 = 2;

pub const USAGE: &str = "\
nerve-hub - Nerve status hub daemon

Usage:
  nerve-hub serve [-v|--verbose]
  nerve-hub hook
  nerve-hub ensure

Commands:
  ensure               Reuse or start the single local hub and await readiness.
  serve                Bind 127.0.0.1:17890 and serve the ingest + stream
                       contract in the foreground.
  hook                 Codex command hook: read one host event from stdin
                       and POST it to the local hub. Always exits 0.

Options:
  -v, --verbose        Debug logs (event names, job ids, attach/detach).
                       Override with RUST_LOG. Files go under the OS log
                       directory (macOS: ~/Library/Logs/Nerve/).
  -h, --help           Print this message.

The port is fixed. It doubles as the single-instance lock and is hard-coded
by the agent hooks, so it is deliberately not a flag. If another hub already
holds it, this one reuses a compatible Nerve instance. An incompatible hub
or another service on this port is an explicit error.";

/// What the binary was asked to do.
#[derive(Debug)]
pub enum Command {
    Serve(ServeArgs),
    Hook,
    Ensure,
    Help,
}

impl Command {
    /// Parse arguments **without** the program name.
    pub fn parse<I>(args: I) -> Result<Self, UsageError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut args = args.into_iter();
        let Some(command) = args.next() else {
            return Err(UsageError::new("missing command; expected `serve`"));
        };

        match command.as_str() {
            "-h" | "--help" => Ok(Self::Help),
            "serve" => {
                let rest: Vec<String> = args.collect();
                if rest.iter().any(|arg| arg == "-h" || arg == "--help") {
                    return Ok(Self::Help);
                }
                ServeArgs::parse(rest).map(Self::Serve)
            }
            "hook" => Ok(Self::Hook),
            "ensure" if args.next().is_none() => Ok(Self::Ensure),
            other => Err(UsageError::new(format!(
                "unknown command `{other}`; expected `serve` or `hook`"
            ))),
        }
    }
}

/// Settings for `nerve-hub serve`.
#[derive(Debug)]
pub struct ServeArgs {
    verbose: bool,
}

impl ServeArgs {
    /// `debug` filter when `RUST_LOG` is unset.
    pub fn verbose(&self) -> bool {
        self.verbose
    }

    fn parse(args: Vec<String>) -> Result<Self, UsageError> {
        let mut verbose = false;
        let args = args.into_iter();

        for arg in args {
            if arg == "-v" || arg == "--verbose" {
                verbose = true;
                continue;
            }
            return Err(UsageError::new(format!("unexpected argument `{arg}`")));
        }

        Ok(Self { verbose })
    }
}

/// An unusable command line. Callers map this to [`EXIT_USAGE`].
#[derive(Debug)]
pub struct UsageError {
    message: String,
}

impl UsageError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for UsageError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_defaults_are_quiet() {
        let args = ServeArgs::parse(vec![]).unwrap();
        assert!(!args.verbose());
    }

    #[test]
    fn serve_accepts_verbose_and_rejects_grace() {
        assert!(ServeArgs::parse(vec!["-v".into()]).unwrap().verbose());
        assert!(ServeArgs::parse(vec!["--grace-secs".into(), "10".into()]).is_err());
    }

    #[test]
    fn serve_rejects_unknown_flags() {
        assert!(ServeArgs::parse(vec!["--port".into(), "9".into()]).is_err());
    }
}
