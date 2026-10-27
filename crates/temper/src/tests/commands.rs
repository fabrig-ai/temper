use crate::{
    Command,
    commands::{self, COMMANDS, Who},
    configure_with, fake,
};

fn bytes(words: &[&str]) -> Vec<Box<[u8]>> {
    words.iter().map(|word| Box::from(word.as_bytes())).collect()
}
fn configured(words: &[&str]) -> Result<Command, String> {
    configure_with(&bytes(words), &[(Box::from(b"HOME".as_slice()), Box::from(b"/home/test".as_slice()))], b"/")
}
fn command_words(command: &Command) -> Vec<&str> {
    match command {
        Command::Usage(row) => row.words.to_vec(),
        Command::Init { .. } => vec!["init"],
        Command::Serve { boxed: false, .. } => vec!["serve"],
        Command::Serve { boxed: true, .. } => vec!["internal", "serve"],
        Command::Box { .. } | Command::BoxFakeBackends { .. } => vec!["box"],
        Command::ProgramLogin { .. } => vec!["program-login"],
        Command::Agent { .. } => vec!["internal", "agent"],
        Command::Open { .. } => vec!["open"],
        Command::Ctl { .. } => vec!["ctl"],
        Command::Help { .. } => vec!["help"],
        Command::Fake { arguments } => vec![
            "fake",
            match arguments.kind {
                fake::Kind::Llm => "llm",
                fake::Kind::Forge => "forge",
                fake::Kind::Oauth => "oauth",
            },
        ],
        Command::FakeCheck { .. } => vec!["fake", "forge", "check"],
        Command::FakeGit { .. } => vec!["fake", "git"],
        Command::ForgeChild { .. } => vec!["internal", "forge-child"],
        Command::Forge { arguments, .. } => vec!["forge", arguments[0].to_str().expect("forge verb")],
    }
}

#[test]
fn every_row_parses_from_an_example() {
    let examples: &[(&[&str], &[&str])] = &[
        (&["help"], &["help"]),
        (&["init"], &["init"]),
        (&["serve"], &["serve"]),
        (&["box"], &["box", "--echo", "steps"]),
        (&["box"], &["box", "--fake-backends", "--bundle", "/bundle"]),
        (&["program-login"], &["program-login", "source"]),
        (&["forge", "fetch"], &["forge", "fetch", "1"]),
        (&["forge", "prepare"], &["forge", "prepare", "1"]),
        (&["forge", "run"], &["forge", "run", "1", "forgejo"]),
        (&["forge", "sign-in"], &["forge", "sign-in", "1"]),
        (&["forge", "reconcile"], &["forge", "reconcile", "1"]),
        (&["ctl"], &["ctl", "DIR", "task", "show", "1"]),
        (&["ctl"], &["ctl", "DIR", "me"]),
        (&["ctl"], &["ctl", "DIR", "--call", "FILE"]),
        (&["ctl"], &["ctl", "help", "task", "show"]),
        (&["open"], &["open", "/client", "http://127.0.0.1:7000"]),
        (&["internal", "agent"], &["internal", "agent", "agent.json"]),
        (&["internal", "serve"], &["internal", "serve", "--box"]),
        (&["internal", "forge-child"], &["internal", "forge-child", "--parent", "1", "--signal", "kill", "--", "p"]),
        (&["fake", "llm"], &["fake", "llm"]),
        (&["fake", "llm"], &["fake", "llm", "--document", "llm.json"]),
        (&["fake", "forge"], &["fake", "forge", "1"]),
        (&["fake", "forge"], &["fake", "forge", "--document", "forge.json"]),
        (&["fake", "forge", "check"], &["fake", "forge", "check", "1", "observations", "store"]),
        (&["fake", "oauth"], &["fake", "oauth", "--document", "oauth.json"]),
        (&["fake", "git"], &["fake", "git", "status"]),
    ];
    for &(words, example) in examples {
        let row = commands::find(&bytes(example)).expect("example names a row");
        assert_eq!(row.words, words, "{example:?}: longest command words");
        let command = configured(example).expect("example parses");
        assert_eq!(command_words(&command), words, "{example:?}: dispatched command");
    }
    for row in &COMMANDS {
        assert_eq!(
            examples.iter().filter(|(words, _)| *words == row.words).count(),
            row.forms.len(),
            "{:?}: an example per form",
            row.words
        );
    }
    assert!(matches!(configured(&["box", "--fake-backends"]).expect("fake box"), Command::BoxFakeBackends { .. }));
}

#[test]
fn each_row_answers_its_help() {
    for row in &COMMANDS {
        let mut words = row.words.to_vec();
        words.push("--help");
        assert_eq!(configure_with(&bytes(&words), &[], b"/unused"), Ok(Command::Usage(row)));
        assert!(commands::row(row).starts_with(&format!("usage: temper {} {}\n", row.words.join(" "), row.forms[0])));
    }
    let row = commands::find(&bytes(&["forge", "run"])).expect("forge run row");
    assert_eq!(commands::row(row), "\
usage: temper forge run [DIRECTORIES] FORGE forgejo|runner
Run an owned forge's Forgejo or its runner.
  DIRECTORIES: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
  FORGE: A forge's number in the configuration.
Checks the binary and the preparation, and takes the forge's lock, first. Run alone, it replaces itself by the program, whose exit status is then its own; given the forge's listener, as the box gives it, it runs the program as its child.
  --config CONFIG (TEMPER_CONFIG; default $XDG_CONFIG_HOME/temper)
      Configuration, including temper.json.
  --data DATA (TEMPER_DATA; default $XDG_DATA_HOME/temper)
      The deployment's data and secrets.
  --cache CACHE (TEMPER_CACHE; default $XDG_CACHE_HOME/temper)
      Binaries that can be fetched again.
");
}

#[test]
fn temper_help_lists_rows_by_who() {
    let text = commands::help();
    assert!(text.starts_with("usage: temper COMMAND [ARGUMENT...]\ntemper help prints the index of temper's pages; temper COMMAND --help prints one command.\n\ndocumentation:\n"));
    let groups = [Who::Documentation, Who::Deployment, Who::Clients, Who::Spawned, Who::TestDoubles];
    let mut previous_end = 0;
    for who in groups {
        let heading = if who == Who::Spawned { "spawned, never run by hand" } else { who.word() };
        let heading = format!("\n{heading}:\n");
        assert_eq!(text.matches(&heading).count(), 1);
        let start = text.find(&heading).expect("group heading");
        assert!(start >= previous_end, "groups in Who order");
        let end =
            text[start + heading.len()..].find("\n\n").map_or(text.len(), |offset| start + heading.len() + offset);
        let group = &text[start..end];
        for row in COMMANDS.iter().filter(|row| row.who == who) {
            for form in row.forms {
                let form = format!("  temper {} {form}\n", row.words.join(" "));
                assert_eq!(text.matches(&form).count(), 1, "each form once");
                assert!(group.contains(form.trim_end()), "form under its group");
            }
            if who == Who::Spawned {
                assert!(!text.contains(row.line), "spawned rows list forms alone");
            } else {
                let transition = if row.transition { " (a transition)" } else { "" };
                assert!(group.contains(&format!("    {}{transition}", row.line)));
            }
        }
        previous_end = end;
    }
}

#[test]
fn spawned_commands_answer_only_under_internal() {
    for (words, error) in [
        (vec!["agent", "X"], "temper: no command agent; temper --help lists them\n"),
        (vec!["serve", "--box"], "temper serve: invalid arguments\nusage: temper serve [DIRECTORIES]\n"),
        (
            vec!["forge", "child", "--parent", "1", "--signal", "kill", "--", "p"],
            "temper: no command forge child; temper --help lists them\n",
        ),
    ] {
        assert_eq!(configured(&words), Err(error.into()));
    }
    assert_eq!(configured(&["internal", "agent", "X"]), Ok(Command::Agent { configuration: "X".into() }));
    let Command::Serve { directories, boxed } = configured(&["internal", "serve", "--box"]).expect("box step") else {
        panic!("serve")
    };
    assert!(boxed);
    assert_eq!(directories.config.as_ref(), b"/home/test/.config/temper");
    assert_eq!(
        configured(&["internal", "forge-child", "--parent", "1", "--signal", "kill", "--", "p"]),
        Ok(Command::ForgeChild {
            parent: 1,
            signal: skein_io::kernel::Signal::Kill,
            program: "p".into(),
            arguments: vec![],
        })
    );
    assert!(configured(&["internal", "serve"]).is_err(), "internal serve requires --box");
}

#[test]
fn refusals_name_the_command_and_its_forms() {
    for (words, error) in [
        (vec![], "temper: no command; temper --help lists them\n"),
        (vec!["bogus"], "temper: no command bogus; temper --help lists them\n"),
        (vec!["forge", "bogus", "1"], "temper: no command forge bogus; temper --help lists them\n"),
        (
            vec!["forge", "run", "1"],
            "temper forge run: invalid arguments\nusage: temper forge run [DIRECTORIES] FORGE forgejo|runner\n",
        ),
    ] {
        assert_eq!(configured(&words), Err(error.into()));
    }
}

#[test]
fn rows_are_within_bounds() {
    for (index, row) in COMMANDS.iter().enumerate() {
        assert!(row.line.len() <= 120, "{:?}: line", row.words);
        assert!(row.notes.len() <= 1024, "{:?}: notes", row.words);
        for argument in row.arguments {
            assert!(argument.line.len() <= 200, "{:?}: {}", row.words, argument.shown);
        }
        for other in &COMMANDS[index + 1..] {
            assert_ne!(row.words, other.words, "unique command words");
        }
    }
}

#[test]
fn the_page_commands_is_what_the_table_renders() {
    let rendered = commands::reference();
    assert_eq!(
        include_str!("../../../../docs/help/reference/commands.md"),
        rendered,
        "the page should be:\n{rendered}"
    );
}

#[test]
fn every_ctl_group_page_is_what_the_table_renders() {
    use temper_client_tools::{help, table::GROUPS};
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/help/reference");
    let expected = GROUPS
        .iter()
        .map(|group| format!("ctl-{}", core::str::from_utf8(group.name).expect("group name")))
        .collect::<Vec<_>>();
    let mut files = std::fs::read_dir(&directory)
        .expect("reference directory")
        .map(|entry| entry.expect("reference file").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| path.file_stem().expect("page name").to_str().expect("ASCII name").to_owned())
        .filter(|name| name.starts_with("ctl-"))
        .collect::<Vec<_>>();
    files.sort();
    let mut sorted = expected.clone();
    sorted.sort();
    assert_eq!(files, sorted, "the group pages and GROUPS are the same set");
    let compiled = temper_docs::PAGES
        .iter()
        .filter(|page| page.name.starts_with("ctl-"))
        .map(|page| page.name)
        .collect::<Vec<_>>();
    assert_eq!(compiled, expected, "compiled group pages follow GROUPS' order");
    let commands = temper_docs::PAGES.iter().position(|page| page.name == "commands").expect("commands page");
    for (index, (group, name)) in GROUPS.iter().zip(&expected).enumerate() {
        let page = &temper_docs::PAGES[commands + 1 + index];
        assert_eq!(page.name, name, "group pages immediately follow commands");
        assert_eq!(page.kind, temper_docs::Kind::Reference, "{name}");
        let mut measured =
            skein_lib::Writer::new(usize::try_from(temper_client_tools::limits::TEXT).expect("text bound"));
        help::reference(group, &mut measured).expect("reference within renderer's text bound");
        let mut out = skein_lib::Writer::new(measured.written());
        help::reference(group, &mut out).expect("reference within renderer's text bound");
        let rendered = String::from_utf8(out.finish().into_vec()).expect("reference UTF-8");
        assert!(rendered.len() <= temper_docs::PAGE_BYTES, "{name}: {} bytes", rendered.len());
        assert_eq!(
            std::fs::read_to_string(directory.join(name).with_extension("md")).expect("group page"),
            rendered,
            "{name} should be:\n{rendered}"
        );
        assert_eq!(page.text, rendered, "compiled {name} should be:\n{rendered}");
    }
}
