//! The real engine's webhook listener meets skein's forge peer.
use skein_world::domain::heap::Counting;
#[global_allocator]
static HEAP: Counting = Counting;

#[test]
fn a_forge_delivery_is_accepted_by_the_engines_listener() {
    use skein_fake_peers::forge::Delivery;
    use temper_real_loop_world::{calls::advance, forge, run_forge, scripts};
    // No chat runs: one script supplies the peer's declared behaviour.
    const CHATS: usize = 1;
    let ran = run_forge(forge::fixture(), |_| scripts::chat_scripts(CHATS), || vec![advance()]);
    let seen = ran.forge();
    assert_eq!((seen.deliveries(), seen.responses()), (&[Delivery::Written][..], &[204_u16][..]), "{:?}", ran.served());
}

fn adopt(name: &'static str) -> temper_real_loop_world::Ran {
    use temper_real_loop_world::{
        argv,
        calls::{D1, call, sign_in},
        forge, run_forge, scripts,
    };
    // Adoption runs no agent; one unused script declares the model peer.
    const CHATS: usize = 1;
    let fixture = forge::Fixture {
        configure: |value, _, _| {
            // The two resource segments and seven options exceed the base fixture's 64 bytes.
            value["engine"]["limits"]["overrides"]["people.amendment_bytes"] = 512.into();
            // forge/host/owner/repository/branch/temper/root/marker-task.
            value["engine"]["limits"]["overrides"]["authority.segments"] = 8.into();
        },
        ..forge::fixture()
    };
    run_forge(
        fixture,
        |_| scripts::chat_scripts(CHATS),
        || {
            vec![
                sign_in(D1),
                call(
                    D1,
                    argv![
                        "repository",
                        "adopt",
                        "--project",
                        "1",
                        "--owner",
                        "acme",
                        "--name",
                        name,
                        "--landing",
                        "main",
                        "--key",
                        "adopt-app"
                    ],
                ),
            ]
        },
    )
}

#[test]
fn adopt_through_ctl_adopts_the_forges_repository() {
    use temper_real_loop_world::Exit;
    // The adoption follows the directory's sign-in call.
    const ADOPT: usize = 1;
    let ran = adopt("app");
    let adopted = &ran.calls()[ADOPT];
    assert_eq!(adopted.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&adopted.stderr));
    assert!(
        adopted.stdout.starts_with(b"Adopted acme/app on forge 1 as owned.\n"),
        "{}",
        String::from_utf8_lossy(&adopted.stdout)
    );
}
