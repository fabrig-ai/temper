use temper_client_world::{
    app::{Event, Root},
    fixtures,
    front::{Joint, account_acts as accounts},
};
fn call(front: &mut Joint, root: &mut Root, number: u32, words: &[&[u8]]) -> String {
    let Event::Call { call, json, .. } = fixtures::call(words) else { unreachable!() };
    accounts::event(front, root, Event::Call { number, call, json, value: None });
    let printed = root.agent().printed(number).expect("call answered");
    assert_eq!(printed.status, 0, "{printed:?}");
    String::from_utf8(printed.stdout.to_vec()).expect("UTF-8 output")
}
#[test]
fn a_person_amends_a_tasks_controls_and_reads_them_back() {
    let (mut front, mut root) = accounts::tools_fixture(fixtures::SEED);
    let (goal, dependency) = fixtures::amend_tasks(&mut front);
    let goal = goal.to_string();
    let dependency = dependency.to_string();
    call(&mut front, &mut root, 2, &[b"task", b"amend", goal.as_bytes(), b"--priority", b"3", b"Move this first"]);
    let priority = call(&mut front, &mut root, 3, &[b"task", b"show", goal.as_bytes()]);
    assert!(priority.contains("priority 3"), "{priority}");
    let document = call(&mut front, &mut root, 4, &[b"task", b"show", goal.as_bytes(), b"--json"]);
    let revision = document
        .split("\"spec_revision\":{\"value\":")
        .nth(1)
        .expect("spec revision")
        .split('}')
        .next()
        .expect("revision number");

    let scratch = std::env::temp_dir().join(format!("temper-client-amend-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("world scratch directory");
    let instructions = scratch.join("instructions.txt");
    std::fs::write(&instructions, b"Run the focused checks\n").expect("world instructions file");
    call(
        &mut front,
        &mut root,
        5,
        &[
            b"task",
            b"amend",
            goal.as_bytes(),
            b"--rev",
            revision.as_bytes(),
            b"--deadline",
            b"2030-01-02T03:04Z",
            b"--instructions",
            instructions.as_os_str().as_encoded_bytes(),
            b"--dependency",
            dependency.as_bytes(),
            b"Set the controls",
        ],
    );
    std::fs::remove_dir_all(&scratch).expect("remove world scratch directory");
    let text = call(&mut front, &mut root, 6, &[b"task", b"show", goal.as_bytes()]);
    assert!(text.contains("deadline 2030-01-02 03:04 UTC"), "{text}");
    assert!(text.contains("instructions: Run the focused checks"), "{text}");
    assert!(text.contains(&format!("\ndependencies:\n  task {dependency} ")), "{text}");
    assert!(text.contains("added by you at "), "{text}");
    let document = call(&mut front, &mut root, 7, &[b"task", b"show", goal.as_bytes(), b"--json"]);
    assert!(
        document.contains(&format!("\"dependency_facts\":[{{\"number\":{dependency},\"pending\":true,\"added\":{{")),
        "{document}"
    );
    assert!(document.contains("\"instructions\":{\"bytes\":\"Run the focused checks\""), "{document}");
    assert_eq!(
        temper_client_tools::table::parse(&[
            b"task",
            b"amend",
            goal.as_bytes(),
            b"--spec",
            b"8",
            b"--model",
            b"scripted",
            b"because"
        ]),
        Err(temper_client_tools::jig::argv::Usage::AmendmentRevision),
        "the_old_spec_revision_form_asks_for_rev"
    );
}
