//! Temper's standalone commands (protocol/README.md, section 4).
use std::os::unix::ffi::OsStrExt;
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    skein_fail_stop::install();
    if env::args_os().next().as_deref().is_some_and(temper::fake::git::is_git) {
        return ExitCode::from(temper::fake::git::run(&skein_shell::arguments()));
    }
    if env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new(smith_agent_shell::relay::FLAG)) {
        return smith_agent_shell::relay::main();
    }
    let arguments = skein_shell::arguments();
    if arguments.as_ref() == [Box::from(b"--help".as_slice())] {
        print!("{}", temper::commands::help());
        return ExitCode::SUCCESS;
    }
    let working = match env::current_dir() {
        Ok(path) => path,
        Err(why) => {
            eprintln!("temper: {why}");
            return ExitCode::FAILURE;
        }
    };
    match temper::configure_with(&arguments, &skein_shell::environment(), working.as_os_str().as_bytes())
        .and_then(temper::run)
    {
        Ok(code) => ExitCode::from(code),
        Err(why) => {
            eprint!("{why}");
            ExitCode::FAILURE
        }
    }
}
