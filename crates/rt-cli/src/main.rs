use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rt-cli", about = "RustTraycer process lifecycle CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Exec the rt-host binary (host writes pid.json).
    Start,
    /// SIGTERM the host named in pid.json (idempotent).
    Stop,
    /// Outside view: paths + is the host pid alive.
    Doctor,
    /// pid.json only: alive, pid, rpcUrl, dataDir. No /rpc.
    Status,
    /// Print the tail of host.log (no --follow).
    Logs {
        /// Lines to print (default 200, 1..=10000).
        #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..=10_000))]
        lines: u32,
    },
    /// Delete host.db (+ wal/shm). Requires --yes. Refuses if host is running.
    ResetDb {
        #[arg(long)]
        yes: bool,
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
        Command::Logs { lines } => {
            let text = rt_cli::logs(lines)?;
            print!("{text}");
            Ok(())
        }
        Command::ResetDb { yes } => {
            rt_cli::reset_db(yes)?;
            println!("reset-db ok");
            Ok(())
        }
    }
}
