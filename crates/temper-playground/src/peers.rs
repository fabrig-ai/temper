//! The fake byte peers assembled from the caller's scripts, forge and bounds.
use skein_fake_peers::{forge, llm};
use std::{cell::RefCell, net::SocketAddr, rc::Rc};

/// The model peer's configuration and scripts, supplied as values.
pub struct LlmStart {
    pub address: SocketAddr,
    pub limits: skein_fake_peers::Limits,
    pub provider: skein_fake_llm_protocol::provider::Config,
    pub credential: skein_llm::Credential,
    pub protocol: skein_fake_llm_protocol::provider::Limits,
    pub config: skein_fake_llm_domain::Config,
    pub seed: u64,
    pub scripts: Box<[skein_fake_llm_domain::api::Script]>,
}
/// Starts the scripted plaintext model peer on the simulated machine.
#[must_use]
pub fn llm(start: LlmStart) -> llm::Peer {
    let domain = skein_fake_llm_domain::Domain::try_scripted(&start.config, start.seed, start.scripts)
        .expect("bounded provider scripts");
    llm::Peer::new(
        start.address,
        skein_fake_peers::Transport::Plaintext,
        start.limits,
        start.provider,
        start.credential,
        start.protocol,
        domain,
        start.config,
    )
    .expect("configured provider")
}

/// The forge peer's configuration and shared repository state, supplied as values.
pub struct ForgeStart {
    pub address: SocketAddr,
    pub limits: skein_fake_peers::Limits,
    pub table: skein_fake_forge_protocol::Config,
    pub protocol: skein_fake_forge_protocol::Limits,
    pub forge: Rc<RefCell<forge::Forge>>,
    pub seed: u64,
}
/// Starts the forge byte peer over the same forge the hosted git program uses.
#[must_use]
pub fn forge(start: ForgeStart) -> forge::Peer {
    forge::Peer::new(start.address, start.limits, start.table, start.protocol, start.forge, start.seed)
        .expect("forge peer")
}
