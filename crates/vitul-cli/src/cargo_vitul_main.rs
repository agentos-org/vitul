fn main() {
  if let Err(error) = vitul_cli::run() {
    eprintln!("error: {error:#}");
    std::process::exit(1);
  }
}
