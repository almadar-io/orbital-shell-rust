fn main() -> std::process::ExitCode {
    orbital_core::desktop_protocol::run(&orbital_shell_rust::desktop::Desktop)
}
