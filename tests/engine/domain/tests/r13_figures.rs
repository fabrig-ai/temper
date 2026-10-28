#[test]
fn restore_repair_figures() {
        let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
        let own = temper_service::limits::profile(temper_service::limits::Profile::Small);
        let limits = temper_service::limits::engine(&jig, &own).expect("small profile");
        println!("R13 temper root={:?} forge={:?} issue_size={} root_size={} forge_size={}",
            temper_engine_domain::worst_case(&limits),
            temper_engine_domain_forge::worst_case(&limits.forge),
            core::mem::size_of::<temper_engine_domain_forge::IssueRow>(),
            core::mem::size_of::<temper_engine_domain::Domain>(),
            core::mem::size_of::<temper_engine_domain_forge::Domain>());
}
