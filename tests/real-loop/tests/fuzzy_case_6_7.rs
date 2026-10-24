//! Reference case 6.7: deadline admission, escalation, split and release.
use temper_real_loop_world::{
    Exit,
    case::{self, oracle::Observed},
    run_cluster_forge,
};

#[test]
fn a_sub_owner_past_its_deadline_is_held_at_its_next_run_and_escalated_through_its_owner_to_the_director_who_splits_it_by_amendment_and_release_without_a_grace_for_its_delegates()
 {
    // A1 parks before the cap; jig's B1 does not hold running tasks. a5
    // finishes its one running attempt: a grace for later admissions is
    // 6.8's conflict. A held A1 writes no escalation words; the split is
    // instructions amended by the director, not a new sub-owner (6.8).
    // Four hours stand for CAP_SPAN from script construction. F11:
    // delegate accepts only an absolute wall (temper-engine-smith/src/nested.rs,
    // 206-209), while jig at 48b18f9 has Deadline::Span (boundary.rs:857).
    // The timer is deadline + 1 ms: jig's strict wall > deadline (48b18f9,
    // jig-domain-authority/src/check.rs:524). A's and a5's delegations
    // carry the same constructed deadline; A's own deadline stays later.
    // Singleton batches and input cue rules tolerate parked extra runs.
    // A's pass and the director's amendment are held for the checkpoint
    // reads; amendment then release are separate calls.
    // Earlier sections stay unchanged; batch_member, batch_held and the
    // shared tool, delivery and attempt checks are reused by calls.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_7(),
        case::scripts::step_6_7,
        case::scripts::step_6_7_rules,
        case::acts::step_6_7,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(
            called.exit,
            Some(Exit::Code(0)),
            "call {at}: {}\n{}",
            String::from_utf8_lossy(&called.stderr),
            String::from_utf8_lossy(&called.stdout)
        );
    }
    let observed = Observed::from_ran(&ran, case::acts::READS_6_7);
    for (read, doc) in &observed.documents {
        eprintln!("{read:?}: {doc}");
    }
    for told in &observed.told {
        let messages: Vec<_> = told
            .history
            .iter()
            .flat_map(|m| &m.parts)
            .filter_map(|p| match p {
                skein_fake_llm_domain::api::Part::ToolOutput { output, .. } => Some(String::from_utf8_lossy(output)),
                _ => None,
            })
            .collect();
        eprintln!("{:?} outputs: {messages:?}", told.agent);
    }
    eprintln!("CAP opening {:?} ns", case::oracle::cap_opening_nanos(&observed));
    case::oracle::assert_met(&case::oracle::step_6_7(&observed));
}

#[test]
fn debug_cap_with_requester_timer() {
    use temper_real_loop_world::{argv,calls::{self,D1,call,number,sign_in,start,join}};
    use skein_fake_llm_domain::api::{Line,Turn,Finish,Script,Rule};
    use serde_json::json;
    const HOLD:&[u8]=b"cap-probe-6-7";
    let fixture=temper_real_loop_world::forge::Fixture {
        configure:|value,agent,forge|{(case::fixture_6_7().configure)(value,agent,forge);value["engine"]["limits"]["overrides"]["tasks.funders"]=5.into();},
        ..case::fixture_6_7()
    };
    let ran=run_cluster_forge(case::PAUSE_WORKERS,&[],fixture,|forge|{
        let mut original=case::scripts::step_6_7(forge).into_vec();
        let Line::Call{arguments,..}=&original[1].turns[0].lines[0] else {panic!("delegate")};
        let delegated:serde_json::Value=serde_json::from_slice(arguments).expect("delegate");
        let deadline=delegated["batch"][0]["authority"]["budget"]["deadline"].as_u64().expect("deadline");
        original[1].turns[1].lines=original[1].turns[1].lines.iter().filter(|l|matches!(l,Line::Call{name,..} if name.as_ref()==b"decide_escalation")).cloned().collect();
        original[0].turns[1].lines=original[0].turns[1].lines.iter().filter(|l|matches!(l,Line::Call{name,..} if name.as_ref()==b"amend")).cloned().collect();
        original[0].turns[2].lines=original[0].turns[2].lines.iter().filter(|l|matches!(l,Line::Call{name,..} if name.as_ref()==b"release")).cloned().collect();
        let mut held=original[3].turns[0].clone();
        for line in &mut held.lines {if let Line::Pieces{pieces}=line {for piece in pieces {if piece.hold.is_some(){piece.hold=Some(HOLD.into());}}}}
        let finish=Turn{lines:Box::new([original[3].turns[0].lines[1].clone()]),finish:Finish::ToolCalls,tokens:20};
        original.push(Script{cue:b"@case_cap_probe".as_slice().into(),turns:Box::new([
            Turn{lines:Box::new([
                Line::Call{name:b"subscribe".as_slice().into(),arguments:serde_json::to_vec(&json!({"kind":"timer","at":deadline+case::CAP_TIMER_MARGIN})).expect("timer").into()},
                Line::Call{name:b"wait".as_slice().into(),arguments:b"{}".as_slice().into()},
            ]),finish:Finish::ToolCalls,tokens:20},held,finish,
        ])});
        original.into()
    },||{
        let mut original=case::scripts::step_6_7_rules().into_vec();
        original.push([
            ("Finish was not accepted because messages arrived.",2),(": ",1),("Begin the work",0),("",1)
        ].map(|(cue,turn)|Rule{cue:cue.as_bytes().into(),turn}).into());original.into()
    },||vec![
        sign_in(D1),
        call(D1,argv!["chat","--project",number(case::PROJECT),"--key","cap",case::CAP_PERSON]),
        call(D1,argv!["wait","task","1","--until","replied"]),
        call(D1,argv!["wait","task","2","--until","replied"]),
        call(D1,argv!["wait","task","3","--until","replied"]),
        calls::until(D1,argv!["task","4","--json"],b"\"work\":\"writing\"",b"\"task\""),
        call(D1,argv!["chat","--project",number(case::PROJECT),"--key","probe","@case_cap_probe"]),
        call(D1,argv!["wait","task","5","--until","phase:idle"]),
        start(D1,argv!["wait","task","1","--until","phase:running"]),
        calls::until(D1,argv!["task","5","--json"],b"\"work\":\"writing\"",b"\"task\""),
        join(8),
        call(D1,argv!["wait","task","1","--until","phase:idle"]),
        call(D1,argv!["task","3","--activity","32","--json"]),
        call(D1,argv!["task","2","--activity","32","--json"]),
        calls::release_hold(HOLD),calls::release_hold(case::CAP_A5_HOLD),
        call(D1,argv!["wait","task","5","--until","ended"]),
    ]);
    for (at,c) in ran.calls().iter().enumerate(){if at>=8{eprintln!("PROBE {at} {:?}: {} {}",c.exit,String::from_utf8_lossy(&c.stdout),String::from_utf8_lossy(&c.stderr));}}
    let o=Observed::from_ran(&ran,&[]);
    for t in &o.told {for m in &t.history {for p in &m.parts {
        if let skein_fake_llm_domain::api::Part::Text{text} | skein_fake_llm_domain::api::Part::ToolOutput{output:text,..}=p {
            let text=String::from_utf8_lossy(text);
            if text.contains("Held task")||text.contains("Refused")||text.contains("Escalation")||text.contains("denied")||text.contains("Authority") {
                eprintln!("QUERY {:?}: {text}",t.agent);
            }
        }
    }}}
}
