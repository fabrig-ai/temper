//! Standalone issuer/model binary seam; register with `mod oauth;` in the owner's module.
use crate::support::{self, cluster, ctl};
use serde_json::{Value, json};
use skein_io::kernel::Exit;
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

const SEED: &str = "23";
const ANSWER: &str = "A first reply through the standalone fake issuer.";
const WITHIN: Duration = Duration::from_secs(20);
const MEMBERS: usize = 3;
const COMPLETED_CALLS: usize = 12;
const CHAT_READ: usize = 9;

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("fixture document")).expect("fixture JSON")
}
fn write(path: &Path, value: &Value) {
    support::write_private(path, &serde_json::to_vec(value).expect("fixture JSON"));
}

/// The cluster owns readiness, pipes, cgroups, signals and clean tree shutdown.
fn description(directory: &Path) {
    fs::create_dir(directory).expect("description directory");
    let peer = |name: &str, kind: &str| {
        json!({
            "name":name,"env":{},"files":[],"memory_bytes":null,
            "argv":["{artifact.temper}","fake",kind,"--document",format!("{{proc.{name}.dir}}/peer.json"),"--ready","ready ","--seed",SEED],
            "ready":[{"capture":"address","prefix":"ready ","stream":"stdout"}]
        })
    };
    write(
        &directory.join("experiment.json"),
        &json!({
            "format":"lab-experiment/1","cost":"temper-m1-calibration","dirs":[],
            "processes":[peer("issuer", "oauth"), peer("model", "llm"), {
                "name":"engine","env":{},"files":[],"memory_bytes":null,
                "argv":["{artifact.temper}","box","--config","{proc.engine.dir}","--data","{proc.engine.dir}","--cache","{proc.engine.dir}"],
                "ready":[{"capture":"link","prefix":"http://","stream":"stdout"}]
            }]
        }),
    );
}

fn initialize(directory: &Path) -> (SocketAddr, SocketAddr, SocketAddr) {
    let initialized = std::process::Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(support::configuration::flags(directory))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(directory)
        .output()
        .expect("initialize the shipped registration");
    assert!(initialized.status.success(), "{}", String::from_utf8_lossy(&initialized.stderr));
    support::own_listener_files(directory);
    let path = directory.join("temper.json");
    let mut value = read(&path);
    let taken: Vec<SocketAddr> = [
        "/application/clients/listener/socket",
        "/application/webhooks/listener/socket",
        "/application/demo/llm/listen/socket",
        "/application/setup/0/mode/fake/listen/socket",
        "/application/setup/1/mode/owned/listen/socket",
    ]
    .iter()
    .map(|pointer| {
        value.pointer(pointer).expect("init listener").as_str().expect("socket string").parse().expect("init socket")
    })
    .collect();
    let [issuer, callback, model] = support::free_addresses(&taken);
    let sign_in = &mut value["engine"]["llm"]["providers"][0]["sources"]["sign_in"];
    let original = sign_in["issuer"].as_str().expect("shipped issuer").to_owned();
    for member in ["issuer", "authorization", "token", "revocation", "keys"] {
        sign_in[member] =
            sign_in[member].as_str().expect("shipped endpoint").replace(&original, &format!("http://{issuer}")).into();
    }
    sign_in["redirect_port"] = callback.port().into();
    sign_in["transport"] = "plaintext".into();
    sign_in.as_object_mut().expect("registration").remove("server_name");
    write(&path, &value);
    (issuer, callback, model)
}

fn model_endpoint(directory: &Path, model: &str) {
    for name in ["temper.json", "agent.json"] {
        let path = directory.join(name);
        let mut value = read(&path);
        let endpoint = if name == "temper.json" {
            &mut value["engine"]["llm"]["endpoints"][0]
        } else {
            &mut value["endpoints"][0]
        };
        endpoint["address"] = if name == "temper.json" { json!({"socket":model}) } else { json!(model) };
        endpoint["transport"] = "plaintext".into();
        for tls in ["server_name", "trust", "trust_der"] {
            endpoint.as_object_mut().expect("endpoint").remove(tls);
        }
        write(&path, &value);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Peers,
    Engine,
    Chat,
    Stopping,
}
struct Judge {
    members: cluster::Members,
    directory: PathBuf,
    client: PathBuf,
    issuer: SocketAddr,
    stage: Stage,
    visited: bool,
    deadline: Time,
    timed_out: bool,
}
impl Referee<cluster::Root> for Judge {
    fn act(&mut self, now: Time, hosts: &mut [cluster::Root]) {
        let (members, client) = hosts.split_at_mut(MEMBERS);
        self.members.act(now, members);
        let cluster::Root::Client { calls, .. } = &mut client[0] else { panic!("client calls") };
        if now >= self.deadline || self.members.failed() {
            self.timed_out |= now >= self.deadline;
            if let Some(calls) = calls {
                calls.cancel();
            }
            self.members.stop();
            self.stage = Stage::Stopping;
            self.members.act(now, members);
            return;
        }
        if self.stage == Stage::Peers && self.members.is_up() {
            model_endpoint(&self.directory, self.members.capture("model", "address").expect("model readiness"));
            self.members.up(&["engine"]);
            self.stage = Stage::Engine;
            self.members.act(now, members);
        }
        if self.stage == Stage::Engine && self.members.is_up() {
            let link = format!("http://{}", self.members.capture("engine", "link").expect("engine link"));
            let mut batch = ctl::Calls::linked(&link, &self.client);
            batch.push(ctl::Step::Run(ctl::Call::sign_in()));
            for arguments in [
                &["me", "--json"][..],
                &["account", "connect", "1", "--key", "standalone-sign-in"],
                &["account", "list", "--json"],
                &["account", "list", "--until", "settled", "--json"],
                &["account", "list", "--json"],
                &[
                    "chat",
                    "start",
                    "--project",
                    "1",
                    "--model",
                    "gpt-6-luna",
                    "--key",
                    "first-standalone-chat",
                    "Hello",
                    "--json",
                ],
                &["task", "show", "1", "--until", "replied", "--json"],
                &["task", "show", "1", "--json"],
                &["chat", "show", "1", "--json"],
                &["chat", "close", "1", "--key", "close-standalone-chat", "--json"],
                &["task", "show", "1", "--until", "ended", "--json"],
            ] {
                batch.push(ctl::Step::Run(ctl::Call::new(arguments)));
            }
            *calls = Some(Box::new(batch));
            self.stage = Stage::Chat;
        }
        let Some(batch) = calls else { return };
        if self.stage == Stage::Chat && !self.visited && batch.ends().len() >= 3 {
            let authorization = batch.ends()[2]
                .stdout
                .split(|b| *b == b'\n')
                .find(|line| line.starts_with(b"http://127.0.0.1:"))
                .expect("connect's authorization URL");
            let authorization = std::str::from_utf8(authorization).expect("local URL");
            let grant = support::github::request_url(self.issuer, "GET", authorization, "application/json", b"");
            assert_eq!(grant.status, 302, "issuer auto-grants the shipped registration");
            let callback = grant.location.expect("authorization redirect");
            let address = callback
                .strip_prefix("http://")
                .expect("loopback callback")
                .split('/')
                .next()
                .expect("callback authority")
                .parse()
                .expect("callback socket");
            let returned = support::github::request_url(address, "GET", &callback, "application/json", b"");
            assert_eq!(returned.status, 200, "the normal ctl sign-in callback finishes");
            self.visited = true;
        }
        if self.stage == Stage::Chat && batch.ends().len() == COMPLETED_CALLS && batch.is_empty() {
            self.members.stop();
            self.stage = Stage::Stopping;
            self.members.act(now, members);
        }
    }
    fn observe(&mut self, now: Time, hosts: &[cluster::Root]) {
        self.members.observe(now, &hosts[..MEMBERS]);
    }
    fn next_deadline(&self) -> Option<Time> {
        self.members.next_deadline().into_iter().chain((self.stage != Stage::Stopping).then_some(self.deadline)).min()
    }
    fn overdue(&self, now: Time) -> Option<String> {
        self.members.overdue(now)
    }
    fn passed(&self) -> bool {
        if !self.members.passed() {
            return false;
        }
        let stopped = self.members.stopped().expect("all three leaders stopped");
        assert_eq!(stopped.len(), MEMBERS);
        for child in stopped {
            assert_eq!(child.exit, Some(Exit::Code(0)), "{}: {}", child.name, String::from_utf8_lossy(&child.stderr));
            assert!(!child.killed && child.survivors.is_empty(), "{} drained without cleanup", child.name);
        }
        !self.timed_out && self.visited
    }
}

#[test]
fn standalone_oauth_signs_in_and_answers_the_first_model_chat() {
    let scratch = support::Scratch::new("standalone_fake_oauth");
    let description_dir = scratch.path().join("description");
    description(&description_dir);
    let mut description = cluster::Cluster::read_from(scratch.path(), &description_dir);
    let directory = description.prepare("engine");
    let (issuer, callback, model) = initialize(&directory);
    let issuer_dir = description.prepare("issuer");
    write(
        &issuer_dir.join("peer.json"),
        &json!({"listen":{"socket":issuer.to_string()},"redirect_port":callback.port()}),
    );
    let model_dir = description.prepare("model");
    write(
        &model_dir.join("peer.json"),
        &json!({
            "listen":{"socket":model.to_string()},"tls":null,"key":model_dir.join("primary.key"),
            "added_keys":[model_dir.join("signed-in.key")],
            "models":[{"name":"gpt-6-luna","window":131_072,"output":65_536}],
            "cued":[],"asked":[],"otherwise":[ANSWER,"The first chat is closed."]
        }),
    );
    let clock = Clock::new();
    let began = clock.now().now;
    let (mut members, hosts) = description.members();
    members.up(&["issuer", "model"]);
    let mut world = real::World::new(Judge {
        members,
        client: directory.join("client"),
        directory,
        issuer,
        stage: Stage::Peers,
        visited: false,
        deadline: began.saturating_add(WITHIN),
        timed_out: false,
    });
    for host in hosts {
        world.spawn(|| host);
    }
    world.spawn(cluster::Root::client);
    let outcome = world.run(&clock, WITHIN);
    assert!(outcome.procs.iter().all(Host::is_empty), "all supervised children and pipes drained");
    let cluster::Root::Client { calls: Some(calls), .. } = &outcome.procs[MEMBERS] else {
        panic!("finished client calls")
    };
    for call in calls.ends() {
        assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
    }
    support::sign_in::assert_plan_account(&calls.ends()[1], &calls.ends()[5]);
    let chat = ctl::json(&calls.ends()[CHAT_READ]);
    let replies: Vec<_> = chat["activity"]["rows"]
        .as_array()
        .expect("conversation rows")
        .iter()
        .filter_map(|row| row["event"]["owner"]["decoded"]["turn"]["decoded"]["messages"].as_array())
        .flatten()
        .filter(|message| message["speaker"] == "assistant")
        .flat_map(|message| message["blocks"].as_array().expect("assistant blocks"))
        .filter_map(|block| block["text"]["text"].as_str())
        .collect();
    assert_eq!(replies, [ANSWER], "first reply committed through the model's added credential");
    eprintln!("standalone fake OAuth story: {:?}", clock.now().now.saturating_since(began));
    scratch.remove();
}
