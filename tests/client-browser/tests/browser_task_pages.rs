//! Real task addresses, a fresh open and the lineage bar, on a procedure's plan (client/README.md,
//! 5.10), through `temper box`'s demo and its own fakes. The terminal-side proofs of the same pages
//! are tests/client's `tests/task.rs`, `b2::roadmap_b2_through_the_task_pages` (including a fresh
//! client at a delegate's address), and the agent's page golden
//! `crates/temper-client-shell-terminal/src/tests/golden/agent_task.txt`. `crates/temper/tests/qa_delegation.rs`
//! opens Task 2 in a box too, but has been broken on main since a2d78e2a4 (its "Task 2" link prints
//! an outward mark the typist does not match), so it is cited, not relied on. The browser alone
//! proves the addresses and the first read after a full page load at `/tasks/3`.
use skein_fake_person::{Find, Step as PersonStep, Target};
use skein_fake_person_browser::{Harness, Outcome, Script, Step};
use skein_ui::Role;
use temper_client_browser_world::{Tier, WITHIN};

#[test]
fn a_procedures_plan_reaches_its_agent_and_back_by_address() {
    let tier = Tier::demo("procedure-plan-agent-addresses");
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let press = |find| Step::Person(PersonStep::Press { find });
    let heading = |level, name: &[u8]| Find::exactly_named(Role::Heading { level }, name);
    let task = |number: &[u8]| Find::exactly_named(Role::Link, number);
    let change = b"Export widget's table as CSV";
    let agent = "T3 · Export widget's table as CSV".as_bytes();
    let plan_agent = || {
        task(b"Task 3")
            .within(Box::new([Target::ExactRole { role: Role::TreeItem, name: b"Task 3".as_slice().into() }]))
    };
    let mut story = Script::new(Box::new([
        see(Find::exactly_named(Role::Button, b"Try the playground")),
        press(Find::exactly_named(Role::Button, b"Try the playground")),
        // The box starts the demo's group only when asked: wait for its start to be answered.
        see(Find::exactly_named(Role::Status, b"Ready")),
        Step::Person(PersonStep::Gone { find: Find::exactly_named(Role::Status, b"Starting"), within: WITHIN }),
        see(tier.states().live),
        see(heading(1, b"What shall we change?")),
        see(Find::exactly_named(Role::Button, b"Add CSV export to widget")),
        press(Find::exactly_named(Role::Button, b"Add CSV export to widget")),
        // 2: qa_delegation::running_card_names_the_change_and_exposes_its_steps opens this procedure
        // (broken on main since a2d78e2a4, see the module's doc).
        // The card settles at its call once the turn's words show (client/README.md, 5.7).
        // A person reads the reply, then follows its card.
        see(Find::text("I’ll delegate this change in playground/widget. It will push a branch, open a pull request, check CI and land into main on Playground forge.".as_bytes())
            .within(Box::new([Target::ExactRole { role: Role::Log, name: b"Conversation".as_slice().into() }]))),
        see(Find::exactly_named(Role::Region, b"Task 2")),
        see(task(b"Task 2")
            .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Task 2".as_slice().into() }]))),
        press(
            task(b"Task 2")
                .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Task 2".as_slice().into() }])),
        ),
        Step::Address(b"/tasks/2".as_slice().into()),
        see(heading(1, change)),
        see(heading(2, b"Its tasks")),
        // 3: task::a_delegates_page_shows_its_brief_run_and_report follows a plan to its agent.
        see(Find::exactly_named(Role::TreeItem, b"Task 3")),
        see(plan_agent()),
        press(plan_agent()),
        Step::Address(b"/tasks/3".as_slice().into()),
        see(heading(1, agent)),
        see(Find::exactly_named(Role::Region, b"Task brief")),
        // 4: task::a_delegates_page_shows_its_brief_run_and_report and
        // b2::roadmap_b2_through_the_task_pages return through the lineage.
        press(task(b"Task 2")),
        Step::Address(b"/tasks/2".as_slice().into()),
        see(heading(2, b"Its tasks")),
        // 5: b2::roadmap_b2_through_the_task_pages starts a fresh client at a delegate's address.
        Step::Person(PersonStep::Go { address: b"/tasks/3".as_slice().into() }),
        see(heading(1, agent)),
        see(Find::exactly_named(Role::Region, b"Task brief")),
        Step::Address(b"/tasks/3".as_slice().into()),
    ]));
    let mut harness = Harness::new(tier.setup());
    assert_eq!(harness.run(&mut [&mut story]), Outcome::Done);
}
