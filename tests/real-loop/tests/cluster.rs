//! The cluster's two one-slot workers each run a chat to its end.
use skein_world::domain::heap::Counting;
#[global_allocator]
static HEAP: Counting = Counting;
use temper_real_loop_world::{
    End, Exit, argv,
    calls::{D1, D2, file, join, same_party, start, task, watching},
    fixtures, llm, run_cluster, scripts,
};
const WORKERS: &[&str] = &["w1", "w2"];
const CHATS: usize = 2;
const RUNS_PER_WORKER: usize = 1;
const LINKS_PER_WORKER: usize = 1;
const FIRST_SIGN_IN: usize = 0;
const FIRST_CHAT: usize = 2;
const SECOND_CHAT: usize = 3;
const FIRST_REPLY: usize = 4;
const SECOND_REPLY: usize = 5;
const FIRST_WAIT: usize = 6;
const FIRST_CLOSE: usize = 7;
const SECOND_WAIT: usize = 8;
const SECOND_CLOSE: usize = 9;
#[test]
fn two_workers_each_run_a_chat_to_its_end() {
    let cluster = run_cluster(
        WORKERS,
        || {
            use skein_fake_llm_domain::api::{Line, Piece};
            let mut scripts = scripts::chat_scripts(CHATS);
            for script in &mut scripts {
                *script.turns.first_mut().expect("chat turn").lines.first_mut().expect("chat text") = Line::Pieces {
                    pieces: Box::new([Piece {
                        text: scripts::REPLY.into(),
                        hold: Some(temper_real_loop_world::cluster::PLACED.into()),
                    }]),
                };
            }
            llm::scripted(scripts)
        },
        || {
            vec![
                start(D1, argv!["sign-in", file("link")]),
                same_party(D2),
                join(FIRST_SIGN_IN),
                start(D1, argv!["chat", "start", "--project", fixtures::PROJECT, "--key", "first", scripts::WORDS]),
                start(D2, argv!["chat", "start", "--project", fixtures::PROJECT, "--key", "second", scripts::WORDS]),
                join(FIRST_CHAT),
                join(SECOND_CHAT),
                start(D1, argv!["task", "show", task(FIRST_CHAT), "--until", "replied"]),
                start(D2, argv!["task", "show", task(SECOND_CHAT), "--until", "replied"]),
                join(FIRST_REPLY),
                join(SECOND_REPLY),
                start(D1, argv!["task", "show", task(FIRST_CHAT), "--until", "ended"]),
                watching(FIRST_WAIT),
                start(D1, argv!["chat", "close", task(FIRST_CHAT), "--key", "close-first"]),
                join(FIRST_CLOSE),
                join(FIRST_WAIT),
                start(D2, argv!["task", "show", task(SECOND_CHAT), "--until", "ended"]),
                watching(SECOND_WAIT),
                start(D2, argv!["chat", "close", task(SECOND_CHAT), "--key", "close-second"]),
                join(SECOND_CLOSE),
                join(SECOND_WAIT),
            ]
        },
    );
    for (at, called) in cluster.calls().iter().enumerate() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "call {at}: {}", String::from_utf8_lossy(&called.stderr));
    }
    assert_eq!(cluster.served(), Some(&End::Drained));
    let mut tasks = [FIRST_CHAT, SECOND_CHAT].map(|call| cluster.number(call));
    let mut placed = Vec::new();
    for name in WORKERS {
        let worker = cluster.worker(name);
        assert_eq!(worker.linked(), LINKS_PER_WORKER, "{name} linked once");
        assert_eq!(worker.end(), Some(&End::Drained), "{name} drained");
        assert_eq!(worker.ran().len(), RUNS_PER_WORKER, "{name} ran one chat");
        placed.extend_from_slice(worker.ran());
    }
    placed.sort_unstable();
    tasks.sort_unstable();
    assert_eq!(placed, tasks, "each worker first ran a different chat");
}
