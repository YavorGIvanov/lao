fn main() {
    if let Err(error) = lao_daemon::run() {
        eprintln!("lao-daemon: {error}");
        std::process::exit(1);
    }
}
