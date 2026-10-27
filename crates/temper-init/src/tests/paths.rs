use super::fixtures::{self, DIRECTORIES, content, parse};
use crate::{Directories, Fact, Facts, Reason, Refusal, files};
use alloc::boxed::Box;
use serde_json::json;
use skein_lib::bytes;

fn absolute(length: usize) -> Box<[u8]> {
    let mut path = bytes::zeroed(length);
    path.fill(b'x');
    *path.first_mut().expect("a nonempty test path") = b'/';
    path
}

fn assign<'a>(facts: &mut Facts<'a>, directories: &mut Directories<'a>, fact: Fact, path: &'a [u8]) {
    match fact {
        Fact::Binary => facts.binary = path,
        Fact::Git => facts.git = Some(path),
        Fact::Claude => facts.claude = Some(path),
        Fact::Config => directories.config = path,
        Fact::Data => directories.data = path,
    }
}

#[test]
fn a_fact_is_refused_naming_it() {
    let long = absolute(4097);
    let mut nontext_long = long.clone();
    *nontext_long.last_mut().expect("the path has 4097 bytes") = 0xff;
    let facts_in_order = [Fact::Binary, Fact::Git, Fact::Claude, Fact::Config, Fact::Data];
    for fact in facts_in_order {
        for (row, reason, path) in [
            ("relative", Reason::Relative, b"relative".as_slice()),
            ("empty", Reason::Relative, b"".as_slice()),
            ("text", Reason::Text, b"/\xff".as_slice()),
            ("bound", Reason::Bound, long.as_ref()),
            ("relative before text", Reason::Relative, b"\xff".as_slice()),
            ("text before bound", Reason::Text, nontext_long.as_ref()),
        ] {
            let mut facts = fixtures::facts();
            let mut directories = DIRECTORIES;
            assign(&mut facts, &mut directories, fact, path);
            assert_eq!(
                files(&facts, &directories, &[0; 32], &[0xab; 32]),
                Err(Refusal { fact, reason }),
                "{row}: {fact:?}"
            );
        }
    }
    let config = absolute(4086);
    let data = absolute(4088);
    for (row, fact, path) in [
        ("agent joined past bound", Fact::Config, config.as_ref()),
        ("programs joined past bound even without claude", Fact::Data, data.as_ref()),
    ] {
        let mut facts = fixtures::facts();
        let mut directories = DIRECTORIES;
        assign(&mut facts, &mut directories, fact, path);
        assert_eq!(
            files(&facts, &directories, &[0; 32], &[0xab; 32]),
            Err(Refusal { fact, reason: Reason::Bound }),
            "{row}"
        );
    }
    for (index, first) in facts_in_order.iter().enumerate() {
        for second in facts_in_order.iter().skip(index.checked_add(1).expect("there are six facts")) {
            let mut facts = fixtures::facts();
            let mut directories = DIRECTORIES;
            assign(&mut facts, &mut directories, *first, b"/\xff");
            assign(&mut facts, &mut directories, *second, b"relative");
            assert_eq!(
                files(&facts, &directories, &[0; 32], &[0xab; 32]),
                Err(Refusal { fact: *first, reason: Reason::Text }),
                "{first:?} before {second:?}"
            );
        }
    }
}

#[test]
fn paths_join_once_and_accept_the_bound() {
    for (row, config, data, agent, programs) in [
        (
            "no trailing slash",
            b"/c/temper".as_slice(),
            b"/d/temper".as_slice(),
            "/c/temper/agent.json",
            "/d/temper/programs",
        ),
        (
            "trailing slash",
            b"/c/temper/".as_slice(),
            b"/d/temper/".as_slice(),
            "/c/temper/agent.json",
            "/d/temper/programs",
        ),
        ("root", b"/".as_slice(), b"/".as_slice(), "/agent.json", "/programs"),
    ] {
        let facts = Facts { claude: Some(b"/opt/claude/bin/claude"), ..fixtures::facts() };
        let answered = files(&facts, &Directories { config, data }, &[0; 32], &[0xab; 32]).expect("valid directories");
        let document = parse(content(&answered, b"temper.json"));
        assert_eq!(
            *document.pointer("/workers/0/agent/configuration").expect("the worker has an agent configuration"),
            json!(agent),
            "{row}"
        );
        assert_eq!(
            *document.pointer("/workers/0/agent/arguments/2").expect("the agent has its configuration argument"),
            json!(agent),
            "{row}"
        );
        let document = parse(content(&answered, b"agent.json"));
        assert_eq!(
            *document.pointer("/programs/claude_code/directory").expect("Claude has a programs directory"),
            json!(programs),
            "{row}"
        );
    }
    let long = absolute(4096);
    for fact in [Fact::Binary, Fact::Git, Fact::Claude] {
        let mut facts = fixtures::facts();
        let mut directories = DIRECTORIES;
        assign(&mut facts, &mut directories, fact, &long);
        files(&facts, &directories, &[0; 32], &[0xab; 32]).expect("4096 bytes is within the bound");
    }
    for (row, config_length, data_length, slash) in
        [("joined bound", 4085, 4087, false), ("joined bound, trailing slash", 4086, 4088, true)]
    {
        let mut config = absolute(config_length);
        let mut data = absolute(data_length);
        if slash {
            *config.last_mut().expect("the directory is nonempty") = b'/';
            *data.last_mut().expect("the directory is nonempty") = b'/';
        }
        files(&fixtures::facts(), &Directories { config: &config, data: &data }, &[0; 32], &[0xab; 32]).expect(row);
    }
    // The encoder's worst expansion also fits: six JSON bytes per byte.
    let mut controls = long;
    controls.get_mut(1..).expect("the path has 4096 bytes").fill(1);
    let facts = Facts { binary: &controls, git: Some(&controls), claude: Some(&controls), ..fixtures::facts() };
    let mut config = absolute(4085);
    config.get_mut(1..).expect("the configuration directory has 4085 bytes").fill(1);
    let mut data = absolute(4087);
    data.get_mut(1..).expect("the data directory has 4087 bytes").fill(1);
    let directories = Directories { config: &config, data: &data };
    let answered =
        files(&facts, &directories, &[0; 32], &[0xab; 32]).expect("bounded text can use JSON escapes throughout");
    assert_eq!(
        parse(content(&answered, b"temper.json"))
            .pointer("/workers/0/agent/program")
            .expect("the worker has an agent program")
            .as_str()
            .expect("a JSON path is text")
            .as_bytes(),
        controls.as_ref()
    );
}
