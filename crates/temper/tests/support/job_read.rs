//! A check's returned name is the input of the next sitting's real job-log read.
use super::{Scratch, forgejo::Throwaway, person};
use serde_json::json;
use skein_fake_llm_domain::api::{Part, Script};

fn output(model: &super::forge_land::Seen, cue: &str) -> String {
    model
        .requests
        .iter()
        .flat_map(|query| &query.messages)
        .flat_map(|message| &message.parts)
        .find_map(|part| match part {
            Part::ToolOutput { output, .. } => {
                let text = std::str::from_utf8(output).expect("tool text");
                text.contains(cue).then(|| text.to_owned())
            }
            Part::Opaque { .. } | Part::Text { .. } | Part::ToolCall { .. } => None,
        })
        .unwrap_or_else(|| panic!("model received no {cue} tool answer"))
}

pub fn prove() {
    let scratch = Scratch::new("check-names-readable-job");
    let forgejo = Throwaway::start_with_job_sleep(&scratch, 0);
    let head = forgejo.checked_head();
    super::configuration(scratch.path(), &forgejo.forge());
    let mut hello = super::chat_script(super::ANSWER);
    hello.cue = b"Hello".as_slice().into();
    let checks = Script {
        cue: b"@read-checks".as_slice().into(),
        turns: Box::new([
            super::first_repository::call(b"read_checks", &json!({"repository":"acme/widget", "commit":head})),
            super::first_repository::call(b"finish", &json!({"form":"report", "text":"Read the check.", "fields":{}})),
        ]),
    };
    let (seen, model) = super::run_observed(
        scratch.path(),
        person::adopt_and_chat(b"@read-checks Read CI", b"Read the check."),
        super::fake_llm(Box::new([hello, checks])),
    );
    assert!(seen.finished, "{}", String::from_utf8_lossy(&seen.stderr));
    let checks = output(&model.expect("checks model"), "read_job head ");
    assert!(checks.contains(": passed"), "a passed Actions check: {checks}");
    assert!(checks.contains("/jobs/0"), "the link carries index zero: {checks}");
    let name = checks.lines().find_map(|line| line.strip_prefix("read_job head ")).expect("readable name");
    let parts: Vec<_> = name.split_whitespace().collect();
    let [named_head, "run", run, "job", job, "attempt", attempt] = parts.as_slice() else { panic!("job name: {name}") };
    assert_eq!(*named_head, head);
    assert_ne!(*job, "0", "the API job id differs from index zero");
    // The second sitting's tool input is built solely from the first read's answer.
    let arguments = json!({"repository":"acme/widget", "head":named_head,
        "run":run.parse::<u64>().expect("run id"), "job":job.parse::<u64>().expect("job id"),
        "attempt":attempt.parse::<u32>().expect("attempt"), "max_bytes":4096});
    let script = Script {
        cue: b"@read-log".as_slice().into(),
        turns: Box::new([
            super::first_repository::call(b"read_job", &arguments),
            super::first_repository::call(b"finish", &json!({"form":"report", "text":"Read the log.", "fields":{}})),
        ]),
    };
    let (seen, model) = super::run_observed(
        scratch.path(),
        person::answered_chat(b"@read-log Read that job", b"Read the log."),
        super::fake_llm(Box::new([script])),
    );
    assert!(seen.finished, "{}", String::from_utf8_lossy(&seen.stderr));
    let log = output(&model.expect("log model"), &format!("Job {job} run {run}"));
    assert!(log.contains("Job succeeded"), "the named job's real runner log: {log}");
    forgejo.stop();
    scratch.remove();
}
