mod tools;

use std::time::Duration;

use clap::{Parser, Subcommand};
use rmcp::{transport::stdio, ServiceExt};
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(name = "mcp-watch", about = "Condition watcher — MCP server & CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the MCP stdio server (default when no subcommand)
    Serve,

    /// Wait until a TCP port accepts connections
    Port {
        host: String,
        port: u16,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },

    /// Wait until a filesystem event occurs on a path
    File {
        path: String,
        #[arg(short, long, default_value = "create")]
        event: String,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },

    /// Wait until a URL returns an expected HTTP status code
    Url {
        url: String,
        #[arg(short, long, default_value = "200")]
        status: u16,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },

    /// Wait until a process exits
    Pid {
        pid: u32,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },

    /// Wait until a Docker container exits
    Docker {
        container: String,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },

    /// Wait until a GitHub Actions run completes
    #[command(name = "gh-run")]
    GhRun {
        run_id: String,
        #[arg(short, long)]
        repo: Option<String>,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },

    /// Wait until a shell command exits with code 0
    #[command(name = "cmd")]
    Cmd {
        command: String,
        #[arg(short, long, default_value = "5")]
        interval: u64,
        #[arg(short, long, default_value = "300")]
        timeout: u64,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Command::Serve) => run_server().await,
        Some(cmd) => run_cli(cmd).await,
    }
}

async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting mcp-watch server");

    let service = tools::NotifyServer::new()
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("serving error: {:?}", e))?;

    service.waiting().await?;
    Ok(())
}

async fn run_cli(cmd: Command) -> Result<(), Box<dyn std::error::Error>> {
    let ct = CancellationToken::new();

    let result = match cmd {
        Command::Port {
            host,
            port,
            timeout,
        } => tools::port::wait(&host, port, Duration::from_secs(timeout), ct).await,
        Command::File {
            path,
            event,
            timeout,
        } => tools::file::wait(&path, &event, Duration::from_secs(timeout), ct).await,
        Command::Url {
            url,
            status,
            timeout,
        } => tools::url::wait(&url, status, Duration::from_secs(timeout), ct).await,
        Command::Pid { pid, timeout } => {
            tools::pid::wait(pid, Duration::from_secs(timeout), ct).await
        }
        Command::Docker { container, timeout } => {
            tools::docker::wait(&container, Duration::from_secs(timeout), ct).await
        }
        Command::GhRun {
            run_id,
            repo,
            timeout,
        } => tools::ghrun::wait(&run_id, repo.as_deref(), Duration::from_secs(timeout), ct).await,
        Command::Cmd {
            command,
            interval,
            timeout,
        } => {
            tools::command::wait(
                &command,
                Duration::from_secs(interval),
                Duration::from_secs(timeout),
                ct,
            )
            .await
        }
        Command::Serve => unreachable!(),
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    if result.status == "error" {
        std::process::exit(2);
    } else if result.status == "timeout" {
        std::process::exit(1);
    }

    Ok(())
}
