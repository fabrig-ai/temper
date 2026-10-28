use super::*;
use crate::forge::observations::{self, GitCalls, Observation};
use skein_fake_forge_domain as domain;
use skein_fake_forge_github::{
    GitCall, Minted,
    installations::{Reach, Scope},
    service::{Outcome, Request},
};
use skein_fake_forge_protocol::git::{self, Call};
use skein_forge::github::request::Method;
use skein_lib::{Duration, List, Secret, Writer, base64};

fn document() -> Value {
    let mut doc = forge_document();
    doc["owner"] = json!("acme");
    doc["github"] = json!({
        "app":{"id":4242,"slug":"temper-test","bot":4243,"key":0},
        "accounts":[
            {"login":"acme","id":22,"kind":"organization","installation":{"id":1,"repositories":"all"}},
            {"login":"umbra","id":23,"kind":"user","installation":null}
        ]
    });
    doc
}
fn checked(doc: &Value) -> forge::Document {
    forge::Document::decode(&serde_json::to_vec(doc).unwrap()).unwrap()
}

#[test]
fn a_github_document_installs_each_accounts_installation() {
    for life in [None, Some(90_u64)] {
        let mut doc = document();
        if let Some(life) = life {
            doc["github"]["token_life_seconds"] = json!(life);
        }
        // An uninstalled account consumes no id; the next installed account gets 2.
        doc["github"]["accounts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"login":"third","id":24,"kind":"user","installation":{"id":2,"repositories":[]}}));
        let (mut peer, _forge) = forge::peer(&checked(&doc), &[b'a'; 40], 0).unwrap();
        let store = peer.github_layer_mut().unwrap().installations_mut();
        let installation = store.get(1).unwrap();
        assert_eq!(installation.account, 22);
        assert_eq!(installation.login.as_ref(), b"acme");
        match &installation.reach {
            Reach::All => {}
            Reach::Listed(_) => panic!("all repositories selected"),
        }
        assert_eq!(store.iter().filter(|row| row.account == 23).count(), 0);
        assert_eq!(store.get(2).unwrap().account, 24);
        let (_, expires) = store.mint(1, Scope::Contents, Wall::EPOCH).unwrap();
        assert_eq!(expires.as_nanos(), Duration::from_secs(life.unwrap_or(3600)).as_nanos());
    }
    // Same repository name on different owners; listed reach uses their own row ids.
    let mut doc = document();
    doc["repositories"]
        .as_array_mut()
        .unwrap()
        .push(json!({"owner":"umbra","name":"widget","landing":"main","ci":null}));
    doc["github"]["accounts"][0]["installation"]["repositories"] = json!(["widget"]);
    let (peer, shared) = forge::peer(&checked(&doc), &[b'a'; 40], 0).unwrap();
    let installation = peer.github_layer().unwrap().installations().get(1).unwrap();
    let Reach::Listed(ids) = &installation.reach else { panic!("listed reach") };
    assert_eq!(ids.as_slice(), &[1]);
    shared
        .borrow()
        .domain
        .inspect(&forge::LIMITS, b"umbra/widget", &domain::api::Read::Branch { branch: b"main".as_slice().into() })
        .expect("the second owner's repository exists");
}

#[test]
fn a_github_document_with_an_unknown_owner_or_key_is_refused() {
    for (pointer, value, path) in [
        ("/owner", json!("absent"), "repositories[0].owner"),
        ("/github/app/key", json!(3), "github.app.key"),
        ("/github/accounts/1/id", json!(22), "github.accounts[1]"),
        ("/github/accounts/1/login", json!("acme"), "github.accounts[1]"),
        ("/github/accounts/1/installation", json!({"id":1,"repositories":"all"}), "github.accounts[1].installation.id"),
        ("/github/accounts/0/installation/id", json!(2), "github.accounts[0].installation.id"),
        ("/github/accounts/0/installation/repositories", json!(["absent"]), "github.accounts"),
        ("/github/app/bot", json!(4242), "github.app.bot"),
        ("/github/app/id", json!(0), "github.app.id"),
    ] {
        let mut doc = document();
        *doc.pointer_mut(pointer).unwrap() = value;
        let refusal = forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).unwrap_err();
        assert!(refusal.line().starts_with(path.as_bytes()), "{pointer}: {}", String::from_utf8_lossy(&refusal.line()));
    }
    let mut doc = document();
    doc["github"]["token_life_seconds"] = json!(0);
    forge::Document::decode(&serde_json::to_vec(&doc).unwrap()).expect_err("zero token life refused");
}

fn basic(token: &Secret) -> Box<[u8]> {
    let value = super::super::read::joined(&[b"x-access-token:", token.expose()]);
    let length = base64::encoded_len(base64::Alphabet::Standard, u32::try_from(value.len()).unwrap()).unwrap();
    let mut out = Writer::new(usize::try_from(length).unwrap());
    base64::encode(base64::Alphabet::Standard, &value, &mut out).unwrap();
    super::super::read::joined(&[b"Basic ", &out.finish()])
}
fn clone(
    peer: &mut skein_fake_peers::forge::Peer,
    forge: &mut skein_fake_peers::forge::Forge,
    authorization: Option<&[u8]>,
) -> u16 {
    let call = git::client::encode(&Call::Clone { repository: b"acme/widget".as_slice().into() }, &forge::git_limits())
        .unwrap();
    let service = peer.github_layer_mut().unwrap();
    let reply = || ReplyTo::new(Token::new(77));
    let mut outcome = service.request(
        &Request {
            method: Method::Post,
            target: &call.target,
            body: &call.body,
            authorization,
            version: None,
            agent: false,
            validator: None,
        },
        Wall::EPOCH,
        reply(),
    );
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: forge::LIMITS };
    for _ in 0..8 {
        outcome = match outcome {
            Outcome::Reply(response) => return response.status,
            Outcome::Admit(admission) => service.admit(admission, &mut forge.domain, &env, reply()),
            Outcome::Call(pending, event) => {
                let mut out = Queue::uncapped();
                domain::step_admitted(&mut forge.domain, &env, event, &mut out);
                domain::fire(&mut forge.domain, &env, &mut out);
                let mut result = None;
                while let Some(request) = out.pop() {
                    match request {
                        domain::Request::Reply { result: answer, .. } => result = Some(answer),
                        domain::Request::Hook { .. } => {}
                    }
                }
                service.finish(pending, result.expect("zero-latency domain reply"), reply(), &forge.domain)
            }
        };
    }
    panic!("clone settles")
}

#[test]
fn the_observations_hold_each_git_call_without_its_token() {
    let (mut peer, shared) = forge::peer(&checked(&document()), &[b'a'; 40], 0).unwrap();
    let token = peer.github_layer_mut().unwrap().installations_mut().mint(1, Scope::Contents, Wall::EPOCH).unwrap().0;
    let authorization = basic(&token);
    assert_eq!(clone(&mut peer, &mut shared.borrow_mut(), Some(&authorization)), 200);
    let mut cursor = GitCalls::new();
    let mut out = List::with_capacity(4096);
    assert!(!cursor.drain(peer.git_calls().unwrap(), &mut out).unwrap());
    assert_eq!(
        observations::decode(out.as_slice()).unwrap(),
        Some(Observation::GitCall(GitCall {
            minted: Some(Minted { installation: 1, mint: 1 }),
            writes: false,
            status: 200
        }))
    );
    assert!(!out.as_slice().windows(token.expose().len()).any(|window| window == token.expose()));
    assert!(!out.as_slice().windows(authorization.len()).any(|window| window == authorization.as_ref()));
    let first = out.len();
    assert!(!cursor.drain(peer.git_calls().unwrap(), &mut out).unwrap());
    assert_eq!(out.len(), first, "a call is written once");
    assert_eq!(clone(&mut peer, &mut shared.borrow_mut(), None), 401);
    out.clear();
    assert!(!cursor.drain(peer.git_calls().unwrap(), &mut out).unwrap());
    assert_eq!(
        observations::decode(out.as_slice()).unwrap(),
        Some(Observation::GitCall(GitCall { minted: None, writes: false, status: 401 }))
    );
}

#[test]
fn a_full_git_call_ring_ends_the_observation_stream() {
    let mut calls = Queue::with_capacity(2);
    let call = GitCall { minted: Some(Minted { installation: 1, mint: 2 }), writes: true, status: 200 };
    let mut cursor = GitCalls::new();
    let mut out = List::with_capacity(4096);
    calls.push(call);
    assert!(!cursor.drain(&calls, &mut out).unwrap());
    out.clear();
    calls.push(call);
    assert!(cursor.drain(&calls, &mut out).unwrap());
    let lines = out.as_slice().split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(observations::decode(lines[0]).unwrap(), Some(Observation::GitCall(call)));
    assert_eq!(observations::decode(lines[1]).unwrap(), Some(Observation::GitCallsFull { capacity: 2 }));
    out.clear();
    assert!(cursor.drain(&calls, &mut out).unwrap());
    assert!(out.is_empty(), "terminal record is written only once");
}
