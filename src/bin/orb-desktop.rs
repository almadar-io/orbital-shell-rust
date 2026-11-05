fn main() -> std::process::ExitCode {
    // Logs on stderr, filtered by RUST_LOG (silent by default); the request reply stays the only stdout.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    orbital_core::desktop_protocol::run(&orbital_shell_rust::desktop::Desktop)
}
