//! Browser sign-in ways supplied by the page (protocol/sign-in.md, 7).
use crate::forge_sign_in::Forges;
use crate::words::{Shell, Word, Words};
use alloc::boxed::Box;
use skein_lib::Writer;
use skein_ui::{Builder, Role, Run, phrases::Hole};

/// The longest page origin a sign-in link is written from.
pub const ORIGIN_BYTES: usize = 260;
/// Known ways only, and the page's origin their links leave from; no other page-owned text is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ways {
    pub google: bool,
    /// The page's `http` or `https` origin, as `location.origin` gives it.
    pub origin: Option<Box<[u8]>>,
}
impl Ways {
    pub const NONE: Self = Self { google: false, origin: None };
    /// The meta's ways, offered only from an `http` or `https` origin within `ORIGIN_BYTES`.
    #[must_use]
    pub fn parse(content: &str, origin: &[u8]) -> Self {
        let mut google = false;
        for way in content.split_ascii_whitespace() {
            if way == "google" {
                google = true;
            }
        }
        let web = origin.starts_with(b"https://") || origin.starts_with(b"http://");
        if !web || origin.len() > ORIGIN_BYTES {
            return Self::NONE;
        }
        Self { google, origin: Some(Box::from(origin)) }
    }
}
pub(crate) fn offered(root: &temper_client_domain::Domain, words: &Words, ways: &Ways) -> bool {
    words.shell == Shell::Browser
        && ways.google
        && ways.origin.is_some()
        && root.client().link().state == jig_client::Link::SignedOut
        && root.client().link().not_signed_in.is_none()
}
/// Unreserved bytes and `/` stay as they are in the return path.
fn kept(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte)
}
fn google(origin: &[u8], path: &[u8]) -> Box<[u8]> {
    const PREFIX: &[u8] = b"/sign-in/google?then=";
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut size = origin.len().checked_add(PREFIX.len()).expect("bounded URL");
    for byte in path {
        size = size.checked_add(if kept(*byte) { 1 } else { 3 }).expect("bounded URL");
    }
    let mut out = Writer::new(size);
    out.put(origin).expect("sized URL");
    out.put(PREFIX).expect("sized URL");
    for byte in path {
        if kept(*byte) {
            out.put(&[*byte]).expect("sized URL");
        } else {
            out.put(&[
                b'%',
                *HEX.get(usize::from(byte >> 4_u8)).expect("high nibble"),
                *HEX.get(usize::from(byte & 15_u8)).expect("low nibble"),
            ])
            .expect("sized URL");
        }
    }
    out.finish()
}
pub(crate) fn main(
    b: &mut Builder,
    root: &temper_client_domain::Domain,
    words: &Words,
    origin: &[u8],
    forges: &Forges,
) {
    front(b, root, words, Some(origin), forges, None);
}
fn front(
    b: &mut Builder,
    root: &temper_client_domain::Domain,
    words: &Words,
    origin: Option<&[u8]>,
    forges: &Forges,
    query: Option<&[u8]>,
) {
    b.open(Role::Main);
    b.style(crate::look::MAIN);
    b.style(crate::look::SIGN_IN);
    b.open(Role::Slot);
    b.style(crate::look::CARD);
    b.open(Role::Paragraph);
    b.style(crate::look::BRAND);
    words.text(b, Word::Brand, &[]);
    b.close();
    b.open(Role::Heading { level: 1 });
    let heading = words.say(Word::SignInTitle, &[]);
    b.name(&heading);
    words.text(b, Word::SignInTitle, &[]);
    b.close();
    if let Some(query) = query {
        b.open(Role::Paragraph);
        words.text(b, Word::SignInContinue, &[]);
        let forge = forge_name(forges, query, words);
        b.text(Run::Strong, &forge);
        b.close();
    }
    if root.signed_out() {
        for forge in forges.entries() {
            b.open(Role::Paragraph);
            words.text(b, Word::SignInSignedOut, &[Hole::Name(forge.name(words))]);
            b.close();
        }
    }
    if let Some(origin) = origin {
        let path = match query {
            Some(query) => {
                let prefix = b"/oauth/authorize?";
                let mut out = Writer::new(prefix.len().checked_add(query.len()).expect("bounded authorization"));
                out.put(prefix).expect("sized path");
                out.put(query).expect("sized path");
                out.finish()
            }
            None => temper_client_domain::address::print(root.pages().address()),
        };
        b.open(Role::Link);
        b.name(&words.say(Word::ContinueGoogle, &[]));
        b.external(&google(origin, &path));
        words.text(b, Word::ContinueGoogle, &[]);
        b.close();
    } else {
        b.open(Role::Paragraph);
        words.text(b, Word::SignedOutMain, &[]);
        b.close();
    }
    b.close();
    b.close();
}

fn forge_name(forges: &Forges, query: &[u8], words: &Words) -> Box<[u8]> {
    match forges.client(query) {
        Some(forge) => forge.name(words),
        None => words.say(Word::ForgeOther, &[]),
    }
}
pub(crate) fn authorize(
    b: &mut Builder,
    root: &temper_client_domain::Domain,
    state: temper_client_domain::authorize::State,
    query: &[u8],
    words: &Words,
    ways: &Ways,
    forges: &Forges,
) {
    use jig_client::AuthorizeWhy;
    use temper_client_domain::authorize::State;
    if state == State::SignedOut {
        front(b, root, words, if ways.google { ways.origin.as_deref() } else { None }, forges, Some(query));
        return;
    }
    b.open(Role::Main);
    b.style(crate::look::MAIN);
    b.style(crate::look::SIGN_IN);
    b.open(Role::Slot);
    b.style(crate::look::CARD);
    let title = match state {
        State::Waiting | State::Pending | State::Continuing => Word::AuthorizeTitle,
        State::Refused(_) => Word::AuthorizeRefused,
        State::Busy => Word::AuthorizeBusy,
        State::SignedOut => unreachable!("signed-out front drawn above"),
    };
    b.open(Role::Heading { level: 1 });
    b.name(&words.say(title, &[]));
    words.text(b, title, &[]);
    b.close();
    match state {
        State::Waiting | State::Pending | State::Continuing => {
            if let Some(me) = root.pages().me(root.client()) {
                b.open(Role::Paragraph);
                words.text(
                    b,
                    Word::AuthorizeContinuing,
                    &[
                        Hole::Name(me.party.name.bytes().into()),
                        Hole::Name(forge_name(forges, query, words)),
                        Hole::Name(me.party.login.bytes().into()),
                    ],
                );
                b.close();
            }
        }
        State::Refused(AuthorizeWhy::UnknownClient | AuthorizeWhy::UnknownRedirect) => {
            b.open(Role::Paragraph);
            words.text(b, Word::AuthorizeUnknown, &[]);
            b.close();
            if let Some(origin) = &ways.origin {
                let mut url = Writer::new(origin.len().checked_add(1).expect("origin bound"));
                url.put(origin).expect("sized URL");
                url.put(b"/").expect("sized URL");
                b.open(Role::Link);
                b.name(&words.say(Word::AuthorizeBack, &[]));
                b.external(&url.finish());
                words.text(b, Word::AuthorizeBack, &[]);
                b.close();
            }
        }
        State::Busy => {
            b.open(Role::Paragraph);
            words.text(b, Word::AuthorizeBusyBody, &[]);
            b.close();
        }
        State::Refused(AuthorizeWhy::Malformed | AuthorizeWhy::NoProvider | AuthorizeWhy::NotAPerson)
        | State::SignedOut => {}
    }
    b.close();
    b.close();
}

#[cfg(test)]
mod tests {
    #[test]
    fn return_paths_keep_only_unreserved_bytes_and_slashes() {
        for (path, expected) in [
            (b"/a-b._~/".as_slice(), b"https://t.example/sign-in/google?then=/a-b._~/".as_slice()),
            (
                b"/a b/%?q=\xc3\xa9".as_slice(),
                b"https://t.example/sign-in/google?then=/a%20b/%25%3Fq%3D%C3%A9".as_slice(),
            ),
        ] {
            assert_eq!(super::google(b"https://t.example", path).as_ref(), expected);
        }
    }
}

#[cfg(test)]
mod authorize_tests {
    #![allow(clippy::disallowed_methods, reason = "tests inspect semantic links and construct return paths")]
    use super::*;
    use crate::tests::fixtures;
    use jig_client::AuthorizeWhy;
    use temper_client_domain::authorize::State;
    fn render(state: State, query: &[u8]) -> skein_ui::View {
        let root = fixtures::root_in_project(None);
        let words = fixtures::browser_words();
        let mut limits = fixtures::tree_limits();
        limits.text_per_node = crate::Limits::of(&fixtures::limits(), &words).unwrap().tree.text_per_node;
        let mut view = skein_ui::View::new(&limits);
        let mut builder = view.builder();
        let ways = Ways::parse("google", b"https://temper.example");
        let mut forges = Forges::new();
        forges.read("1 forgejo https://forge.example:3000 other");
        builder.open(Role::Page);
        authorize(&mut builder, &root, state, query, &words, &ways, &forges);
        builder.close();
        view.finish(builder, 1, &mut skein_lib::Queue::with_capacity(960));
        view
    }
    #[test]
    fn each_authorization_page_uses_the_mockup_words() {
        for state in [State::Waiting, State::Pending, State::Continuing] {
            let tree = render(state, b"client_id=forgejo");
            assert!(
                fixtures::find(tree.tree(), Role::Heading { level: 1 }, "Continuing to Forgejo…".as_bytes()).is_some()
            );
            let text = fixtures::main_text(tree.tree());
            assert!(
                core::str::from_utf8(&text)
                    .unwrap()
                    .contains("Forgejo at forge.example:3000 opens in a moment, signed in as")
            );
        }
        for why in [AuthorizeWhy::UnknownClient, AuthorizeWhy::UnknownRedirect] {
            let tree = render(State::Refused(why), b"client_id=unknown");
            assert!(
                fixtures::find(tree.tree(), Role::Heading { level: 1 }, "This sign-in can’t continue".as_bytes())
                    .is_some()
            );
            assert!(fixtures::find(tree.tree(), Role::Link, b"Back to sign in").is_some());
            assert!(
                core::str::from_utf8(&fixtures::main_text(tree.tree())).unwrap().contains("Nothing was sent anywhere.")
            );
        }
        for why in [AuthorizeWhy::Malformed, AuthorizeWhy::NoProvider, AuthorizeWhy::NotAPerson] {
            let tree = render(State::Refused(why), b"client_id=forgejo");
            assert_eq!(fixtures::main_text(tree.tree()).as_ref(), "This sign-in can’t continue".as_bytes());
        }
        let tree = render(State::Busy, b"client_id=forgejo");
        let text = fixtures::main_text(tree.tree());
        assert!(core::str::from_utf8(&text).unwrap().contains("temper couldn’t sign you in just now"));
        assert!(core::str::from_utf8(&text).unwrap().contains("Nothing was kept: try again in a moment."));
    }
    #[test]
    fn authorization_boundaries_draw_the_domain_selected_page() {
        use temper_client_domain as d;
        for (query_bytes, signed_out) in [
            (1024 - b"/oauth/authorize?".len(), true),
            (1025 - b"/oauth/authorize?".len(), true),
            (1025 - b"/oauth/authorize?".len(), false),
            (usize::try_from(d::authorize::QUERY_BYTES).unwrap(), false),
            (usize::try_from(d::authorize::QUERY_BYTES).unwrap() + 1, false),
            (usize::try_from(d::authorize::QUERY_BYTES).unwrap() + 1, true),
        ] {
            let limits = fixtures::limits();
            let mut root = if signed_out { fixtures::root_in_project(None) } else { d::new(&limits, 11) };
            let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits };
            let mut out = skein_lib::Queue::uncapped();
            if signed_out {
                d::step(&mut root, &env, d::Event::SignedOut, &mut out);
            } else {
                d::step(
                    &mut root,
                    &env,
                    d::Event::Client { event: jig_client::Event::SignIn { with: jig_client::With::Transport } },
                    &mut out,
                );
            }
            let mut path = b"/oauth/authorize?".to_vec();
            path.resize(path.len() + query_bytes, b'x');
            d::step(&mut root, &env, d::Event::Went { place: d::address::parse(&path) }, &mut out);
            // A starting link waits on the continuing page; a signed-out
            // link selects its page immediately.
            if signed_out {
                d::take(&mut root, &env, &mut out);
            }
            let page = root.authorization().unwrap();
            let tree = render(page.state(), page.query());
            let heading = if query_bytes > usize::try_from(d::authorize::QUERY_BYTES).unwrap()
                || (signed_out && path.len() > 1024)
            {
                "This sign-in can’t continue"
            } else if signed_out {
                "Sign in"
            } else {
                "Continuing to Forgejo…"
            };
            assert!(fixtures::find(tree.tree(), Role::Heading { level: 1 }, heading.as_bytes()).is_some());
            if heading == "This sign-in can’t continue" {
                assert_eq!(fixtures::main_text(tree.tree()).as_ref(), heading.as_bytes());
            }
        }
    }
    #[test]
    fn signed_out_keeps_the_raw_authorization_as_then_and_names_its_forge() {
        for (query, name) in [
            (b"client_id=f%6Frgejo&state=a+b".as_slice(), b"Forgejo at forge.example:3000".as_slice()),
            (b"client_id=unknown".as_slice(), b"Forgejo".as_slice()),
        ] {
            let tree = render(State::SignedOut, query);
            assert!(
                core::str::from_utf8(&fixtures::main_text(tree.tree()))
                    .unwrap()
                    .contains(core::str::from_utf8(name).unwrap())
            );
            let link = fixtures::find(tree.tree(), Role::Link, b"Continue with Google").unwrap();
            let node = tree.tree().nodes().iter().find(|node| node.id == link).unwrap();
            let mut path = b"/oauth/authorize?".to_vec();
            path.extend_from_slice(query);
            assert_eq!(node.external.as_deref(), Some(google(b"https://temper.example", &path).as_ref()));
        }
    }
}
