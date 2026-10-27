use super::fixtures as f;
use jig_client::{self as client, contract as c};
use skein_lib::List;
use temper_client_protocol_wire::{self as wire, v1 as w};

#[test]
fn repositories_parses_bounds_defaults_and_the_requested_installation_and_page() {
    for words in
        [&["forge", "repositories", "3"][..], &["forge", "repositories", "3", "--installation", "11", "--page", "2"]]
    {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(words);
        let client::Ask::Family { scope: c::Scope::Deployment, record } = &root.sends[&call].2 else {
            panic!("deployment request")
        };
        assert_eq!((record.family, record.version), (wire::FORGE, wire::CLIENT_VERSION));
        let bounds = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).unwrap();
        let request = wire::decode_requesting(&record.bytes, &bounds).unwrap();
        let w::Request::Reachable(request) = request.into_parts().request else { panic!("reachable") };
        assert_eq!(request.forge(), 3);
        assert_eq!(*request.installation(), if words.len() == 3 { None } else { Some(11) });
        assert_eq!(request.page(), if words.len() == 3 { 1 } else { 2 });
    }
    for words in [
        &["forge", "repositories", "0"][..],
        &["forge", "repositories", "65536"],
        &["forge", "repositories", "3", "--page", "0"],
        &["forge", "repositories", "3", "--page", "4294967296"],
        &["forge", "repositories", "3", "--installation", "0"],
    ] {
        f::parse(words).expect_err("bound");
    }
    f::parse(&["forge", "repositories", "65535", "--page", "4294967295", "--installation", "18446744073709551615"])
        .unwrap();
}
fn page(more: bool) -> w::Outcome {
    let bounds = temper_client_domain::forge::wire_limits(&f::limits_root().domain.forge).unwrap();
    let mut rows = List::with_capacity(2);
    for (id, name, private) in [(42, b"app".as_slice(), true), (43, b"public".as_slice(), false)] {
        rows.push(
            w::Reached::new(
                &bounds,
                w::ReachedParts {
                    id,
                    owner: w::Account::new(&bounds, w::AccountParts { id: 10, login: b"acme".as_slice().into() })
                        .unwrap(),
                    name: name.into(),
                    private,
                },
            )
            .unwrap(),
        )
        .unwrap();
    }
    w::Outcome::Page(w::Page::new(&bounds, w::PageParts { rows, more }).unwrap())
}
#[test]
fn repositories_prints_rows_ids_privacy_and_the_next_call_or_no_more() {
    for more in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["forge", "repositories", "3", "--installation", "11", "--page", "2"]);
        root.adoption_answer(call, page(more));
        let printed = root.answer(call).unwrap();
        assert_eq!(printed.status, 0);
        assert_eq!(
            printed.stdout.as_ref(),
            if more {
                b"acme/app private 42/10\nacme/public 43/10\ntemper ctl DIR forge repositories 3 --installation 11 --page 3\n".as_slice()
            } else {
                b"acme/app private 42/10\nacme/public 43/10\nNo more repositories follow.\n".as_slice()
            }
        );
    }
}
#[test]
fn repositories_json_keeps_the_page_shape() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit_prepared(f::call(&["forge", "repositories", "3"]), true, None);
    root.adoption_answer(call, page(true));
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 0);
    assert_eq!(printed.stdout.as_ref(), br#"{"rows":[{"id":42,"owner":{"id":10,"login":"acme"},"name":"app","private":true},{"id":43,"owner":{"id":10,"login":"acme"},"name":"public","private":false}],"more":true}"#);
}
