//! An agent's task's lineage, brief, run and conversation (client/README.md, 5.9).
use crate::{
    frame::Bindings,
    look,
    words::{Times, Word, Words},
};
use alloc::boxed::Box;
use jig_client::{Latest, contract as c};
use skein_lib::bytes::copy_of;
use skein_ui::{Builder, Role, Run, State, phrases::Hole};
use temper_client_domain::{
    Domain,
    pages::{Act, task},
};

fn text(b: &mut Builder, role: Role, value: &[u8]) {
    b.open(role);
    b.name(value);
    b.text(Run::Plain, value);
    b.close();
}
pub(crate) fn main(
    b: &mut Builder,
    root: &Domain,
    words: &Words,
    bindings: &mut Bindings,
    rows: u32,
    times: &mut Times,
) {
    b.open(Role::Main);
    b.style(look::MAIN);
    b.style(look::CHAT);
    if let Some(page) = root.pages().task()
        && let Some(task) = page.document(root.client())
    {
        super::task::tree_status(b, page, words);
        let me = root.pages().me(root.client());
        lineage(b, task, words, bindings);
        b.open(Role::Slot);
        b.style(look::CHATBAR);
        b.open(Role::Slot);
        text(
            b,
            Role::Heading { level: 1 },
            &words.say(Word::ChatTitle, &[Hole::Number(task.chip.task), Hole::Name(copy_of(task.chip.title.bytes()))]),
        );
        super::task::state(b, task::header_state(task, me), words);
        b.close();
        b.open(Role::Slot);
        super::task::executor(b, task.chip.executor, words);
        text(b, Role::Paragraph, &words.say(Word::RowSpent, &[Hole::Number(task.budget.numbers.spent)]));
        if root.pages().confirmation().is_none()
            && match &page.amending {
                Some(open) => open.done,
                None => true,
            }
        {
            super::task::act(b, task, c::ActKind::Amend, Word::Amend, Act::Amend, words, bindings);
            super::task::act(b, task, c::ActKind::Stop, Word::Stop, Act::Stop, words, bindings);
        }
        b.close();
        b.close();
        b.open(Role::Slot);
        b.style(look::THREAD);
        super::task::amendment(b, root, words, bindings);
        brief(b, task, me, words, bindings);
        if let Some(run) = task::run(task) {
            run_heading(b, run, words);
        }
        super::task::controls(b, &task.chip, root, words, bindings, rows);
        if let jig_client::thread::window::Edge::More(_) = page.thread.lines.edges.after {
            b.open(Role::Button);
            b.name(&words.say(Word::Older, &[]));
            b.bind(bindings.insert(Act::Older));
            b.close();
        }
        super::chat::conversation(
            b,
            &page.thread,
            words,
            (rows, page.thread.limits.text),
            super::chat::speakers(task, me),
            super::chat::Drawn::Lines,
            bindings,
        );
        super::task::outcome(b, root, words);
        super::task::result(b, task, words, look::AGENT_RESULT);
        if let Some(document) = root.client().document(page.tree)
            && let Some(Latest::Jig(tree)) = document.latest
        {
            let delegates = match tree {
                c::Delivery::OwnerTree(tree) => !tree.rows.is_empty(),
                c::Delivery::Tree(tree) => tree.nodes.len() > 1,
                c::Delivery::OwnerProject(_)
                | c::Delivery::Me(_)
                | c::Delivery::Task(_)
                | c::Delivery::OwnerProposal(_)
                | c::Delivery::Chips(_)
                | c::Delivery::Inbox(_)
                | c::Delivery::OwnerInbox(_)
                | c::Delivery::Tasks(_)
                | c::Delivery::Run(_)
                | c::Delivery::RunState(_)
                | c::Delivery::Settings(_)
                | c::Delivery::Deployment(_)
                | c::Delivery::OwnerDeployment(_)
                | c::Delivery::OwnerAccounts(_) => false,
            };
            if delegates || !page.cards.is_empty() {
                super::task::plan(
                    b,
                    Some(tree),
                    root,
                    words,
                    bindings,
                    rows,
                    super::task::Plan { word: Word::Plan, ended: None, goal: None },
                );
            }
        }
        b.close();
        composer(b, root, page, task, words, bindings, times);
    }
    crate::confirm::main(b, root, words, bindings, rows);
    b.close();
}
fn brief(b: &mut Builder, task: &c::Task, me: Option<&c::Me>, words: &Words, bindings: &mut Bindings) {
    b.open(Role::Region);
    b.style(look::AGENT_BRIEF);
    b.name(&words.say(Word::TaskBrief, &[]));
    text(b, Role::Heading { level: 2 }, &words.say(Word::TaskBrief, &[]));
    skein_ui::markdown::read(task.spec.specification.words.bytes(), 3, b);
    super::task::requester(b, task.requester, me, words);
    super::task::dependencies(
        b,
        task.chip.task,
        &task.dependencies,
        task.dependency_facts.as_slice(),
        false,
        me,
        words,
        bindings,
    );
    b.close();
}
fn lineage(b: &mut Builder, task: &c::Task, words: &Words, bindings: &mut Bindings) {
    b.open(Role::Slot);
    b.style(look::AGENT_LINEAGE);
    if let Some(parent) = task.lineage_chips.as_slice().last() {
        b.open(Role::Slot);
        super::task::task_link(b, parent.task, words, bindings);
        text(b, Role::Paragraph, parent.title.bytes());
        b.close();
    } else if let c::OwnerRequester::Task(parent) = task.requester {
        b.open(Role::Slot);
        super::task::task_link(b, parent, words, bindings);
        b.close();
    }
    b.open(Role::Slot);
    text(b, Role::Paragraph, &words.say(Word::Task, &[Hole::Number(task.chip.task)]));
    text(b, Role::Paragraph, task.chip.title.bytes());
    b.close();
    b.close();
}
fn run_heading(b: &mut Builder, run: task::Run, words: &Words) {
    let (word, attempt, since) = match run {
        task::Run::Live { attempt, since } => (Word::RunLive, Some(attempt), since),
        task::Run::Waiting { attempt, since } => (Word::RunWaiting, Some(attempt), since),
        task::Run::Ended { attempt } => (Word::RunOver, Some(attempt), None),
        task::Run::NoRun => (Word::NoRun, None, None),
    };
    let heading = match attempt {
        Some(attempt) => words.say(word, &[Hole::Number(attempt)]),
        None => words.say(word, &[]),
    };
    b.open(Role::Slot);
    b.style(look::AGENT_RUN);
    text(b, Role::Heading { level: 2 }, &heading);
    if let Some(at) = since {
        text(b, Role::Paragraph, &words.run_started(at));
    }
    b.close();
}

fn composer(
    b: &mut Builder,
    root: &Domain,
    page: &task::Task,
    task: &c::Task,
    words: &Words,
    bindings: &mut Bindings,
    times: &mut Times,
) {
    let Some(admission) = jig_client::facts::offer(task.acts.as_slice(), c::ActKind::Message) else {
        crate::choice::composer(b, root, Some(task), words, bindings, times);
        return;
    };
    b.open(Role::Slot);
    b.style(look::COMPOSER_AREA);
    match admission {
        c::Admission::Allowed | c::Admission::Bounded { .. } => {
            b.open(Role::Paragraph);
            b.style(look::AGENT_STEER);
            words.text(b, Word::SteerNote, &[]);
            b.close();
            crate::choice::composer(b, root, Some(task), words, bindings, times);
            b.open(Role::Slot);
            b.style(look::ENTRY);
            b.open(Role::TextArea);
            b.name(&words.say(Word::Message, &[]));
            b.state(State::SubmitsOnEnter);
            b.style(look::COMPOSER);
            b.value(&page.words, temper_client_domain::pages::shown(root.pages()));
            b.bind(bindings.insert(Act::MessageWords { words: Box::default() }));
            b.close();
            b.open(Role::Paragraph);
            b.style(look::HINT);
            words.text(b, Word::ChatSendHint, &[]);
            b.close();
            b.open(Role::Button);
            b.name(&words.say(Word::Send, &[]));
            b.bind(bindings.insert(Act::Send));
            b.close();
            b.close();
        }
        // Why Message isn't offered is a note, as the steer note is, not an outcome (5.11).
        c::Admission::Refused(refusal) => {
            text(b, Role::Paragraph, &words.say(Word::Message, &[]));
            b.open(Role::Paragraph);
            b.style(look::AGENT_STEER);
            if let c::Refusal::NotWaiting = refusal {
                words.text(b, Word::MessageOnceRuns, &[]);
            } else {
                words.refusal(b, refusal, 0);
            }
            b.close();
            crate::choice::composer(b, root, Some(task), words, bindings, times);
        }
        c::Admission::Proposed { .. } => {
            super::task::act(b, task, c::ActKind::Message, Word::Message, Act::Send, words, bindings);
            crate::choice::composer(b, root, Some(task), words, bindings, times);
        }
    }
    b.close();
}
