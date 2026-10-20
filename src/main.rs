use std::process::ExitCode;

fn main() -> ExitCode {
    let code = dynimg::cli::run(std::env::args_os());
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
