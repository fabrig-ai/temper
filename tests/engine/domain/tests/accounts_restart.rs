//! Issued names precede live account rows in jig's restart walk
//! (jig's protocol/store.md, 4.4 and 7.2; domain/root.md, 9.1).
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_people as people;
use skein_lib::{Queue, ReplyTo, Token};
use temper_engine_domain::{self as engine, Key, Record};
use temper_engine_domain_world::{direct::Driver, store::Store, walking};

const SEED: u64 = 91;
const CLAUDE: u32 = 2;
const CHATGPT: u32 = 3;
const LOGIN: &[u8] = b"login";
// One declared program account and one imported sign-in account.
const ACCOUNTS: u32 = 2;
// The walking world's normal page holds the reservation and counter together.
const PAGE: u32 = 2;
// A one-row page crosses that same boundary through an exclusive `after`.
const SINGLE_ROW: u32 = 1;
// Six stored rows, read two at a time.
const PAGES: u32 = 3;
// The direct world's existing settlement step budget, with an earlier exit
// on the restore refusal so a stopped root need not become quiescent.
const RESTORE_STEPS: u32 = 200;

fn config() -> engine::Config {
    let mut config = walking::config(SEED);
    config.core.settings.llm_providers = accounts::Providers {
        providers: Box::new([
            accounts::Provider {
                number: CLAUDE,
                name: b"Fake Claude".as_slice().into(),
                signed_in_plan: true,
                manage_usage: None,
                models: Box::new([]),
                offers: accounts::Offers {
                    sign_in: false,
                    key: false,
                    programs: Box::new([b"claude_code".as_slice().into()]),
                },
            },
            accounts::Provider {
                number: CHATGPT,
                name: b"Fake ChatGPT".as_slice().into(),
                signed_in_plan: true,
                manage_usage: None,
                models: Box::new([]),
                offers: accounts::Offers { sign_in: true, key: false, programs: Box::new([]) },
            },
        ]),
    };
    config
}

fn limits() -> engine::Limits {
    let mut limits = walking::limits();
    limits.accounts.accounts = ACCOUNTS;
    limits.accounts.holder_accounts = ACCOUNTS;
    // The core reserves per-run slots for configured credentials and accounts.
    limits.fleet.grants = limits.accounts.credentials + ACCOUNTS;
    limits
}

fn kept_store() -> Store {
    // Start empty: Driver::configured seeds deployment key accounts, which
    // would consume both slots before either operation in this story.
    let mut driver = Driver::cold(Store::new(), config(), &limits());
    driver.settle();
    driver.sign_in();
    driver.settle();
    driver.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(101)),
        sign_in: driver.session(),
        key: [1; 16],
        ask: people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::DeclareProgram { provider: CLAUDE, join: None, name: LOGIN.into() },
        },
    }));
    driver.settle();
    // Play only the imported record's component boundary, with no real login
    // or credential bytes. The root and accounts child write the durable rows.
    driver.send(engine::Event::Party(engine::PartyInput::Key {
        reply_to: ReplyTo::new(Token::new(102)),
        sign_in: driver.session(),
        key: [2; 16],
        ask: Box::new(people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::MoveIn { digest: [7; 32] },
        }),
        carried: core::Carried { number: 1_u64 << 63, digest: [7; 32] },
    }));
    driver.settle();
    let source = driver
        .protocol
        .accounts
        .iter()
        .find_map(|request| {
            if let accounts::Request::Signing(accounts::Signing::Open { source, .. }) = request {
                Some(*source)
            } else {
                None
            }
        })
        .expect("the fake component is asked to open the imported record");
    driver.send(engine::Event::Account(accounts::Event::Opened {
        source,
        provider: CHATGPT,
        subject: b"fake-chatgpt-owner".as_slice().into(),
        display: b"Fake owner".as_slice().into(),
    }));
    driver.settle();
    driver.protocol.sign_in_sources.insert(source);
    driver.send(engine::Event::Account(accounts::Event::Adopted { source, kept: true }));
    driver.settle();
    // Replay the real durable commits into a fresh store component.
    driver.store.fork()
}

fn account_key(key: accounts::Key) -> Key {
    Key::Core(core::Key::Accounts(key))
}

#[test]
fn declared_and_imported_rows_walk_reservations_before_the_counter() {
    let mut store = kept_store();
    let range = engine::Range::Core(engine::CoreRange::Accounts);
    let mut after = None;
    let mut keys = Vec::new();
    for _ in 0..PAGES {
        let (rows, next) = store.page(&range, after.as_ref(), PAGE);
        keys.extend(rows.iter().map(Record::key));
        after = next;
    }
    assert_eq!(
        keys,
        vec![
            account_key(accounts::Key::Issued { number: 1 }),
            account_key(accounts::Key::Next),
            account_key(accounts::Key::Account { number: 0 }),
            account_key(accounts::Key::Account { number: 3 }),
            account_key(accounts::Key::Source { account: 0, number: 1 }),
            account_key(accounts::Key::Source { account: 3, number: 2 }),
        ]
    );
}

#[test]
fn restart_accepts_issued_and_counter_in_one_page() {
    let mut driver = Driver::cold(kept_store(), config(), &limits());
    for _ in 0..RESTORE_STEPS {
        driver.advance(true);
        if driver.root.restore_load_failure().is_some() || driver.root.ready() {
            break;
        }
    }
    assert_eq!(driver.root.restore_load_failure(), None, "restart must accept the store's accounts walk");
    assert!(driver.root.ready(), "restored accounts permit the startup switch");
}

#[test]
fn accounts_load_accepts_counter_after_an_issued_page_cursor() {
    use engine::loads::{self, Loads, Request};
    let mut store = kept_store();
    let range = engine::Range::Core(engine::CoreRange::Accounts);
    let (_, after) = store.page(&range, None, SINGLE_ROW);
    let (rows, next) = store.page(&range, after.as_ref(), SINGLE_ROW);
    let mut loads = Loads::new(&limits().loads);
    // Each loads entry point emits at most one request or terminal.
    let mut out = Queue::with_capacity(1);
    let waiter = Token::new(103);
    let owner = loads::begin(&mut loads, waiter, range, after, SINGLE_ROW, &mut out).expect("load admitted");
    let _ = out.pop().expect("store request");
    loads::loaded(&mut loads, owner, rows, next, &mut out);
    let terminal = out.pop();
    assert_eq!(
        terminal,
        Some(Request::Loaded {
            waiter,
            rows: Box::new([Record::Core(core::Record::Accounts(accounts::Stored::Next { number: 4 }))]),
            next: Some(account_key(accounts::Key::Next)),
            cut: None,
        }),
        "accounts continuation must accept the counter after Issued without skipping it"
    );
}
