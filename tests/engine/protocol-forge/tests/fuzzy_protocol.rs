use skein_fake_forge_protocol::Faults;
use skein_forge_machine_world::joint::REPO;
use temper_engine_domain_forge as top;
use temper_engine_protocol_forge_world::{fixtures, protocol::Protocol};
#[test]
fn stories_through_cut_streams_answer_each_call_once() {
    let faults = Faults { reset: 20, drop: 20, server_error: 20 };
    for seed in 0..16 {
        let mut world = Protocol::cutting(seed, 100, faults);
        world.adopt();
        world.event(top::Event::Project {
            closing_generation: None,
            entry: 1,
            repository: REPO,
            view: fixtures::goal_view(),
        });
        world.event(top::Event::Committed { entry: 1 });
        world.run_for(seed % 3);
        world.restart();
        world.run_for(60);
        assert_eq!(world.issues_made(), 1, "seed {seed}: the keyed issue made once");
        assert!(world.stored().get(&top::Key::Entry(1)).is_none(), "seed {seed}: the entry settled");
        world.finish();
        assert!(world.token_kept(), "seed {seed}: the API token stayed below the top");
        assert!(world.tally().total - world.start_reads() <= world.costs(), "seed {seed}: no request uncounted");
    }
}
