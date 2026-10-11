//! The `tpt-des` binary. All logic is in the library; see `tpt_des_cli::execute`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print!("{}", tpt_des_cli::USAGE);
        return ExitCode::from(if args.is_empty() { 2 } else { 0 });
    }
    let [command, path] = args.as_slice() else {
        eprint!("{}", tpt_des_cli::USAGE);
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    match tpt_des_cli::execute(command, &text) {
        Ok(outcome) => {
            print!("{}", outcome.output);
            if outcome.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(message) => {
            eprintln!("tpt-des: {message}");
            ExitCode::from(2)
        }
    }
}
