//! The shipped box lands the owner's first change by turn on its owned Forgejo.
#[expect(
    dead_code,
    clippy::unused_self,
    clippy::struct_field_names,
    reason = "shared process support includes unused fixtures for other stories"
)]
mod support;

use serde_json::{Value, json};
use skein_io::kernel::Exit;
use skein_lib::Duration;
use std::{cell::Cell, rc::Rc};
use support::{
    Scratch, boxed,
    ctl::{self, Call, Step},
    first_repository::{self, assert_box, flags, generated, stage_binaries},
    landing_by_turn::{self, Person},
};

#[test]
#[expect(clippy::too_many_lines, reason = "the two sittings and forge assertions form one owned turn story")]
fn a_change_lands_by_turn_on_the_owned_forge() {
    // domain/landing.md, 5.2, 6, 7, 8 and 10: the proved candidate is main's exact tip.
    const TASK_START_STEPS: usize = 3; // Account key, usable account, then the owner's first chat.
    const REBASED_PARENTS: usize = 1; // The one change commit is rebased onto the advanced main.
    let clock = skein_shell::Clock::new();
    let began = clock.now().now;
    let scratch = Scratch::new("qa-landing-by-turn");
    let root = scratch.path();
    generated(root);
    stage_binaries(root);
    let settings_root = root.to_path_buf();
    let sitting = clock.now().now;
    let first = boxed::run_ctl(
        root,
        flags(root),
        root,
        vec![
            Step::Run(Call::sign_in()),
            Step::Run(Call::new(&["settings", "show", "policy", "1", "--json"])),
            Step::Run(Call::new(&["settings", "show", "--json"])),
            Step::Then(Box::new(move |calls| {
                let mut reads = calls.iter().rev();
                let inventory = reads.next().expect("settings inventory");
                let policy = reads.next().expect("policy 1");
                landing_by_turn::fund_landings(&settings_root, policy, inventory)
            })),
            Step::Run(Call::new(&["help"])),
            Step::Run(Call::new(&["forge", "setup", "3"])),
            Step::Run(Call::new(&["forge", "list", "--until", "made:3", "--timeout", "60s", "--json"])),
        ],
        Duration::from_secs(180),
        || landing_by_turn::configure_repository(root),
    );
    eprintln!("owned turn first sitting: {}", landing_by_turn::elapsed(&clock, sitting));
    assert_box(&first);
    for call in &first.calls {
        assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
    }
    let setup = first.checked.expect("repository and project configured before stop");
    landing_by_turn::configure_turn(root, &setup);
    let task = first_repository::Task { project: setup.project, ..first_repository::FIRST };
    let mut steps = first_repository::task_steps_for(root, task);
    let remaining = steps.split_off(TASK_START_STEPS);
    steps.push(Step::Until(Call::new(&["task", "show", &task.producer.to_string(), "--json"]), br#""phase":"ended""#));
    steps.push(Step::Until(Call::new(&["change", "show", &task.change.to_string(), "--json"]), br#""pull":{"#));
    let advanced = Rc::new(Cell::new(None));
    let advance = advanced.clone();
    let person = Person::new(root);
    steps.push(Step::Then(Box::new(move |calls| {
        let opened = ctl::json(calls.last().expect("opened pull"));
        assert!(!opened["pull"].is_null(), "the branch was pushed and its pull opened: {opened}");
        let original = person.get("pulls/1")["head"]["sha"].as_str().expect("original pull head").to_owned();
        person.add("notes.txt", landing_by_turn::NOTES);
        let moved = person.get("branches/main")["commit"]["id"].as_str().expect("advanced main").to_owned();
        advance.set(Some((original, moved)));
        Call::new(&["change", "show", &task.change.to_string(), "--json"])
    })));
    steps.push(Step::Start(Call::new(&[
        "change",
        "show",
        &task.change.to_string(),
        "--until",
        "landed",
        "--timeout",
        "300s",
        "--json",
    ])));
    steps.push(Step::Until(Call::new(&["change", "show", &task.change.to_string(), "--json"]), br#""step":"landed""#));
    steps.push(Step::Join);
    steps.extend(remaining);
    steps.insert(0, Step::Run(Call::new(&["forge", "list", "--until", "settled", "--timeout", "60s"])));
    let sitting = clock.now().now;
    let second = boxed::run_ctl_with_llm(
        root,
        flags(root),
        root,
        steps,
        Duration::from_secs(120),
        |_| landing_by_turn::evidence(root, &setup, task.change),
        first_repository::restarted_llm(root),
    );
    eprintln!("owned turn second sitting: {}", landing_by_turn::elapsed(&clock, sitting));
    assert_box(&second);
    let stderr = String::from_utf8_lossy(&second.stderr);
    for call in &second.calls {
        assert_eq!(call.exit, Some(Exit::Code(0)), "{}; box: {stderr}", String::from_utf8_lossy(&call.stderr));
    }
    let evidence = second.checked.unwrap_or_else(|| panic!("landing and forge evidence missing; box: {stderr}"));
    for call in [&evidence.list, &evidence.show, &evidence.panel, &evidence.change, &evidence.task] {
        assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
    }
    let panel = ctl::json(&evidence.panel);
    let stored = ctl::json(&evidence.task);
    eprintln!("owned turn stored landing budget: {}", stored["budget"]);
    assert_eq!(panel["phase"], "landed", "{panel}");
    let member = &panel["members"][0];
    let candidate = member["candidate"].as_str().expect("candidate");
    // The vocabulary pads SHA-1 to 32 bytes; Forgejo uses its original 20 bytes.
    let commit = &candidate[..40];
    assert_eq!(evidence.main["commit"]["id"], commit);
    assert_eq!(member["landed"], candidate);
    assert_eq!(evidence.pull["merged"], true);
    assert_eq!(evidence.pull["merge_commit_sha"], commit);
    assert_eq!(evidence.pull["head"]["sha"], commit);
    let (original, moved) = advanced.take().expect("main moved after the pull opened");
    assert_ne!(original, commit, "the forge rewrote the change's head");
    assert_eq!(evidence.commit["parents"].as_array().expect("parents").len(), REBASED_PARENTS);
    assert_eq!(evidence.commit["parents"][0]["sha"], moved);
    assert!(
        evidence.readme.split(|byte| *byte == b'\n').any(|line| line == b"Changed by temper."),
        "the task's README edit is on main"
    );
    assert_eq!(evidence.notes, landing_by_turn::NOTES);
    let gate = &member["gates"][0];
    let receipt = &gate["proof"]["passed"];
    let run = receipt["run"].as_u64().expect("passed receipt's run");
    let runs = evidence.runs["workflow_runs"].as_array().expect("workflow runs");
    let runs: Vec<&Value> = runs.iter().filter(|run| run["workflow_id"] == "gate.yaml").collect();
    assert_eq!(runs.len(), 1, "one gate dispatch at the candidate: {}", evidence.runs);
    assert_eq!(runs[0]["id"], run);
    assert_eq!(runs[0]["commit_sha"], commit);
    assert_eq!(runs[0]["trigger_event"], "workflow_dispatch");
    assert_eq!(runs[0]["trigger_user"]["login"], "temper");
    let key = gate["key"].as_str().expect("gate key");
    let statuses = evidence.status["statuses"].as_array().expect("commit statuses");
    let status = statuses.iter().find(|status| status["context"] == "temper/gate/gate").expect("gate status");
    assert_eq!(status["status"], "success");
    assert_eq!(status["description"], format!("key {} passed", &key[..16]));
    assert_eq!(status["target_url"], receipt["address"]);
    let show = String::from_utf8(evidence.show.stdout).expect("landing text");
    assert!(show.contains(&format!("passed by run {run}")) && show.contains(&format!("key {}", &key[..16])), "{show}");
    let change = String::from_utf8(evidence.change.stdout).expect("change text");
    assert!(change.contains(&format!("way: turn · landing task {}", panel["task"])), "{change}");
    eprintln!(
        "turn landing evidence: {}",
        json!({"landing":panel["task"], "candidate":commit,
            "gate_run":{"id":runs[0]["id"], "commit_sha":runs[0]["commit_sha"],
                "trigger_event":runs[0]["trigger_event"], "trigger_user":runs[0]["trigger_user"]["login"],
                "status":runs[0]["status"]},
            "gate_status":status, "merge":evidence.pull["merge_commit_sha"]})
    );
    scratch.remove();
    eprintln!("owned turn story wall time: {}", landing_by_turn::elapsed(&clock, began));
}
