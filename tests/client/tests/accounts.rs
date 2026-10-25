use skein_lib::Secret;
use temper_client_domain::{
    self as d,
    pages::{accounts::SignInState, connect_model::Act},
};
use temper_client_world::{fixtures, front::account_acts as accounts};
const NAME: [u8; 16] = [113; 16];
fn act(act: Act) -> temper_client_world::app::Event {
    d::Event::Act { action: d::Action::Pages(d::pages::Act::ConnectModel(act)) }.into()
}
fn select() -> temper_client_world::app::Event {
    d::Event::Went {
        place: d::Place::Address(d::Address::ConnectModel {
            way: Some(d::ModelWay { source: None, provider: accounts::PROVIDER, kind: d::WayKind::Key }),
        }),
    }
    .into()
}
#[test]
fn a_lost_key_answer_replays_under_the_same_name_without_a_second_account() {
    let (mut front, mut root) = accounts::fixture(fixtures::SEED);
    accounts::event(&mut front, &mut root, select());
    accounts::lose_next(&mut front);
    accounts::event(
        &mut front,
        &mut root,
        d::Event::AccountKey { value: Secret::new(Box::from(b"test-private-key".as_slice())) }.into(),
    );
    accounts::event(&mut front, &mut root, act(Act::Submit { name: NAME }));
    let source = accounts::kept(&mut front);
    accounts::settle(&mut front, &mut root);
    accounts::lost(&mut front, &mut root);
    let page = root.person().pages().connect_model().expect("account fixture state");
    assert_eq!(page.request.pending.as_ref().expect("account fixture state").outcome, Some(jig_client::Outcome::Lost));
    accounts::event(
        &mut front,
        &mut root,
        d::Event::AccountKey { value: Secret::new(Box::from(b"test-private-key".as_slice())) }.into(),
    );
    accounts::event(&mut front, &mut root, act(Act::Submit { name: NAME }));
    let root = root.person();
    assert_eq!(
        root.pages()
            .connect_model()
            .expect("account fixture state")
            .request
            .pending
            .as_ref()
            .expect("account fixture state")
            .outcome,
        Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(jig_client::contract::Answer::Done(
            jig_client::contract::Outcome::Owner(jig_client::contract::OwnerOutcome::AccountKeyKept { source })
        ))))
    );
    let doc = d::pages::accounts::document(root.client()).expect("account fixture state");
    assert_eq!(doc.accounts.len(), 1);
    assert_eq!(doc.accounts.get(0).expect("account fixture state").sources.len(), 1);
    accounts::assert_no_carried(&front);
}
#[test]
fn a_sign_in_started_answer_is_followed_by_its_actual_end_on_the_accounts_watch() {
    use jig_domain_accounts as a;
    let (mut front, mut root) = accounts::plan_fixture(fixtures::SEED);
    accounts::event(
        &mut front,
        &mut root,
        d::Event::Went { place: d::Place::Address(d::Address::ConnectModel { way: None }) }.into(),
    );
    let limits = fixtures::domain_limits();
    let words = fixtures::phrasebooks(&limits);
    let bounds = fixtures::view_bounds(&limits, &words);
    let mut view = temper_client_view::View::new(&bounds, &limits, words, fixtures::FORGE);
    let mut patches = skein_lib::Queue::with_capacity(bounds.tree.patches);
    temper_client_view::build(&mut view, root.person(), fixtures::NOW, &mut patches);
    let button = view
        .tree()
        .nodes()
        .iter()
        .find(|node| node.role == skein_ui::Role::Button && node.name.as_deref() == Some(b"Continue with ChatGPT"))
        .expect("plan sign-in way")
        .id;
    let press = temper_client_view::event(&mut view, button, skein_ui::Act::Press).expect("way press");
    accounts::event(&mut front, &mut root, press.into());
    let source = accounts::signing(&front);
    front.engine.send(temper_engine_domain::Event::Account(a::Event::Visited {
        source,
        address: Box::from(b"https://issuer/visit".as_slice()),
    }));
    accounts::settle(&mut front, &mut root);
    assert_eq!(
        root.person()
            .pages()
            .connect_model()
            .expect("account fixture state")
            .request
            .sign_in_state(root.person().client()),
        SignInState::Signing { source }
    );
    patches = skein_lib::Queue::with_capacity(bounds.tree.patches);
    temper_client_view::build(&mut view, root.person(), fixtures::NOW, &mut patches);
    assert!(
        view.tree()
            .nodes()
            .iter()
            .any(|node| node.role == skein_ui::Role::Link && node.external.as_deref() == Some(b"https://issuer/visit"))
    );
    front.engine.send(temper_engine_domain::Event::Account(a::Event::SignedIn {
        source,
        subject: Box::from(b"subject".as_slice()),
        display: Box::from(b"alice@example.com".as_slice()),
        declined: false,
    }));
    accounts::settle(&mut front, &mut root);
    let root = root.person();
    assert_eq!(
        root.pages().connect_model().expect("account fixture state").request.sign_in_state(root.client()),
        SignInState::Ended { source }
    );
    assert!(d::pages::accounts::document(root.client()).expect("account fixture state").last_end.is_some());
    assert_eq!(
        root.pages().connect_model().expect("account fixture state").request.sign_in_address(root.client()),
        None
    );
    patches = skein_lib::Queue::with_capacity(bounds.tree.patches);
    temper_client_view::build(&mut view, root, fixtures::NOW, &mut patches);
    let text: Vec<u8> =
        view.tree().nodes().iter().flat_map(|node| node.text.iter().flat_map(|run| run.text.iter().copied())).collect();
    assert!(String::from_utf8(text).expect("view words").contains("Signed in to ChatGPT as alice@example.com"));
}

#[test]
fn a_client_shows_a_third_account_at_two_rows() {
    // Three independently keyed adds exercise one row beyond the client's two-row bound.
    const NAMES: [[u8; 16]; 3] = [[113; 16], [114; 16], [115; 16]];
    let (mut front, mut root) = accounts::small_rows_fixture(fixtures::SEED);
    accounts::event(&mut front, &mut root, select());
    for name in NAMES {
        accounts::event(
            &mut front,
            &mut root,
            d::Event::AccountKey { value: Secret::new(Box::from(b"test-private-key".as_slice())) }.into(),
        );
        accounts::event(&mut front, &mut root, act(Act::Submit { name }));
        accounts::kept(&mut front);
        accounts::settle(&mut front, &mut root);
    }
    accounts::event(&mut front, &mut root, d::Event::Went { place: d::Place::Address(d::Address::Accounts) }.into());
    let doc = d::pages::accounts::document(root.person().client()).expect("accounts document");
    let third = doc.accounts.get(2).expect("third account decoded whole").number;
    let mut limits = fixtures::domain_limits();
    limits.client.domain.rows = accounts::SMALL_ROWS;
    let words = fixtures::phrasebooks(&limits);
    let bounds = fixtures::view_bounds(&limits, &words);
    let mut view = temper_client_view::View::new(&bounds, &limits, words, fixtures::FORGE);
    temper_client_view::build(
        &mut view,
        root.person(),
        fixtures::NOW,
        &mut skein_lib::Queue::with_capacity(bounds.tree.patches),
    );
    // The row is named by its provider and holder, rather than “Account N”.
    // Its remove act still identifies the independently keyed third account.
    let removes: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter_map(|node| {
            (node.role == skein_ui::Role::Button && node.name.as_deref() == Some(b"Remove account")).then_some(node.id)
        })
        .collect();
    let shown = removes.into_iter().any(|node| {
        matches!(
            temper_client_view::act(&mut view, node, skein_ui::Act::Press),
            Some(d::Action::Pages(d::pages::Act::Accounts(d::pages::accounts::Act::Submit {
            row, operation: d::pages::accounts::Operation::RemoveAccount, ..
        }))) if row.account == third
        )
    });
    assert!(shown, "third account has its own row and remove act");
}

#[test]
fn me_carries_the_configured_provider_offer() {
    let (_, root) = accounts::fixture(fixtures::SEED);
    let me = d::pages::accounts::current_me(root.person().client()).expect("authenticated me");
    let offer = me.providers.get(0).expect("configured provider");
    assert_eq!(
        (
            (me.providers.len(), me.key_bytes, me.providers_cut),
            (
                offer.provider,
                offer.name.bytes(),
                offer.sign_in,
                offer.key,
                offer.programs.len(),
                offer.programs_cut,
                offer.none,
                offer.signed_in_plan,
                offer.manage_usage.as_deref(),
                offer.key_bytes,
            ),
        ),
        ((1, 256, false), (accounts::PROVIDER, b"Scripted".as_slice(), true, true, 0, false, false, false, None, 256))
    );
}
