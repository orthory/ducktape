use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;

use duckdnsd::{
    CaStore, ControlClient, ControlRequest, DnsHandler, LeafResolver, SharedState,
    configured_control_address, configured_state_dir, control_token_path, load_or_create_token,
    run_control, run_dns, run_https, tls_config,
};
use tokio::net::{TcpListener, UdpSocket};

const USAGE: &str = "\
duckdnsd — device-local DuckDNS DNS/HTTPS helper

usage:
  duckdnsd serve [--state-dir PATH] [--dns-listen ADDR] [--https-listen ADDR] [--control-listen ADDR]
  duckdnsd register --workspace ID --ingress ADDR --name HOST... [--lease SECONDS] [connection flags]
  duckdnsd clear --workspace ID [connection flags]
  duckdnsd status [connection flags]
  duckdnsd root-ca [--state-dir PATH]
  duckdnsd rotate-ca [--state-dir PATH]

connection flags:
  --state-dir PATH          helper state containing control.token
  --control-listen ADDR     default 127.77.0.1:45853

serve defaults:
  DNS UDP/TCP  127.77.0.1:53
  HTTPS        127.77.0.1:443
  control      127.77.0.1:45853
";

#[tokio::main]
async fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("duckdnsd: {error}");
            ExitCode::from(1)
        }
    }
}

async fn run(mut arguments: Vec<String>) -> Result<(), String> {
    if arguments.is_empty() || matches!(arguments[0].as_str(), "help" | "--help" | "-h") {
        print!("{USAGE}");
        return Ok(());
    }
    let command = arguments.remove(0);
    match command.as_str() {
        "serve" => serve(Options::parse(arguments)?).await,
        "register" => register(Options::parse(arguments)?).await,
        "clear" => clear(Options::parse(arguments)?).await,
        "status" => status(Options::parse(arguments)?).await,
        "root-ca" => root_ca(Options::parse(arguments)?),
        "rotate-ca" => rotate_ca(Options::parse(arguments)?),
        other => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    }
}

async fn serve(options: Options) -> Result<(), String> {
    options.reject_names()?;
    let state_dir = options.state_dir();
    let dns: SocketAddr = options
        .value("dns-listen")
        .unwrap_or("127.77.0.1:53")
        .parse()
        .map_err(|error| format!("invalid --dns-listen: {error}"))?;
    let https: SocketAddr = options
        .value("https-listen")
        .unwrap_or("127.77.0.1:443")
        .parse()
        .map_err(|error| format!("invalid --https-listen: {error}"))?;
    let control = options.control_address()?;
    for (name, address) in [("DNS", dns), ("HTTPS", https), ("control", control)] {
        if !address.ip().is_loopback() {
            return Err(format!("{name} listener must be loopback, got {address}"));
        }
    }

    let ca = CaStore::load_or_create(&state_dir)?;
    let token = load_or_create_token(&state_dir)
        .map_err(|error| format!("load DuckDNS control token: {error}"))?;
    let state = SharedState::default();
    let handler = match https.ip() {
        IpAddr::V4(address) => DnsHandler::new(state.clone(), Some(address), None)?,
        IpAddr::V6(address) => DnsHandler::new(state.clone(), None, Some(address))?,
    };
    let udp = UdpSocket::bind(dns)
        .await
        .map_err(|error| format!("bind DuckDNS UDP {dns}: {error}"))?;
    let tcp = TcpListener::bind(dns)
        .await
        .map_err(|error| format!("bind DuckDNS TCP {dns}: {error}"))?;
    let https_listener = TcpListener::bind(https)
        .await
        .map_err(|error| format!("bind DuckDNS HTTPS {https}: {error}"))?;
    let control_listener = TcpListener::bind(control)
        .await
        .map_err(|error| format!("bind DuckDNS control {control}: {error}"))?;
    let tls = tls_config(LeafResolver::new(ca.clone()));
    let control_state = state.clone();
    let https_state = state.clone();
    println!(
        "duckdnsd: DNS={dns} HTTPS={https} control={control} installation={} root_ca={} token={}",
        ca.installation_id(),
        state_dir.join(duckdnsd::ROOT_CERT_FILE).display(),
        control_token_path(&state_dir).display()
    );

    tokio::try_join!(
        async {
            run_dns(udp, tcp, handler)
                .await
                .map_err(|error| error.to_string())
        },
        async {
            run_control(control_listener, control_state, token)
                .await
                .map_err(|error| error.to_string())
        },
        async {
            run_https(https_listener, tls, https_state)
                .await
                .map_err(|error| error.to_string())
        }
    )?;
    Ok(())
}

async fn register(options: Options) -> Result<(), String> {
    let workspace = options.required("workspace")?.to_owned();
    let ingress = options
        .required("ingress")?
        .parse()
        .map_err(|error| format!("invalid --ingress: {error}"))?;
    let lease_seconds = options
        .value("lease")
        .unwrap_or("30")
        .parse()
        .map_err(|error| format!("invalid --lease: {error}"))?;
    if options.names.is_empty() {
        return Err("register requires at least one --name".into());
    }
    let client = options.control_client()?;
    let status = client
        .request(ControlRequest::Register {
            workspace_id: workspace,
            ingress,
            names: options.names,
            lease_seconds,
        })
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&status).map_err(|error| error.to_string())?
    );
    Ok(())
}

async fn clear(options: Options) -> Result<(), String> {
    options.reject_names()?;
    let workspace = options.required("workspace")?.to_owned();
    let status = options
        .control_client()?
        .request(ControlRequest::Clear {
            workspace_id: workspace,
        })
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&status).map_err(|error| error.to_string())?
    );
    Ok(())
}

async fn status(options: Options) -> Result<(), String> {
    options.reject_names()?;
    let status = options
        .control_client()?
        .request(ControlRequest::Status)
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&status).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn root_ca(options: Options) -> Result<(), String> {
    options.reject_names()?;
    let state_dir = options.state_dir();
    CaStore::load_or_create(&state_dir)?;
    let pem = std::fs::read_to_string(state_dir.join(duckdnsd::ROOT_CERT_FILE))
        .map_err(|error| format!("read DuckDNS root CA: {error}"))?;
    print!("{pem}");
    Ok(())
}

fn rotate_ca(options: Options) -> Result<(), String> {
    options.reject_names()?;
    let ca = CaStore::rotate(&options.state_dir())?;
    println!("rotated DuckDNS CA installation={}", ca.installation_id());
    Ok(())
}

#[derive(Default)]
struct Options {
    values: std::collections::BTreeMap<String, String>,
    names: Vec<String>,
}

impl Options {
    fn parse(arguments: Vec<String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(flag) = arguments.next() {
            let Some(name) = flag.strip_prefix("--") else {
                return Err(format!("unexpected argument {flag:?}"));
            };
            let value = arguments
                .next()
                .ok_or_else(|| format!("{flag} needs a value"))?;
            if name == "name" {
                options.names.push(value);
            } else if options.values.insert(name.into(), value).is_some() {
                return Err(format!("{flag} was supplied more than once"));
            }
        }
        Ok(options)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    fn required(&self, name: &str) -> Result<&str, String> {
        self.value(name).ok_or_else(|| format!("missing --{name}"))
    }

    fn state_dir(&self) -> PathBuf {
        self.value("state-dir")
            .map(PathBuf::from)
            .unwrap_or_else(configured_state_dir)
    }

    fn control_address(&self) -> Result<SocketAddr, String> {
        match self.value("control-listen") {
            Some(value) => value
                .parse()
                .map_err(|error| format!("invalid --control-listen: {error}")),
            None => configured_control_address(),
        }
    }

    fn control_client(&self) -> Result<ControlClient, String> {
        let state_dir = self.state_dir();
        ControlClient::from_token_file(self.control_address()?, &control_token_path(&state_dir))
    }

    fn reject_names(&self) -> Result<(), String> {
        if self.names.is_empty() {
            Ok(())
        } else {
            Err("--name is valid only for register".into())
        }
    }
}
