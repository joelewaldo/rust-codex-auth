fn main() {
    if let Err(err) = codex_auth::cli::run(std::env::args().skip(1).collect()) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
