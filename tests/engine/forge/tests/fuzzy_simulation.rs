use temper_engine_domain_forge as top;
use temper_engine_domain_forge_issues as issues;
use temper_engine_forge_world::{REPO, World};

#[test]
fn seeded_restarts_recover_one_keyed_issue_without_repeating_the_write() {
    for seed in 0..64 {
        let mut world = World::new(seed);
        world.adopt();
        world.event(top::Event::Project {
            closing_generation: None,
            entry: 1,
            repository: REPO,
            view: issues::GoalView {
                phase: issues::Phase::Waiting,
                goal: 42,
                repository: u64::from(REPO.repository),
                title: Box::from("Goal"),
                goal_text: Box::from("Plan"),
                plan: Box::new([]),
                milestones: Box::new([]),
                finished: None,
            },
        });
        world.event(top::Event::Committed { entry: 1 });
        world.run_for(seed % 3);
        world.restart();
        world.run_for(20);
        assert_eq!(world.writes(), 1, "seed {seed}");
        assert!(world.stored().get(&top::Key::Entry(1)).is_none(), "seed {seed}");
    }
}

#[test]
fn seeded_pull_watch_swaps_preserve_names_only_until_release() {
    for seed in 0..64 {
        let mut world = World::new(seed);
        world.adopt();
        world.produce(b"temper/change");
        let number = world.open_pull(b"temper/change", b"main");
        let branch = top::Name {
            forge: REPO.forge,
            repository: REPO.repository,
            what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"change"[..])])),
        };
        let pull = top::Name { forge: REPO.forge, repository: REPO.repository, what: top::What::Pull(number) };
        world.event(top::Event::Names { task: 51, resources: Box::new([branch.clone()]) });
        world.run_for(seed % 3);
        if seed.is_multiple_of(2) {
            world.event(top::Event::Unnamed { task: 51 });
        }
        world.event(top::Event::Rename { task: 51, resources: Box::new([branch, pull.clone()]) });
        world.run_for(seed % 5);
        if seed.is_multiple_of(2) {
            assert!(world.stored().get(&top::Key::Names(51)).is_none(), "seed {seed}: released before swap");
        } else {
            assert!(
                matches!(world.stored().get(&top::Key::Names(51)),
                Some(top::Stored::Names { resources, .. }) if resources.last() == Some(&pull)),
                "seed {seed}: swapped"
            );
            world.event(top::Event::Unnamed { task: 51 });
        }
        world.restart();
        world.event(top::Event::Rename { task: 51, resources: Box::new([pull]) });
        world.run_for(5);
        assert!(world.stored().get(&top::Key::Names(51)).is_none(), "seed {seed}: released through restart");
    }
}
