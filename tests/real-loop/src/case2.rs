//! Reference case, second edition: data and hour 0 on fakes
//! (draft/reference-case.md, sections 2 and 3). Stories take the step's number;
//! the opening checkpoint precedes step 8.1.
use crate::{case, forge, landing};
use serde_json::{Value, json};
use skein_fake_forge_domain::api;
use std::net::SocketAddr;

pub mod acts;
pub mod cast;
pub mod oracle;
pub mod scripts;
pub use acts::acts;
pub use cast::*;
pub use scripts::scripts;

pub const PROJECT: u64 = 1;
pub const WORKERS: &[&str] = &["director", "owner", "i1", "i2"];
// The first edition permits ten units per run. A delegator also reserves
// its descendants' purses; CAST fits forty units under the chat's fifty.
pub const RUN_SPEND: u64 = 10;

/// Materialized fixture input. The adapters below supply the harness's function
/// pointers, while this value keeps each cast and its landing together.
pub struct Fixture {
    pub cast: Cast,
    pub landing: Landing,
}
#[must_use]
pub fn fixture(cast: &Cast) -> Fixture {
    assert!(!cast.members.is_empty(), "a director opens the case");
    assert!(matches!(cast.members[0].kind, MemberKind::Chat));
    let mut homes = 0;
    for repository in cast.repositories {
        homes += usize::from(repository.home);
    }
    assert_eq!(homes, 1, "one home repository");
    // Direct children are delegated in cast order. Require the creation order
    // to agree, so numbers cannot silently lie.
    let mut next = 1;
    for (at, member) in cast.members.iter().enumerate() {
        assert_eq!(cast.number(member.name), u64::try_from(at).expect("cast length") + 1, "unique names");
        assert!(member.spec.contains(member.cue), "a spec carries its script cue");
        if at == 0 {
            assert!(member.under.is_none());
        } else {
            let parent = member.under.expect("only the chat has no parent");
            assert!(cast.number(parent) < cast.number(member.name), "parent created first");
            assert!(!matches!(member.kind, MemberKind::Chat), "one person's chat");
        }
        for child in cast.members {
            if child.under == Some(member.name) {
                assert_eq!(cast.number(child.name), u64::try_from(next).expect("cast length") + 1);
                next += 1;
            }
        }
    }
    assert_eq!(next, cast.members.len());
    Fixture { cast: *cast, landing: cast.landing }
}
impl Fixture {
    #[must_use]
    pub fn setups(&self) -> Box<[api::Setup]> {
        let mut setups = Vec::with_capacity(self.cast.repositories.len());
        // The hosted git fixture gives its primary (home) repository ID 2.
        for home in [true, false] {
            for repository in self.cast.repositories {
                if repository.home != home {
                    continue;
                }
                let mut setup = (forge::fixture().setup)();
                setup.id = forge::ID + u32::try_from(setups.len()).expect("repository count");
                setup.name = format!("{}/{}", repository.owner, repository.name).into_bytes().into();
                let mut tree = Vec::with_capacity(repository.tree.len());
                for (path, content) in repository.tree {
                    tree.push(api::File { path: path.as_bytes().into(), content: content.as_bytes().into() });
                }
                setup.tree = tree.into_boxed_slice();
                let mut contexts = Vec::with_capacity(self.landing.gates.len());
                for gate in self.landing.gates {
                    if gate.repository == repository.name {
                        contexts.push(gate.name.as_bytes().into());
                    }
                }
                setup.checks.contexts = contexts.into_boxed_slice();
                setup.checks.cue = None;
                setups.push(setup);
            }
        }
        setups.into_boxed_slice()
    }

    pub fn configure(&self, value: &mut Value, agent: &mut Value, forge: SocketAddr) {
        // Reuse the first edition's service, accounts, authority and turn bounds.
        (case::fixture().configure)(value, agent, forge);
        assert!(matches!(self.landing.by, LandingBy::Queue), "turn is added by the landings owner");
        assert!(self.landing.gates.is_empty(), "hour 0 has no landing gates yet");
        value["application"]["landing"]["projects"] = json!([]);
        value["engine"]["limits"]["overrides"]["fleet.calls"] = self.cast.members.len().into();
        let mut models = Vec::with_capacity(2);
        let mut agent_models = Vec::with_capacity(2);
        for model in [Model::Claude, Model::Gpt] {
            let mut configured = value["engine"]["llm"]["models"][0].clone();
            configured["name"] = model.name().into();
            configured.as_object_mut().expect("model").remove("efforts");
            models.push(configured);
            let mut configured = agent["endpoints"][0]["models"][0].clone();
            configured["name"] = model.name().into();
            configured.as_object_mut().expect("model").remove("efforts");
            agent_models.push(configured);
        }
        value["engine"]["llm"]["models"] = models.into();
        value["engine"]["llm"]["providers"][0]["models"] = json!(["claude", "gpt"]);
        agent["endpoints"][0]["models"] = agent_models.into();
        let mut charters = Vec::with_capacity(self.cast.members.len());
        for member in self.cast.members {
            charters.push(json!({"number":member.charter,"model":member.model.name()}));
        }
        value["world_settings"]["case2_charters"] = charters.into();
        let mut kinds = Vec::with_capacity(self.cast.members.len());
        for (at, member) in self.cast.members.iter().enumerate() {
            let mut earlier = false;
            for previous in &self.cast.members[..at] {
                earlier |= previous.charter == member.charter;
            }
            if !earlier {
                kinds.push(json!({"kind":"charter","number":member.charter}));
            }
        }
        for project in value["engine"]["projects"].as_array_mut().expect("projects") {
            project["policy"]["ceiling"]["delegation"]["kinds"] = json!(kinds);
            project["policy"]["roles"][0]["authority"]["delegation"]["kinds"] = json!(kinds);
        }
        // Adoption, not repeated polling, establishes these repositories.
        let story_ms = crate::PATIENCE.as_nanos().div_euclid(1_000_000);
        for name in ["poll_ms", "poll_max_ms"] {
            value["application"]["limits"]["overrides"][format!("forge.client.{name}")] = (story_ms + 1).into();
        }
        landing::admit(value);
    }
}

/// Use the coordinator's charter for Claude members and the coding charter for
/// GPT members, retaining stock limits. Both models use the fake wire.
pub(crate) fn defaults(defaults: &mut jig_domain_core::settings::Defaults, flags: &Value) {
    let Some(charters) = flags["case2_charters"].as_array() else { return };
    for configured in charters {
        let number = configured["number"].as_u64().expect("charter number");
        for charter in &mut defaults.charters {
            if u64::from(charter.number) == number {
                if charter.number == COORDINATORS_CHARTER {
                    charter.policy.instructions = skein_lib::bytes::copy_of(COORDINATORS.as_bytes());
                }
                let name = configured["model"].as_str().expect("model").as_bytes();
                if charter.policy.model.name.as_ref() != name {
                    for alternative in &mut charter.policy.alternatives {
                        if alternative.name.as_ref() == name {
                            std::mem::swap(&mut charter.policy.model, alternative);
                            break;
                        }
                    }
                }
                assert_eq!(charter.policy.model.name.as_ref(), name, "configured charter model");
            }
        }
    }
}

// Each callback binds this cast explicitly. No mutable global passes data
// between fixture construction, peer creation and service configuration.
#[must_use]
pub fn fixture_hour_0() -> forge::Fixture {
    forge::Fixture { setup: setup_hour_0, extra_setups: extra_hour_0, configure: configure_hour_0 }
}
fn setup_hour_0() -> api::Setup {
    fixture(&CAST).setups().into_vec().remove(0)
}
fn extra_hour_0() -> Box<[api::Setup]> {
    let mut setups = fixture(&CAST).setups().into_vec();
    setups.remove(0);
    setups.into_boxed_slice()
}
fn configure_hour_0(value: &mut Value, agent: &mut Value, forge: SocketAddr) {
    fixture(&CAST).configure(value, agent, forge);
}
