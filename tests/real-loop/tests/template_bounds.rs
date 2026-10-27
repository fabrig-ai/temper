//! The init template's two-row bound is below ctl's unchanged 32-row window.
use skein_world::domain::heap::Counting;
use std::{cell::RefCell, rc::Rc};
use temper_real_loop_world::template_bounds::{self, Seen};
#[global_allocator]
static HEAP: Counting = Counting;

#[test]
fn the_two_call_agent_past_smalls_page_is_refused_by_name() {
    use temper_real_loop_world::{fixtures, landing};
    let address = "127.0.0.1:9".parse().expect("local fixture address");
    let (mut agent, mut document) = fixtures::documents(std::path::Path::new("/template-bounds"), address, address);
    landing::configure(&mut document, &mut agent, address);
    agent["profile"]["declared"]["calls_per_response"] = 2.into();
    assert_eq!(agent["profile"]["declared"]["tool_payload"], 4096);
    assert_eq!(agent["endpoints"][0]["models"][0]["output"], 8192);
    let transcript = fixtures::turn_bounds::turn_body(&agent);
    assert_eq!(transcript, 1_792_262);
    fixtures::turn_bounds::apply(&mut document, transcript);
    let bytes = serde_json::to_vec(&document).expect("old two-call configuration");
    let refused = temper_service::named(&bytes, &jig_protocol_config::Directories::working())
        .expect_err("the old two-call shape exceeds Small's page");
    assert_eq!(
        temper_service::startup::lines(&refused).as_ref(),
        b"application.limits.overrides.root.loads.transcript_bytes: its turn's transcript frame, 1792326 bytes, needs a store page past the profile's ceiling of 1590110 bytes\n"
    );
}

#[test]
fn reads_answer_at_the_templates_bounds() {
    let seen = Rc::new(RefCell::new(Seen::default()));
    template_bounds::run(&seen);
    let seen = seen.borrow();
    assert!(seen.completions <= 4, "delegation and wait, plus two reports: {} completions", seen.completions);
    assert!(seen.activity.len() >= 2, "ended activity spans multiple pages");
    assert!(seen.activity.iter().map(|page| page["rows"].as_array().expect("rows").len()).sum::<usize>() >= 3);
    let tree = seen.tree.split_once("tree:\n").expect("tree").1.split_once("digest:").expect("tree digest").0;
    assert_eq!(tree.lines().filter(|line| line.contains(" · ")).count(), 3, "root and two rows: {tree}");
    assert!(tree.starts_with("1 · "), "{tree}");
    assert!(
        tree.contains("cut: 1 more not shown (1 Waiting); read on with: task list --project 1 --parent 1\n"),
        "{tree}"
    );
    assert_eq!(seen.delegates, [4, 5, 6], "all live delegates reached");
}
