use super::*;
/// The chosen main differs from both the template main and its sub-agent.
pub(crate) const MODEL_NAME: &[u8] = b"chosen-model";
pub(crate) const EFFORTLESS_MODEL_NAME: &[u8] = b"plain-model";
pub(crate) const EFFORT_NAME: &[u8] = b"high";
pub(crate) const MODEL_CHAT: &[&str] = &[
    "chat",
    "start",
    "hello",
    "--project",
    "shop",
    "--model",
    "chosen-model",
    "--effort",
    "high",
    "--key",
    "model-chat",
];
pub(crate) const EFFORTLESS_CHAT: &[&str] =
    &["chat", "start", "hello", "--project", "shop", "--model", "plain-model", "--key", "plain-chat"];
pub(crate) fn model_effort(model: &[u8], effort: Option<&[u8]>) -> c::ModelEffort {
    let model = act_text(model);
    let Some(effort) = effort else { return c::ModelEffort { model, effort: None } };
    c::ModelEffort { model, effort: Some(act_text(effort)) }
}
pub(crate) fn prior_choice() -> c::LlmChoice {
    let mut subagents = List::with_capacity(1);
    subagents.push(model_effort(b"small-model", Some(b"low"))).expect("one sub-agent choice");
    c::LlmChoice { account: Some(43), main: model_effort(b"template-model", Some(b"low")), subagents }
}
pub(crate) fn choice_me(budget: u64) -> c::Me {
    let mut me = chat_me(budget);
    let project = me.projects.get_mut(0).expect("project");
    project.templates.get_mut(0).expect("chat template").llm = Some(prior_choice());
    project.models = List::with_capacity(2);
    let mut efforts = List::with_capacity(1);
    efforts.push(act_text(EFFORT_NAME)).expect("one effort");
    project
        .models
        .push(c::MeModel {
            prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
            description: None,
            name: act_text(MODEL_NAME),
            provider: None,
            window: 8192,
            output: 1024,
            efforts,
        })
        .expect("one selectable model");
    project
        .models
        .push(c::MeModel {
            prices: c::ModelPrices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1000 },
            description: None,
            name: act_text(EFFORTLESS_MODEL_NAME),
            provider: None,
            window: 8192,
            output: 1024,
            efforts: List::with_capacity(0),
        })
        .expect("effortless model");
    me
}
pub(crate) fn choice_root(me: c::Me) -> Harness {
    let mut root = started(List::with_capacity(EMPTY));
    root.deliver(c::Delivery::Me(Box::new(me)));
    root
}
pub(crate) fn chat_task(root: &Harness, call: u32) -> &c::NewTask {
    let c::Ask::Create { tasks, .. } = act_ask(root, call) else { panic!("chat creates") };
    tasks.get(0).expect("one chat task")
}

/// The task document matches the caller's shown revision.
pub(crate) const CHOICE_SPEC: u64 = 7;
pub(crate) const MODEL_AMEND: &[&str] = &[
    "task",
    "amend",
    "41",
    "--rev",
    "7",
    "--model",
    "chosen-model",
    "--effort",
    "high",
    "because",
    "--key",
    "model-amend",
];
pub(crate) fn choice_task(admission: c::Admission) -> c::Task {
    let mut task = offered(&[c::ActKind::Amend]);
    task.acts.get_mut(0).expect("amend offer").admission = admission;
    task.spec_revision = c::Revision { value: CHOICE_SPEC };
    task.llm = Some(prior_choice());
    task
}
pub(crate) fn send_choice_amend(argv: &[&str], task: c::Task) -> (Harness, u32) {
    let mut root = choice_root(choice_me(CHAT_BUDGET));
    let call = root.submit(argv);
    assert_eq!(root.reads.get(&call).expect("task read").1, c::Read::Task { task: TASK });
    root.serve(call, task_result(task));
    (root, call)
}
pub(crate) fn model_change(effort: Option<&[u8]>) -> Box<c::Amendment> {
    let mut llm = prior_choice();
    llm.main = model_effort(MODEL_NAME, effort);
    Box::new(c::Amendment {
        addressed: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        instructions: None,
        tracked: None,
        llm: Some(llm),
    })
}

pub(crate) const EFFORTLESS_AMEND: &[&str] =
    &["task", "amend", "41", "--rev", "7", "--model", "plain-model", "because", "--key", "plain-amend"];
