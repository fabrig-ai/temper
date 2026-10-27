use super::fixtures;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
#[test]
fn a_first_start_gives_each_project_one_period() {
    let mut service = fixtures::started_with_projects(&[(1, 100), (2, 73)]);
    let mut periods = skein_lib::List::<(u32, u64, u64)>::with_capacity(2);
    for _ in 0_u32..256 {
        let next = service.engine.as_mut().expect("engine").first_periods.iter().last().copied();
        fixtures::turn(&mut service);
        if let Some((project, budget)) = next
            && service.engine.as_ref().expect("engine").first_periods.iter().last().copied() != next
        {
            periods.push((project, 1, budget)).unwrap();
        }
        if !service.work_pending(skein_lib::Time::from_nanos(1)) {
            break;
        }
    }
    assert_eq!(periods.into_boxed().as_ref(), &[(2, 1, 73), (1, 1, 100)]);
    for (project, budget) in [(1, 100), (2, 73)] {
        let Some(core::Record::Tasks(tasks::Stored::Ledger(row))) = fixtures::get(
            &mut service,
            core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period { project, period: 1 })),
        ) else {
            panic!("period ledger")
        };
        assert_eq!(row.numbers.budget, budget, "project {project}");
        assert_eq!(
            row.period,
            Some(tasks::PeriodTimes {
                began: skein_lib::Wall::from_nanos(1),
                next: Some(skein_lib::Wall::from_nanos(1 + crate::stock::PERIOD_SPAN.as_nanos())),
            }),
            "project {project}"
        );
    }
}
#[test]
fn a_later_start_gives_no_new_period() {
    let store = {
        let old = fixtures::ready();
        old.engine.expect("engine").store
    };
    let mut service = fixtures::start_with_store(&[(1, 73)], Some(store));
    fixtures::restart(&mut service);
    assert!(service.engine.as_ref().expect("engine").first_periods.is_empty());
    let Some(core::Record::Tasks(tasks::Stored::Ledger(row))) = fixtures::get(
        &mut service,
        core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period { project: 1, period: 1 })),
    ) else {
        panic!("period ledger")
    };
    assert_eq!(row.numbers.budget, 100);
}
