//! Stdio entry point for the wire-level fake ACP agent.
//!
//! This binary is spawned by subprocess integration tests through
//! `acp_adapters::process::spawn`. It speaks ACP over stdin/stdout and exits
//! cleanly when the client closes the pipe.

use acp_adapters::wire_agent::{serve, FakeWireAgentConfig};
use std::io::{BufReader, BufWriter};

fn main() {
    let config = FakeWireAgentConfig::from_env();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let result = serve(
        BufReader::new(stdin.lock()),
        BufWriter::new(stdout.lock()),
        config,
    );
    if let Err(error) = result {
        eprintln!("fake-acp-agent: {error}");
        std::process::exit(1);
    }
}
