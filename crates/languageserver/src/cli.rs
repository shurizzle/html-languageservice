//! Command-line interface of the HTML language server.
//!
//! The flags mirror the ones understood by `createConnection()` from
//! `vscode-languageserver`, which is the runtime behind the
//! `vscode-langservers-extracted` npm bin (`bin/vscode-html-language-server`).
//!
//! Differences from upstream:
//! - `--node-ipc` is rejected: it needs a Node.js runtime (`process.send`).
//! - With no transport flag the server defaults to `--stdio` (upstream throws).

use std::fmt;

/// The transport the server should speak LSP over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// Standard input/output.
    Stdio,
    /// Connect to `127.0.0.1:<port>`.
    ///
    /// Upstream's `createServerSocketTransport(port)` connects to a loopback
    /// socket owned by the client.
    Socket { port: u16 },
    /// Connect to a named pipe (Windows) or Unix domain socket (POSIX).
    Pipe { name: String },
}

/// Parsed command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub transport: Transport,
    /// `--clientProcessId`: exit when this process is gone.
    pub client_process_id: Option<u32>,
    pub help: bool,
    pub version: bool,
}

/// Error raised while parsing the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> CliError {
        CliError {
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

impl Options {
    /// Parse the arguments that follow the program name.
    pub fn parse<I, S>(args: I) -> Result<Options, CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let argv: Vec<String> = args.into_iter().map(Into::into).collect();
        let mut transport = None;
        let mut client_process_id = None;
        let mut help = false;
        let mut version = false;
        let mut i = 0;

        while i < argv.len() {
            let arg = argv[i].as_str();
            match arg {
                "-h" | "--help" => help = true,
                "-v" | "--version" => version = true,
                "--stdio" => transport = Some(Transport::Stdio),
                "--node-ipc" => {
                    return Err(CliError::new(
                        "`--node-ipc` is not supported: it requires a Node.js runtime",
                    ));
                }
                "--socket" => {
                    let value = argv
                        .get(i + 1)
                        .ok_or_else(|| CliError::new("missing value for `--socket`"))?;
                    transport = Some(Transport::Socket {
                        port: parse_port(value)?,
                    });
                    i += 1;
                }
                "--pipe" => {
                    let value = argv
                        .get(i + 1)
                        .ok_or_else(|| CliError::new("missing value for `--pipe`"))?;
                    transport = Some(Transport::Pipe {
                        name: value.clone(),
                    });
                    i += 1;
                }
                "--clientProcessId" => {
                    let value = argv
                        .get(i + 1)
                        .ok_or_else(|| CliError::new("missing value for `--clientProcessId`"))?;
                    client_process_id = Some(parse_pid(value)?);
                    i += 1;
                }
                _ => {
                    if let Some(value) = arg.strip_prefix("--socket=") {
                        transport = Some(Transport::Socket {
                            port: parse_port(value)?,
                        });
                    } else if let Some(value) = arg.strip_prefix("--pipe=") {
                        transport = Some(Transport::Pipe {
                            name: value.to_owned(),
                        });
                    } else if let Some(value) = arg.strip_prefix("--clientProcessId=") {
                        client_process_id = Some(parse_pid(value)?);
                    } else if arg.starts_with("--node-ipc=") {
                        return Err(CliError::new(
                            "`--node-ipc` is not supported: it requires a Node.js runtime",
                        ));
                    } else {
                        return Err(CliError::new(format!("unknown argument `{arg}`")));
                    }
                }
            }
            i += 1;
        }

        Ok(Options {
            transport: transport.unwrap_or(Transport::Stdio),
            client_process_id,
            help,
            version,
        })
    }
}

fn parse_port(value: &str) -> Result<u16, CliError> {
    value
        .parse::<u16>()
        .map_err(|_| CliError::new(format!("invalid socket port `{value}`")))
}

fn parse_pid(value: &str) -> Result<u32, CliError> {
    value
        .parse::<u32>()
        .map_err(|_| CliError::new(format!("invalid process id `{value}`")))
}

/// The `--help` text.
pub fn help_text() -> String {
    format!(
        "\
{name} {version}
Rust HTML language server (LSP over stdio, TCP or named pipes).

USAGE:
    {name} [OPTIONS]

OPTIONS:
        --stdio                  Use standard input/output (default)
        --socket <PORT>          Connect to a TCP socket on 127.0.0.1:<PORT>
        --pipe <NAME>            Connect to a named pipe (Windows) or Unix
                                 domain socket (POSIX)
        --clientProcessId <PID>  Exit when the process with this id is gone
    -h, --help                   Print this help
    -v, --version                Print the version

The transport flags mirror `vscode-languageserver`'s createConnection().
`--node-ipc` is not supported because it requires a Node.js runtime.
",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, CliError> {
        Options::parse(args.iter().copied())
    }

    #[test]
    fn defaults_to_stdio() {
        assert_eq!(parse(&[]).unwrap().transport, Transport::Stdio);
    }

    #[test]
    fn parses_socket_in_both_forms() {
        assert_eq!(
            parse(&["--socket", "8080"]).unwrap().transport,
            Transport::Socket { port: 8080 }
        );
        assert_eq!(
            parse(&["--socket=8080"]).unwrap().transport,
            Transport::Socket { port: 8080 }
        );
    }

    #[test]
    fn parses_pipe_in_both_forms() {
        assert_eq!(
            parse(&["--pipe", "/tmp/lsp.sock"]).unwrap().transport,
            Transport::Pipe {
                name: "/tmp/lsp.sock".to_owned()
            }
        );
        assert_eq!(
            parse(&["--pipe=/tmp/lsp.sock"]).unwrap().transport,
            Transport::Pipe {
                name: "/tmp/lsp.sock".to_owned()
            }
        );
    }

    #[test]
    fn parses_client_process_id() {
        assert_eq!(
            parse(&["--clientProcessId", "42"])
                .unwrap()
                .client_process_id,
            Some(42)
        );
        assert_eq!(
            parse(&["--clientProcessId=42"]).unwrap().client_process_id,
            Some(42)
        );
    }

    #[test]
    fn rejects_node_ipc() {
        assert!(parse(&["--node-ipc"]).is_err());
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(parse(&["--nope"]).is_err());
    }

    #[test]
    fn rejects_missing_values() {
        assert!(parse(&["--socket"]).is_err());
        assert!(parse(&["--pipe"]).is_err());
        assert!(parse(&["--clientProcessId"]).is_err());
    }
}
