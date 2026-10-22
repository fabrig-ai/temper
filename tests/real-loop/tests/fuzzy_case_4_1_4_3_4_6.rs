//! The opener, the timer check, and a restarted workstation on the same three workers.
use temper_real_loop_world::{
    End, Exit,
    case::{
        self, acts,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

const WORKERS: &[&str] = &["ws", "s1", "s2"];

#[test]
fn the_opener_the_two_hour_check_and_a_restarted_workstation_run_on_three_workers() {
    // Leaves out survey, instructions/pool, host kinds, Markdown CI's fast path and goal issue; pools/load/queue/disk, moving B and the plane: testing.md 5.3's 4.1 and 4.3 rows.
    let cluster = run_cluster_forge(
        WORKERS,
        &["s2"],
        case::fixture_4_6(),
        case::scripts::step_4_1_4_3_4_6,
        acts::step_4_1_4_3_4_6,
    );
    for (at, called) in cluster.calls().iter().enumerate() {
        assert_eq!(called.exit, Some(Exit::Code(0)), "call {at}: {}", String::from_utf8_lossy(&called.stderr));
    }
    let observed = Observed::from_ran(&cluster, &acts::reads_4_1_4_3_4_6());
    let mut misses = oracle::step_4_1(&observed);
    misses.extend(oracle::step_4_3(&observed));
    oracle::assert_met(&misses);
    let queued = &observed.documents[&acts::Read::BenchmarkWaiting];
    assert_eq!(queued.pointer("/task/chip/phase").and_then(serde_json::Value::as_str), Some("due"));
    for name in WORKERS {
        let worker = cluster.worker(name);
        assert_eq!(worker.end(), Some(&End::Drained), "{name} drained");
    }
    assert_eq!(cluster.worker("ws").ran(), &[case::DIRECTOR, case::CHECK, case::HANG], "ws ran the three chats");
    let s2: Vec<_> =
        cluster.worker("s2").ran().iter().copied().filter(|task| ![case::HANG, case::PHASE].contains(task)).collect();
    assert_eq!(s2, &[case::BENCHMARK], "s2 ran only the benchmark before the third part");
    // A parks between its wakes, so its activations' count is the engine's; the tasks are not.
    let s1 = cluster.worker("s1").ran();
    let opener = [case::A, case::CODER, case::REVIEW];
    let before_phase: Vec<_> = s1.iter().copied().filter(|task| ![case::HANG, case::PHASE].contains(task)).collect();
    let (last, earlier) = before_phase.split_last().expect("s1 ran tasks");
    assert_eq!(*last, case::B, "s1 ran B after the opener");
    assert!(earlier.iter().all(|task| opener.contains(task)), "s1 ran the opener's A and delegates: {s1:?}");
    assert!(opener.iter().all(|task| s1.contains(task)), "s1 ran each of A, its coder and reviewer: {s1:?}");
    let added = cluster.worker("s2").started_at().expect("late worker started");
    for name in ["ws", "s1"] {
        assert!(
            cluster.worker(name).assigned_at().expect("initial worker assigned") <= added,
            "{name} assigned before s2 started"
        );
    }
    let lives = cluster.worker("ws").lives();
    assert!(lives.iter().all(|life| life.drained), "both ws lives drained");
    assert!(lives.iter().all(|life| life.linked > 0), "both ws lives linked");
    assert!(lives.last().expect("restarted ws").ran.is_empty(), "the restarted ws ran no task");
    assert!(
        ["s1", "s2"].iter().any(|name| cluster.worker(name).ran().contains(&case::HANG)),
        "director resumed elsewhere"
    );
    third_part(&cluster, &observed);
    assert!(cluster.killed().is_empty(), "cancellation was cooperative");
}

const DIRECTOR_ATTEMPTS: usize = 2;
const WORKSTATION_LIVES: usize = 2;
const ONE: usize = 1;

fn third_part(cluster: &temper_real_loop_world::Cluster, observed: &Observed) {
    use acts::Read;
    let lives = cluster.worker("ws").lives();
    assert_eq!(lives.len(), WORKSTATION_LIVES, "ws kept its root across two lives");
    assert_eq!(cluster.worker("ws").linked(), WORKSTATION_LIVES, "ws linked for each life");
    for life in lives {
        assert_eq!(life.linked, ONE, "each life linked once");
    }
    let other: Vec<_> =
        ["s1", "s2"].iter().flat_map(|name| cluster.worker(name).ran()).filter(|task| **task == case::HANG).collect();
    assert_eq!(other.len(), ONE, "the director had one activation away from ws");
    for (read, task, report) in
        [(Read::HangEnded, case::HANG, case::HANG_REPORT), (Read::PhaseEnded, case::PHASE, case::PHASE_REPORT)]
    {
        let doc = &observed.documents[&read]["task"];
        assert_eq!(doc["chip"]["task"], task);
        assert_eq!(doc["chip"]["phase"], "ended");
        assert_eq!(doc.pointer("/result/value/report/bytes").and_then(serde_json::Value::as_str), Some(report));
    }
    let mut rows = Vec::new();
    for read in [Read::HangActivity, Read::HangActivityOlder, Read::HangActivityFirst] {
        let activity = &observed.documents[&read];
        assert_eq!(activity["cut"], false);
        rows.extend(activity["rows"].as_array().expect("activity rows"));
    }
    assert!(
        observed.documents[&Read::HangActivityFirst]["before"].is_null(),
        "the activity includes the first attempt"
    );
    assert!(
        rows.windows(2).all(|pair| pair[0]["position"].as_u64() > pair[1]["position"].as_u64()),
        "pages retain the journal's order without overlap"
    );
    let mut open = None;
    let mut attempts = Vec::new();
    let mut endings = Vec::new();
    for row in rows.iter().rev() {
        let line = &row["event"]["owner"]["source"]["line"];
        if let Some(began) = line.get("run_began") {
            assert!(open.is_none(), "never two open attempts in the journal");
            let attempt = began["attempt"].as_u64().expect("begun attempt");
            open = Some(attempt);
            attempts.push(attempt);
        }
        if let Some(ended) = line.get("run_ended") {
            assert_eq!(open.take(), ended["attempt"].as_u64(), "end the current attempt before beginning the next");
            endings.push(ended);
        }
    }
    assert!(open.is_none(), "every attempt ended");
    assert_eq!(attempts.len(), DIRECTOR_ATTEMPTS, "one initial attempt and one retry");
    assert_eq!(endings.len(), DIRECTOR_ATTEMPTS, "both attempts have a journal ending");
    assert_eq!(
        endings.first().expect("first ending").pointer("/end/failed/class").and_then(serde_json::Value::as_str),
        Some("agent")
    );
    assert_eq!(endings.last().expect("last ending")["end"], "finished");
    let read = &observed.documents[&Read::HangQuestionRead];
    assert!(
        read["rows"].as_array().expect("read barrier activity").iter().any(|row| {
            row.pointer("/event/owner/source/line/turn/read").and_then(serde_json::Value::as_u64)
                == Some(case::PHASE_QUESTION_NUMBER)
        }),
        "the question read was committed before stopping ws"
    );
    assert_eq!(
        observed.documents[&Read::HangWaiting]
            .pointer("/task/chip/details/owner/details/observation/work")
            .and_then(serde_json::Value::as_str),
        Some("waiting"),
        "the director was waiting in its run before the worker stopped"
    );
    told_once(cluster);
    costs(lives, &rows);
}

// Measurements only: the scenario's barriers use events and journal positions.
fn costs(lives: &[temper_real_loop_world::service::Life], rows: &[&serde_json::Value]) {
    let [old, new] = lives else { panic!("two workstation lives") };
    let millis = |nanos| std::time::Duration::from_nanos(nanos).as_secs_f64() * 1000.0;
    let began: Vec<_> =
        rows.iter().rev().filter(|row| row.pointer("/event/owner/source/line/run_began").is_some()).collect();
    let ended: Vec<_> =
        rows.iter().rev().filter(|row| row.pointer("/event/owner/source/line/run_ended").is_some()).collect();
    let first_begin = began.first().expect("initial beginning")["at"].as_u64().expect("wall");
    let first_end = ended.first().expect("initial ending")["at"].as_u64().expect("wall");
    let next_begin = began.last().expect("retry beginning")["at"].as_u64().expect("wall");
    let final_end = ended.last().expect("retry ending")["at"].as_u64().expect("wall");
    eprintln!(
        "third chat {:.1} ms; ws ending {:.1} ms; journal retry gap {:.1} ms; ws restart/link {:.1} ms",
        millis(final_end.saturating_sub(first_begin)),
        millis(old.ended.as_nanos().saturating_sub(old.stop_requested.expect("stop requested").as_nanos())),
        millis(next_begin.saturating_sub(first_end)),
        millis(new.linked_at.expect("restarted link").as_nanos().saturating_sub(new.began.as_nanos()))
    );
}

fn told_once(cluster: &temper_real_loop_world::Cluster) {
    use skein_fake_llm_domain::api::{Part, Role};
    use skein_fake_peers::llm::Observation::Query;
    use temper_real_loop_world::calls::occurrences;
    let mut answers = std::collections::BTreeSet::new();
    let mut received = 0;
    let mut resumed = false;
    for event in cluster.llm() {
        let Query { query, .. } = event else { continue };
        let hang_at =
            query.system.windows(case::HANG_PERSON.len()).position(|part| part == case::HANG_PERSON.as_bytes());
        let phase_at =
            query.system.windows(case::PHASE_SPEC.len()).position(|part| part == case::PHASE_SPEC.as_bytes());
        // A resumed owner's brief can also name its director. Use the fake's
        // earliest-cue rule, rather than any occurrence in the system text.
        let director = hang_at.is_some_and(|hang| phase_at.is_none_or(|phase| hang < phase));
        let owner = phase_at.is_some_and(|phase| hang_at.is_none_or(|hang| phase < hang));
        if director {
            let parts: Vec<_> = query.messages.iter().flat_map(|message| &message.parts).collect();
            for part in &parts {
                if let Part::ToolCall { id, name, arguments } = part
                    && name.as_ref() == b"message"
                {
                    let args: serde_json::Value = serde_json::from_slice(arguments).expect("message arguments");
                    if args["form"] == "answer" && args["question"] == case::PHASE_QUESTION_NUMBER {
                        assert_eq!(args["target"], case::PHASE);
                        assert_eq!(args["words"], case::PHASE_ANSWER);
                        answers.insert(id.as_ref().to_vec());
                    }
                }
            }
            let waits = parts
                .iter()
                .filter(|part| matches!(part, Part::ToolCall { name, .. } if name.as_ref() == b"wait"))
                .count();
            let texts: Vec<_> = query
                .messages
                .iter()
                .filter(|message| message.role == Role::User)
                .flat_map(|message| &message.parts)
                .filter_map(|part| {
                    if let Part::Text { text } | Part::ToolOutput { output: text, .. } = part {
                        Some(text.as_ref())
                    } else {
                        None
                    }
                })
                .collect();
            if waits >= DIRECTOR_ATTEMPTS
                && texts.iter().any(|text| occurrences(text, b"Begin the work your brief describes.") > 0)
            {
                assert_eq!(
                    texts.iter().map(|text| occurrences(text, case::PHASE_QUESTION.as_bytes())).sum::<usize>(),
                    ONE,
                    "the retry retains one copy of A's question"
                );
                resumed = true;
            }
        }
        if owner {
            received += query
                .messages
                .iter()
                .filter(|message| message.role == Role::User)
                .flat_map(|message| &message.parts)
                .filter_map(|part| {
                    if let Part::Text { text } | Part::ToolOutput { output: text, .. } = part {
                        Some(occurrences(text, case::PHASE_ANSWER.as_bytes()))
                    } else {
                        None
                    }
                })
                .sum::<usize>();
        }
    }
    assert!(resumed, "a director continuation retained the committed question");
    assert_eq!(answers.len(), ONE, "the director sent one answer call across the retry");
    assert_eq!(received, ONE, "A was told the answer once");
}
