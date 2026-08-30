fn main() {
    if let Err(error) = run() {
        eprintln!("lao: {error}");
        std::process::exit(1);
    }
}

fn run() -> std::io::Result<()> {
    let _ = (lao_codex::status(), lao_claude::status());
    match std::env::args_os().nth(1).as_deref() {
        Some(command) if command == "preview" => preview(),
        _ => {
            println!("usage: lao preview");
            Ok(())
        }
    }
}

fn preview() -> std::io::Result<()> {
    let bin = std::env::var_os("LAO_LLAMA_SERVER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/opt/homebrew/bin/llama-server".into());
    let budget = lao_run::plan(&bin, lao_run::Mode::Light)?;
    let model = &lao_model::QWEN;
    println!("model: {}", model.id);
    println!("source: {} @ {}", model.url, model.revision);
    println!("download: {} bytes", model.bytes);
    println!("license: {}", model.license);
    println!("runtime: {}", model.runtime);
    println!("context: {}", model.context);
    println!(
        "Light: {:.2} GiB, {} threads",
        budget.bytes as f64 / (1_u64 << 30) as f64,
        budget.threads
    );
    Ok(())
}
