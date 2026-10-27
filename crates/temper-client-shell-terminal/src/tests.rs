use crate::{
    limits,
    route::{self, Down},
};
use jig_client_protocol_link as link;
use skein_lib::{List, Queue, Time};
use temper_client_domain as client;
mod family;
pub(crate) mod fixtures;
mod render;

#[test]
fn a_jig_read_reaches_the_link_with_its_own_token() {
    let down = route::down(client::Request::Client {
        request: jig_client::Request::Down {
            down: jig_client::Down::Read {
                read: fixtures::READ,
                what: jig_client::Readable::Jig(fixtures::inbox_read()),
            },
        },
    });
    let Down::Shell(jig_client::Request::Down {
        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(what) },
    }) = down
    else {
        panic!("jig read reaches its link");
    };
    assert_eq!(read, fixtures::READ);
    assert_eq!(what, fixtures::inbox_read());
}
#[test]
fn a_table_record_saved_then_kept_comes_back_the_same() {
    let limits = fixtures::link_limits();
    {
        let (name, record) = ("table", fixtures::table_record());
        let mut link = link::Link::new(&limits).expect("bounded store codec");
        let mut out = Queue::with_capacity(link.max_out());
        link.step(
            Time::ZERO,
            link::Event::Save { record: link::Saved { version: 2, record: record.clone() } },
            &mut out,
        );
        let Some(link::Request::Save { record: link::Record { key, bytes } }) = out.pop() else {
            panic!("{name}: save writes");
        };
        let mut kept = List::with_capacity(1);
        kept.push(link::Record { key, bytes }).expect("one record fits");
        let mut restarted = link::Link::new(&limits).expect("bounded restore");
        restarted.step(Time::ZERO, link::Event::Start { kept }, &mut out);
        let Some(link::Request::Kept { record: saved }) = out.pop() else {
            panic!("{name}: record restored");
        };
        assert_eq!(Some(saved.record), Some(record), "{name}: unchanged record");
        assert!(out.is_empty(), "{name}: only one restored record");
    }
}
#[test]
fn the_native_limits_pass_each_worst_case() {
    let limits = limits::native();
    assert_eq!(limits.client.client.domain.authority, 14);
    let (view, forge) = limits::view(&limits).expect("usable view");
    let root = client::worst_case(&limits.client).expect("usable root");
    let view = temper_client_view::worst_case(&view, &limits.client, &forge).expect("usable view");
    let shell = jig_client_shell_native::Shell::new(jig_client_shell_native::Settings {
        engine: skein_io::Addr::Ip("127.0.0.1:7100".parse().expect("loopback")),
        directory: Box::from(*b"client"),
        limits: limits.connections,
        io: limits.io,
    });
    assert_eq!(limits::shell(&limits), Some(shell.worst_case()), "checked shell formula");
    assert_eq!(limits::worst_case(&limits), Some(root + view + shell.worst_case()), "sum of all owners");
    assert!(skein_io::worst_case(&limits.io).is_some(), "usable io");
    let wire = client::forge::wire_limits(&limits.client.forge).expect("client wire bounds");
    let demo = temper_client_protocol_wire::v1::DemoAnswer::worst_case_bytes(&wire).expect("demo bound");
    assert!(demo <= limits.client.client.domain.document_bytes, "family read answer fits");
    assert!(demo <= u64::from(limits.connections.body_bytes), "whole HTTP answer fits");
    assert!(limits.client.client.domain.text >= temper_client_protocol_wire::CUE_BYTES, "whole cue sends");
}

#[test]
fn an_equal_line_waits_once_in_the_order_it_came() {
    let mut lines = crate::terminal::waiting::Waiting::new(19);
    for line in [b"sign-in again".as_slice(), b"store failed", b"sign-in again", b"credential one", b"credential two"] {
        lines.push(Box::from(line));
    }
    let mut said = Vec::<Box<[u8]>>::new();
    lines.say(1, |line| said.push(Box::from(line)));
    assert_eq!(said, [Box::<[u8]>::from(b"sign-in again".as_slice())]);
    said.clear();
    lines.say(8, |line| said.push(Box::from(line)));
    assert_eq!(said, [b"store failed".as_slice(), b"credential one", b"credential two"].map(Box::<[u8]>::from));
    assert!(lines.is_empty());
}

#[test]
fn the_native_limits_make_the_terminal() {
    let (view, _) = limits::view(&limits::native()).expect("native view limits");
    let bounds = limits::terminal();
    let settings = skein_ui_text::Settings {
        name: Box::from(*b"temper"),
        longest: bounds.line,
        interval: bounds.interval,
        status: Some(temper_client_view::frame::link_region()),
        lines: bounds.lines,
        line_bytes: 166,
    };
    assert!(skein_ui_text::Terminal::worst_case(&settings, &view.tree).is_some(), "native terminal memory fits");
    assert!(skein_ui_text::Terminal::operations(&settings, &view.tree).is_some(), "native terminal operations fit");
    assert!(skein_ui_text::Terminal::new(settings, &view.tree).is_some(), "native terminal settings fit");
}

#[test]
fn a_kept_credential_signs_in_held_with_no_exchange() {
    let mut link = link::Link::new(&fixtures::link_limits()).expect("bounded link");
    let mut out = Queue::with_capacity(link.max_out());
    link.step(Time::ZERO, link::Event::SignIn { with: link::With::Held(fixtures::credential()) }, &mut out);
    assert_eq!(link.credential(), Some(&fixtures::credential()));
    assert!(out.is_empty(), "a held credential sends no sign-in exchange");
}
