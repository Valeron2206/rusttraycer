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
    }
}
