use gateway_protocol::{
    command_name, ClientHello, DeviceListResult, GatewayCommand, HostDiagnosticsResult,
    HostStatusResult, PairingCreateResult, SessionListResult, SessionSummaryDto, PROTOCOL_VERSION,
};
use gateway_server::GatewayClient;
use host_service::{Host, HostConfig};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

type CliResult = Result<(), Box<dyn Error>>;

#[derive(Debug, Default, Deserialize)]
struct UserConfig {
    #[serde(default)]
    harnesses: HarnessConfig,
}

#[derive(Debug, Default, Deserialize)]
struct HarnessConfig {
    opencode: Option<HarnessEntry>,
}

#[derive(Debug, Deserialize)]
struct HarnessEntry {
    program: Option<PathBuf>,
}

fn load_user_config() -> UserConfig {
    let path = std::env::var_os("SLIGHT_CONFIG_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".slight/config.json"))
        });
    let Some(path) = path else {
        return UserConfig::default();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return UserConfig::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return UserConfig::default();
    };
    if value.get("harnesses").is_some() {
        serde_json::from_value(value).unwrap_or_default()
    } else {
        UserConfig {
            harnesses: serde_json::from_value(value).unwrap_or_default(),
        }
    }
}

#[derive(Default)]
struct Options {
    flags: HashMap<String, String>,
    positionals: Vec<String>,
}

impl Options {
    fn parse(args: &[String]) -> Self {
        let mut options = Options::default();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if let Some(rest) = arg.strip_prefix("--") {
                if let Some((key, value)) = rest.split_once('=') {
                    options.flags.insert(key.to_string(), value.to_string());
                } else if is_boolean_flag(rest) {
                    options.flags.insert(rest.to_string(), "true".to_string());
                } else if index + 1 < args.len() && !args[index + 1].starts_with("--") {
                    options
                        .flags
                        .insert(rest.to_string(), args[index + 1].clone());
                    index += 1;
                } else {
                    options.flags.insert(rest.to_string(), "true".to_string());
                }
            } else {
                options.positionals.push(arg.clone());
            }
            index += 1;
        }
        options
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.flags.get(key).map(String::as_str)
    }

    fn has(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }

    fn socket_addr(&self) -> String {
        self.get("addr")
            .or_else(|| self.get("bind"))
            .unwrap_or("127.0.0.1:8787")
            .to_string()
    }

    fn token(&self) -> String {
        self.get("token")
            .map(str::to_string)
            .or_else(|| std::env::var("SLIGHT_DEVICE_TOKEN").ok())
            .unwrap_or_else(|| "dev".to_string())
    }

    fn json(&self) -> bool {
        self.has("json")
    }
}

fn is_boolean_flag(key: &str) -> bool {
    matches!(key, "json" | "no-dev" | "help")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        usage();
        std::process::exit(2);
    };
    let options = Options::parse(&args[1..]);
    let result = match command.as_str() {
        "serve" => cmd_serve(&options),
        "status" => cmd_status(&options),
        "sessions" => cmd_sessions(&options),
        "diagnostics" => cmd_diagnostics(&options),
        "devices" => cmd_devices(&options),
        "pairings" => cmd_pairings(&options),
        "pair" => cmd_pair(&options),
        "revoke" => cmd_revoke(&options),
        "help" | "--help" | "-h" => {
            usage();
            Ok(())
        }
        other => {
            eprintln!("unknown command: {other}");
            usage();
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn cmd_serve(options: &Options) -> CliResult {
    let dev_mode = !options.has("no-dev");
    let bind_addrs = options
        .socket_addr()
        .split(',')
        .map(str::trim)
        .map(str::parse)
        .collect::<Result<Vec<SocketAddr>, _>>()?;
    let Some((&bind, additional_binds)) = bind_addrs.split_first() else {
        return Err("at least one bind address is required".into());
    };
    let user_config = load_user_config();
    let defaults = HostConfig::default();
    let config = HostConfig {
        bind,
        additional_binds: additional_binds.to_vec(),
        host_name: options
            .get("host-name")
            .unwrap_or("slight-host")
            .to_string(),
        server_id: options
            .get("server-id")
            .unwrap_or("slight-local")
            .to_string(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        dev_mode,
        dev_token: if dev_mode {
            Some(options.token())
        } else {
            None
        },
        opencode_program: options
            .get("opencode-bin")
            .map(PathBuf::from)
            .or_else(|| {
                user_config
                    .harnesses
                    .opencode
                    .and_then(|entry| entry.program)
            })
            .or(defaults.opencode_program),
        claude_code_program: defaults.claude_code_program,
        codex_program: defaults.codex_program,
        session_store_path: options
            .get("session-store")
            .map(PathBuf::from)
            .unwrap_or_else(|| HostConfig::default().session_store_path),
        log_dir: options
            .get("log-dir")
            .map(PathBuf::from)
            .unwrap_or_else(|| HostConfig::default().log_dir),
        ..HostConfig::default()
    };
    let (mut host, addr) = Host::bind(config)?;
    let listeners = host
        .local_addrs()
        .iter()
        .map(SocketAddr::to_string)
        .collect::<Vec<_>>();
    if options.json() {
        println!(
            "{}",
            serde_json::json!({
                "listening": addr.to_string(),
                "listeners": listeners,
                "dev_mode": dev_mode,
            })
        );
    } else {
        println!(
            "acp-host listening on {} (dev mode: {dev_mode})",
            listeners.join(", ")
        );
    }
    host.serve()?;
    Ok(())
}

fn cmd_status(options: &Options) -> CliResult {
    let mut client = connect(options)?;
    let request_id = client.next_request_id();
    let outcome = client.call(GatewayCommand::new(request_id, command_name::HOST_STATUS))?;
    let status: HostStatusResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("state:            {:?}", status.state);
        println!("host:             {}", status.host_name);
        println!("server_id:        {}", status.server_id);
        println!("listener:         {}", status.listener.unwrap_or_default());
        println!(
            "sessions:         {} ({} active)",
            status.session_count, status.active_session_count
        );
        println!("paired devices:   {}", status.paired_device_count);
        println!("uptime_ms:        {}", status.uptime_ms);
        println!("supported agents: {}", status.supported_agents.join(", "));
    }
    Ok(())
}

fn cmd_sessions(options: &Options) -> CliResult {
    let mut client = connect(options)?;
    let request_id = client.next_request_id();
    let outcome = client.call(GatewayCommand::new(request_id, command_name::SESSION_LIST))?;
    let result: SessionListResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&result.sessions)?);
    } else if result.sessions.is_empty() {
        println!("no sessions");
    } else {
        for session in &result.sessions {
            print_session(session);
        }
    }
    Ok(())
}

fn cmd_diagnostics(options: &Options) -> CliResult {
    let mut client = connect(options)?;
    let request_id = client.next_request_id();
    let outcome = client.call(GatewayCommand::new(
        request_id,
        command_name::HOST_DIAGNOSTICS,
    ))?;
    let diagnostics: HostDiagnosticsResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    } else {
        println!("server:      {}", diagnostics.server_version);
        println!("protocol:    v{}", diagnostics.protocol_version);
        println!("sessions:    {}", diagnostics.sessions.len());
        for session in &diagnostics.sessions {
            println!(
                "  {} [{}] agent={} events={} running={}",
                session.session_id,
                session.status,
                session.agent,
                session.event_count,
                session.running
            );
        }
        println!("recent logs:");
        for log in &diagnostics.recent_logs {
            println!("  {} {} {}", log.timestamp, log.level, log.message);
        }
    }
    Ok(())
}

fn cmd_devices(options: &Options) -> CliResult {
    let mut client = connect(options)?;
    let request_id = client.next_request_id();
    let outcome = client.call(GatewayCommand::new(request_id, command_name::DEVICE_LIST))?;
    let result: DeviceListResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&result.devices)?);
    } else if result.devices.is_empty() {
        println!("no paired devices");
    } else {
        for device in &result.devices {
            println!(
                "{} [{}] revoked={} last_seen={}",
                device.device_id, device.label, device.revoked, device.last_seen_at
            );
        }
    }
    Ok(())
}

fn cmd_pairings(options: &Options) -> CliResult {
    let mut client = connect(options)?;
    let request_id = client.next_request_id();
    let outcome = client.call(GatewayCommand::new(request_id, command_name::PAIRING_LIST))?;
    let result: gateway_protocol::PairingListResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&result.pairings)?);
    } else if result.pairings.is_empty() {
        println!("no active pairings");
    } else {
        for pairing in &result.pairings {
            println!(
                "{} code={} label={} consumed={} expires={}",
                pairing.pairing_id,
                pairing.code,
                pairing.label,
                pairing.consumed,
                pairing.expires_at
            );
        }
    }
    Ok(())
}

fn cmd_pair(options: &Options) -> CliResult {
    let mut client = connect(options)?;
    let label = options.get("label").unwrap_or("paired device");
    let request_id = client.next_request_id();
    let command = GatewayCommand::new(request_id, command_name::PAIRING_CREATE).with_params(
        &gateway_protocol::BeginPairingParams {
            label: label.to_string(),
        },
    );
    let outcome = client.call(command)?;
    let result: PairingCreateResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("pairing code: {}", result.code);
        println!("expires at:   {}", result.expires_at);
        println!("qr payload:   {}", result.qr_payload);
    }
    Ok(())
}

fn cmd_revoke(options: &Options) -> CliResult {
    let device_id = options
        .positionals
        .first()
        .cloned()
        .ok_or("usage: acp-host revoke <device-id>")?;
    let mut client = connect(options)?;
    let request_id = client.next_request_id();
    let command = GatewayCommand::new(request_id, command_name::DEVICE_REVOKE).with_params(
        &gateway_protocol::RevokeDeviceParams {
            device_id: device_id.clone(),
        },
    );
    let outcome = client.call(command)?;
    let result: gateway_protocol::DeviceRevokeResult = outcome.result_as()?;
    if options.json() {
        println!("{}", serde_json::to_string_pretty(&result.device)?);
    } else {
        println!("revoked {}", result.device.device_id);
    }
    Ok(())
}

fn connect(options: &Options) -> Result<GatewayClient, Box<dyn Error>> {
    let addr: SocketAddr = options.socket_addr().parse()?;
    let hello = ClientHello {
        protocol_version: PROTOCOL_VERSION,
        client_id: "acp-host-cli".to_string(),
        client_name: "acp-host-cli".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        device_id: Some("acp-host-cli".to_string()),
        credential: Some(options.token()),
        resume: None,
    };
    Ok(GatewayClient::connect(addr, hello)?)
}

fn print_session(session: &SessionSummaryDto) {
    println!(
        "{} [{}] agent={} dir={} seq={} title={}",
        session.id,
        serde_json::to_value(session.status)
            .ok()
            .and_then(|value| value.as_str().map(String::from))
            .unwrap_or_default(),
        session.agent,
        session.working_directory_label,
        session.last_sequence,
        session.title
    );
}

fn usage() {
    println!(
        "\
acp-host <command> [options]

Commands:
  serve         Run the headless host and gateway
  status        Query host status
  sessions      List sessions
  diagnostics   Query host diagnostics
  devices       List paired devices
  pairings      List active pairings
  pair          Create a pairing artifact
  revoke <id>   Revoke a device

Options:
  --addr <addr>       Gateway address (default 127.0.0.1:8787)
  --token <token>     Device credential / dev token
  --bind <addr,...>   Addresses for serve (default 127.0.0.1:8787)
  --host-name <name>  Host display name
  --label <label>     Pairing label
  --session-store <path>  SQLite session database path
  --log-dir <path>    Host log directory (default ~/.slight/logs)
  --no-dev            Disable development credential mode
  --json              Emit JSON"
    );
}
