fn main() {
    if let Err(e) = rt_host::run() {
        eprintln!("rt-host: {e}");
        std::process::exit(e.exit_code());
    }
}
