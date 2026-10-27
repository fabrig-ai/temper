//! The executable's one-shot tools entry point (client/tools.md, 8.1).
use skein_io::kernel::{Exit, Fd};
use skein_shell::{Clock, Config, Host, Kernel};
use std::{ffi::OsString, os::unix::ffi::OsStrExt, path::Path};
use temper_client_shell_cli::{
    arguments,
    host::{Invocation, OneShot, Startup},
};

pub(crate) fn parse<'a>(
    words: &[&'a [u8]],
) -> Result<(Option<&'a [u8]>, temper_client_tools::jig::call::Parsed), String> {
    let (directory, words) = match words.split_first() {
        Some((word, _)) if *word == b"help" => (None, words),
        Some((directory, words)) => (Some(*directory), words),
        None => return Err(String::from_utf8_lossy(temper_client_tools::table::USAGE).into_owned()),
    };
    let parsed = arguments::parse(words).map_err(|why| {
        let size = words.iter().map(|word| word.len()).sum::<usize>() + 256;
        let mut out = skein_lib::Writer::new(size);
        temper_client_tools::table::refusal(why, words, &mut out).expect("usage fits");
        let mut exact = skein_lib::Writer::new(out.written());
        temper_client_tools::table::refusal(why, words, &mut exact).expect("measured usage");
        String::from_utf8_lossy(&exact.finish()).into_owned()
    })?;
    Ok((directory, parsed))
}

fn start(arguments: &[OsString], clock: &Clock) -> Result<u8, String> {
    let words: Vec<_> = arguments.iter().map(|word| word.as_bytes()).collect();
    let (directory, parsed) = parse(&words)?;
    let help = matches!(&parsed, temper_client_tools::jig::call::Parsed::Call { call, .. } if arguments::class(call) == arguments::Class::Help);
    let mut owned = Vec::new();
    let result = (|| {
        let (parent, directory) = if let Some(directory) = directory.filter(|_| !help) {
            let path = super::path(directory);
            let leaf = path.file_name().ok_or("DIRECTORY requires a last component")?;
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            let root = skein_shell::open_root(parent).map_err(|errno| format!("DIRECTORY parent: errno {errno}"))?;
            owned.push(root);
            (Some(root), Some(leaf.as_bytes().into()))
        } else {
            (None, None)
        };
        let file_parent = arguments::file_root(&parsed)
            .map(|directory| {
                let root = skein_shell::open_root(super::path(&directory))
                    .map_err(|errno| format!("FILE parent: errno {errno}"))?;
                owned.push(root);
                Ok::<_, String>(root)
            })
            .transpose()?;
        let mut text_files = Vec::new();
        for (index, directory) in arguments::text_files(&parsed) {
            let root = skein_shell::open_root(super::path(&directory))
                .map_err(|errno| format!("FILE parent: errno {errno}"))?;
            owned.push(root);
            text_files.push((index, root));
        }
        let seed = skein_shell::seed().map_err(|errno| format!("random seed: errno {errno}"))?;
        let signals = skein_shell::open_termination_signals().map_err(|errno| format!("signals: errno {errno}"))?;
        owned.push(signals);
        let mut host = OneShot::with_text_files(
            Invocation { directory, parsed },
            Startup {
                parent,
                file_parent,
                stdin: Fd::new(0),
                stdout: Fd::new(1),
                stderr: Fd::new(2),
                signals,
                user: skein_shell::effective_user(),
                seed,
            },
            text_files,
        )
        .map_err(|_| "client limits refused")?;
        let mut kernel = Kernel::open(Config { operations: host.operations() }).map_err(|why| why.to_string())?;
        owned.clear();
        Ok(match skein_shell::drive(&mut kernel, clock, &mut host) {
            Exit::Code(code) => code,
            Exit::Signal(_) => 1,
        })
    })();
    super::close(owned);
    result
}
#[must_use]
pub fn run(arguments: &[OsString], clock: &Clock) -> u8 {
    match start(arguments, clock) {
        Ok(code) => code,
        Err(why) => {
            eprintln!("temper ctl: {why}");
            1
        }
    }
}
