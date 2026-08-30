fn main() {
    if let Err(error) = lao_cli::run() {
        eprintln!("lao: {error}");
        std::process::exit(1);
    }
}
