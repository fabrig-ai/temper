use crate::{Command, configure_with, help};
use std::{fs, os::unix::ffi::OsStrExt};
use temper_client_tools::jig::call::Parsed;

fn fenced_lines(text: &str) -> Vec<(usize, &str)> {
    let mut fenced = false;
    let mut lines = Vec::new();
    for (at, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if fenced {
            lines.push((at + 1, line));
        }
    }
    lines
}

// Only whole unquoted shell separator words end a command example.
fn command_words(line: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut quote = None;
    for ch in line.chars() {
        if let Some(delimiter) = quote {
            if ch == delimiter {
                quote = None;
            } else {
                word.push(ch);
            }
        } else if ch == '#' && !started {
            return Ok(words);
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
            quoted = true;
            started = true;
        } else if ch.is_ascii_whitespace() {
            if started {
                if !quoted && is_command_cut(&word) {
                    return Ok(words);
                }
                words.push(substitute_placeholders(&word));
                word.clear();
                started = false;
                quoted = false;
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("unclosed quote");
    }
    if started && (quoted || !is_command_cut(&word)) {
        words.push(substitute_placeholders(&word));
    }
    Ok(words)
}

fn is_command_cut(word: &str) -> bool {
    matches!(word, "|" | "||" | ">" | ">>" | "<" | "&&" | ";") || word.starts_with('#')
}

fn substitute_placeholders(word: &str) -> String {
    let mut result = String::new();
    let mut start = 0;
    while start < word.len() {
        let end =
            word[start..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(word.len(), |at| start + at);
        let run = &word[start..end];
        if !run.is_empty()
            && run.as_bytes()[0].is_ascii_uppercase()
            && run.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            result.push_str(if run == "PROPOSAL" { "t1/1" } else { "1" });
        } else {
            result.push_str(run);
        }
        if end == word.len() {
            break;
        }
        let separator = word[end..].chars().next().expect("separator");
        result.push(separator);
        start = end + separator.len_utf8();
    }
    result
}

#[test]
fn every_command_line_in_the_pages_parses() {
    let mut ctl = 0;
    let mut binary = 0;
    let mut failures = Vec::new();
    let environment = [(Box::from(b"HOME".as_slice()), Box::from(b"/home/test".as_slice()))];
    for page in &temper_docs::PAGES {
        for (at, line) in fenced_lines(page.text) {
            if line.split_ascii_whitespace().next() != Some("temper") {
                continue;
            }
            let words = match command_words(line) {
                Ok(words) => words,
                Err(why) => {
                    failures.push(format!("{}:{at}: {line}: {why}", page.name));
                    continue;
                }
            };
            let arguments = words[1..].iter().map(String::as_bytes).collect::<Vec<_>>();
            let result = if words.get(1).is_some_and(|word| word == "ctl") {
                ctl += 1;
                match crate::ctl::parse(&arguments[1..]) {
                    Ok((_, Parsed::Call { .. } | Parsed::File { .. })) => Ok(()),
                    Err(why) => Err(why.to_string()),
                }
            } else {
                binary += 1;
                let arguments = arguments.into_iter().map(Box::from).collect::<Vec<_>>();
                configure_with(&arguments, &environment, b"/").map(|_| ())
            };
            if let Err(why) = result {
                failures.push(format!("{}:{at}: {line}: {why}", page.name));
            }
        }
    }
    assert!(failures.is_empty(), "{} command lines refused:\n{}", failures.len(), failures.join("\n"));
    assert!(ctl > 0 && binary > 0, "checked {ctl} ctl and {binary} binary lines");
    println!("checked {ctl} ctl and {binary} binary command lines");
}

#[test]
fn page_commands_keep_quoted_arguments_and_replace_only_capital_words() {
    assert_eq!(
        command_words("temper ctl DIR message TASK 'two words' --key KEY # comment").expect("words"),
        ["temper", "ctl", "1", "message", "1", "two words", "--key", "1"]
    );
    assert_eq!(
        command_words("  temper ctl DIR decide PROPOSAL accept \"two words\"").expect("words"),
        ["temper", "ctl", "1", "decide", "t1/1", "accept", "two words"]
    );
    for ending in
        ["# comment", "#don't parse quotes", "| cat", "|| cat", "> file", ">> file", "< file", "&& next", "; next"]
    {
        assert_eq!(command_words(&format!("temper help {ending}")).expect("words"), ["temper", "help"]);
    }
    assert_eq!(substitute_placeholders("connected:FORGE/taskTASK/N/_TASK/A2"), "connected:1/taskTASK/1/_TASK/1");
}

#[test]
fn only_whole_unquoted_separator_words_cut_a_page_command() {
    assert_eq!(
        command_words("temper ctl DIR forge-connect 2 github-app 12345 <slug>.private-key.pem --key github-app")
            .expect("words"),
        [
            "temper",
            "ctl",
            "1",
            "forge-connect",
            "2",
            "github-app",
            "12345",
            "<slug>.private-key.pem",
            "--key",
            "github-app"
        ]
    );
    assert_eq!(
        command_words("temper help '<' \"|\" '#' '&&' '||' '>' '>>' ';'").expect("quoted separators"),
        ["temper", "help", "<", "|", "#", "&&", "||", ">", ">>", ";"]
    );
    assert_eq!(
        command_words("temper help a|b >file <file a;b").expect("argument words"),
        ["temper", "help", "a|b", ">file", "<file", "a;b"]
    );
    assert_eq!(command_words("temper help 'unclosed"), Err("unclosed quote"));
}

fn configured(words: &[&str]) -> help::Request {
    let words = words.iter().map(|word| Box::from(word.as_bytes())).collect::<Vec<_>>();
    let Command::Help { request } = configure_with(&words, &[], b"/unused").expect("help arguments") else {
        panic!("help command")
    };
    request
}

#[test]
fn help_prints_the_index_before_init_with_no_environment_or_configuration() {
    let directory = std::env::temp_dir().join(format!("temper-help-{}", std::process::id()));
    fs::create_dir(&directory).expect("empty working directory");
    assert_eq!(fs::read_dir(&directory).expect("working directory").count(), 0);
    let command = configure_with(&[Box::from(b"help".as_slice())], &[], directory.as_os_str().as_bytes())
        .expect("help before init");
    let Command::Help { request } = command else { panic!("help command") };
    let answer = help::answer(&request).expect("index page");
    assert_eq!(answer, temper_docs::find(b"index").expect("index page").text);
    assert_eq!(fs::read_dir(&directory).expect("still empty working directory").count(), 0);
    fs::remove_dir(directory).expect("remove empty working directory");
}

#[test]
fn help_prints_a_whole_page_and_names_an_unknown_one() {
    assert_eq!(
        help::answer(&configured(&["help", "ctl"])).expect("ctl page").as_bytes(),
        include_bytes!("../../../../docs/help/ctl.md")
    );
    assert_eq!(
        help::answer(&configured(&["help", "nope"])),
        Err("temper: no page nope; temper help lists them\n".into())
    );
}

#[test]
fn help_search_names_the_server_section_or_says_nothing_matches() {
    let text = help::answer(&configured(&["help", "--search", "WireGuard"])).expect("WireGuard section");
    assert!(text.lines().any(|line| line.starts_with("server: 7. The workers' host over WireGuard: ")), "{text}");
    assert!(text.contains("WireGuard"), "{text}");
    assert_eq!(
        help::answer(&configured(&["help", "--search", "zzzznotadocword"])),
        Err("temper: nothing matches\n".into())
    );
}

#[test]
fn help_search_joins_words_and_prints_each_search_refusal() {
    assert_eq!(
        configured(&["help", "--search", "owned", "Forgejo"]),
        help::Request::Search(Box::from(b"owned Forgejo".as_slice()))
    );
    for (words, error) in [
        (" \t", "nothing to search for"),
        ("a a a a a a a a a", "at most 8 words"),
        ("123456789012345678901234567890123", "a word is at most 32 bytes"),
    ] {
        assert_eq!(
            help::answer(&help::Request::Search(Box::from(words.as_bytes()))),
            Err(format!("temper: {error}\n"))
        );
    }
}

#[test]
fn help_requires_one_page_or_search_words() {
    for words in [vec!["help", "--search"], vec!["help", "a", "b"], vec!["help", "--unknown"]] {
        let words = words.iter().map(|word| Box::from(word.as_bytes())).collect::<Vec<_>>();
        assert_eq!(
            configure_with(&words, &[], b"/unused"),
            Err("temper help: invalid arguments\nusage: temper help [NAME | --search WORDS...]\n".into())
        );
    }
}
