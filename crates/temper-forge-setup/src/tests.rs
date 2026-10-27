mod fetch;
mod files;
mod fixtures;
mod given;
mod golden;
mod limits;
mod managed;
mod members;
mod names;
mod parent;
mod prepare;
mod read;
mod run;
mod sign_in;
mod spec;
mod steps;
mod upgrade;

#[test]
fn starter_workflow_is_pinned_to_the_setup_design() {
    assert_eq!(
        crate::starter_workflow(b"temper-host").unwrap().as_ref(),
        b"on:\n  push:\n    branches: [main]\n  pull_request:\njobs:\n  check:\n    runs-on: temper-host\n    steps:\n      - run: echo \"temper's starter check passed; replace this step with your own checks\"\n"
    );
    assert_eq!(crate::STARTER_PATH, b".forgejo/workflows/ci.yml");
}
