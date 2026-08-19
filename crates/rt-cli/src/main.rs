use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rt-cli", about = "RustTraycer process lifecycle CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Exec the rt-host binary (host writes pid.json).
    Start,
    /// SIGTERM the host named in pid.json (idempotent).
    Stop,
    /// Outside view: paths + is the host pid alive.
    Doctor,
    /// pid.json only: alive, pid, rpcUrl, dataDir. No /rpc.
    Status,
    /// Print the tail of host.log. --follow keeps printing until SIGINT.
    Logs {
        /// Lines to print (default 200, 1..=10000).
        #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..=10_000))]
        lines: u32,
        /// Keep printing new host.log bytes until SIGINT (exit 0).
        #[arg(long)]
        follow: bool,
    },
    /// Delete host.db (+ wal/shm). Requires --yes. Refuses if host is running.
    ResetDb {
        #[arg(long)]
        yes: bool,
    },
    /// Self-hosted rt-sync (C58): thin client to a running loopback host.
    #[command(visible_alias = "rt-sync")]
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
}

#[derive(Debug, Subcommand)]
enum SyncAction {
    /// Push durable archive to a peer host (`sync.push`).
    Push {
        /// Peer host URL (user-owned; Traycer cloud is forbidden).
        #[arg(long = "peer-url")]
        peer_url: String,
    },
    /// Pull durable archive from a peer host into a workspace (`sync.pull`).
    Pull {
        /// Peer host URL (user-owned; Traycer cloud is forbidden).
        #[arg(long = "peer-url")]
        peer_url: String,
        /// Destination workspace on this host.
        #[arg(long = "workspace-id")]
        workspace_id: String,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli.command) {
        eprintln!("rt-cli: {e}");
        std::process::exit(e.exit_code());
    }
}

fn run(cmd: Command) -> Result<(), rt_cli::CliError> {
    match cmd {
        Command::Start => {
            let bin = rt_cli::prepare_start()?;
            rt_cli::exec_host(&bin)
        }
        Command::Stop => {
            match rt_cli::stop()? {
                rt_cli::StopOutcome::NotRunning => println!("not running"),
                rt_cli::StopOutcome::Stopped { pid } => println!("stopped pid {pid}"),
            }
            Ok(())
        }
        Command::Doctor => {
            let report = rt_cli::doctor()?;
            serde_json::to_writer_pretty(std::io::stdout(), &report)?;
            println!();
            Ok(())
        }
        Command::Status => {
            let report = rt_cli::status()?;
            serde_json::to_writer(std::io::stdout(), &report)?;
            println!();
            Ok(())
        }
        Command::Logs { lines, follow } => {
            if follow {
                rt_cli::logs_follow(lines)
            } else {
                let text = rt_cli::logs(lines)?;
                print!("{text}");
                Ok(())
            }
        }
        Command::ResetDb { yes } => {
            rt_cli::reset_db(yes)?;
            println!("reset-db ok");
            Ok(())
        }
        Command::Sync { action } => {
            let op = match action {
                SyncAction::Push { peer_url } => rt_cli::SyncOp::Push { peer_url },
                SyncAction::Pull {
                    peer_url,
                    workspace_id,
                } => rt_cli::SyncOp::Pull {
                    peer_url,
                    workspace_id,
                },
            };
            let inv = rt_cli::prepare_sync(op)?;
            let ok = rt_cli::sync_execute(&inv)?;
            serde_json::to_writer_pretty(std::io::stdout(), &ok)?;
            println!();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn logs_follow_flag_exists() {
        let cli = Cli::try_parse_from(["rt-cli", "logs", "--follow"]).expect("parse");
        match cli.command {
            Command::Logs { follow, lines } => {
                assert!(follow);
                assert_eq!(lines, 200);
            }
            other => panic!("expected Logs, got {other:?}"),
        }
    }

    #[test]
    fn logs_follow_combines_with_lines() {
        let cli =
            Cli::try_parse_from(["rt-cli", "logs", "--follow", "--lines", "10"]).expect("parse");
        match cli.command {
            Command::Logs { follow, lines } => {
                assert!(follow);
                assert_eq!(lines, 10);
            }
            other => panic!("expected Logs, got {other:?}"),
        }
    }

    #[test]
    fn logs_without_follow_defaults() {
        let cli = Cli::try_parse_from(["rt-cli", "logs"]).expect("parse");
        match cli.command {
            Command::Logs { follow, lines } => {
                assert!(!follow);
                assert_eq!(lines, 200);
            }
            other => panic!("expected Logs, got {other:?}"),
        }
    }

    #[test]
    fn sync_push_clap() {
        let cli = Cli::try_parse_from([
            "rt-cli",
            "sync",
            "push",
            "--peer-url",
            "http://127.0.0.1:47800",
        ])
        .expect("parse");
        match cli.command {
            Command::Sync {
                action: SyncAction::Push { peer_url },
            } => assert_eq!(peer_url, "http://127.0.0.1:47800"),
            other => panic!("expected Sync::Push, got {other:?}"),
        }
    }

    #[test]
    fn rt_sync_alias_pull_clap() {
        let cli = Cli::try_parse_from([
            "rt-cli",
            "rt-sync",
            "pull",
            "--peer-url",
            "http://192.168.0.4:9",
            "--workspace-id",
            "ws-1",
        ])
        .expect("parse");
        match cli.command {
            Command::Sync {
                action:
                    SyncAction::Pull {
                        peer_url,
                        workspace_id,
                    },
            } => {
                assert_eq!(peer_url, "http://192.168.0.4:9");
                assert_eq!(workspace_id, "ws-1");
            }
            other => panic!("expected Sync::Pull, got {other:?}"),
        }
    }

    #[test]
    fn sync_push_has_no_secret_flag() {
        let err = Cli::try_parse_from([
            "rt-cli",
            "sync",
            "push",
            "--peer-url",
            "http://127.0.0.1:9",
            "--secret",
            "nope",
        ])
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unexpected argument") || msg.contains("--secret"),
            "{msg}"
        );
    }

    #[test]
    fn sync_requires_subcommand() {
        assert!(Cli::try_parse_from(["rt-cli", "sync"]).is_err());
    }
}
