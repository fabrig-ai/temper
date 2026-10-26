//! Retained outcomes, field for field with the forge's application family.
use super::{FITS, names};
use crate::{Bounds, Undecodable, v10 as w};
use skein_lib::{Reader, Wall, Writer};
use temper_engine_domain_forge as d;
mod refusal;

pub(super) fn encode(token: u64, until: Wall, outcome: &d::ApplicationOutcome) -> w::Application {
    let outcome = match outcome {
        d::ApplicationOutcome::Adopted(v) => w::ApplicationOutcome::Adopted(
            w::ApplicationAdopted::new(
                &w::CEILINGS,
                w::ApplicationAdoptedParts {
                    repository: names::encode_repository(v.repository),
                    owner: v.owner.clone(),
                    name: v.name.clone(),
                    role: names::encode_role(v.role),
                    home: v.home,
                    kinds: names::encode_kinds(&v.kinds),
                    protection: names::encode_protection(&v.protection),
                },
            )
            .expect(FITS),
        ),
        d::ApplicationOutcome::CoreRefused(v) => {
            let record =
                refusal::encode_refusal(v, &jig_protocol_store::people_v13::CEILINGS).expect("refusal facts fit");
            let mut writer = Writer::new(usize::try_from(record.measure()).expect("small refusal"));
            record.encode(&mut writer).expect("measured room");
            w::ApplicationOutcome::CoreRefused(
                w::CoreRefused::new(&w::CEILINGS, w::CoreRefusedParts { refusal: writer.finish() }).expect(FITS),
            )
        }
        d::ApplicationOutcome::ForgeRefused(v) => w::ApplicationOutcome::ForgeRefused(
            w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason: encode_reason(*v) }).expect(FITS),
        ),
    };
    w::Application::new(&w::CEILINGS, w::ApplicationParts { token, until: until.as_nanos(), outcome }).expect(FITS)
}
pub(super) fn decode(value: w::Application, bounds: &Bounds) -> Result<d::Stored, Undecodable> {
    let w::ApplicationParts { token, until, outcome } = value.into_parts();
    let outcome = match outcome {
        w::ApplicationOutcome::Adopted(v) => {
            let w::ApplicationAdoptedParts { repository, owner, name, role, home, kinds, protection } = v.into_parts();
            d::ApplicationOutcome::Adopted(d::AdoptedRepository {
                repository: names::decode_repository(repository),
                owner,
                name,
                role: names::decode_role(role),
                home,
                kinds: names::decode_kinds(kinds),
                protection: names::decode_protection(protection, bounds)?,
            })
        }
        w::ApplicationOutcome::CoreRefused(v) => {
            let mut reader = Reader::new(v.refusal());
            let record =
                jig_protocol_store::people_v13::Refusal::decode(&jig_protocol_store::people_v13::CEILINGS, &mut reader)
                    .or(Err(Undecodable))?;
            d::ApplicationOutcome::CoreRefused(refusal::decode_refusal(record))
        }
        w::ApplicationOutcome::ForgeRefused(v) => {
            d::ApplicationOutcome::ForgeRefused(decode_reason(v.into_parts().reason))
        }
    };
    Ok(d::Stored::Application { token, until: Wall::from_nanos(until), outcome })
}
fn encode_reason(v: d::AdoptionRefusal) -> w::ForgeReason {
    match v {
        d::AdoptionRefusal::NotInstalled => w::ForgeReason::NotInstalled,
        d::AdoptionRefusal::NoSuchForge => w::ForgeReason::NoSuchForge,
        d::AdoptionRefusal::NotFound => w::ForgeReason::NotFound,
        d::AdoptionRefusal::NoAccess => w::ForgeReason::NoAccess,
        d::AdoptionRefusal::Refused => w::ForgeReason::Refused,
        d::AdoptionRefusal::InFlight => w::ForgeReason::InFlight,
        d::AdoptionRefusal::TooLarge => w::ForgeReason::TooLarge,
        d::AdoptionRefusal::Unreachable => w::ForgeReason::Unreachable,
        d::AdoptionRefusal::InvalidAnswer => w::ForgeReason::InvalidAnswer,
        d::AdoptionRefusal::AnotherProject => w::ForgeReason::AnotherProject,
        d::AdoptionRefusal::HomeHeld => w::ForgeReason::HomeHeld,
        d::AdoptionRefusal::PrefixOccupied => w::ForgeReason::PrefixOccupied,
    }
}
fn decode_reason(v: w::ForgeReason) -> d::AdoptionRefusal {
    match v {
        w::ForgeReason::NotInstalled => d::AdoptionRefusal::NotInstalled,
        w::ForgeReason::NoSuchForge => d::AdoptionRefusal::NoSuchForge,
        w::ForgeReason::NotFound => d::AdoptionRefusal::NotFound,
        w::ForgeReason::NoAccess => d::AdoptionRefusal::NoAccess,
        w::ForgeReason::Refused => d::AdoptionRefusal::Refused,
        w::ForgeReason::InFlight => d::AdoptionRefusal::InFlight,
        w::ForgeReason::TooLarge => d::AdoptionRefusal::TooLarge,
        w::ForgeReason::Unreachable => d::AdoptionRefusal::Unreachable,
        w::ForgeReason::InvalidAnswer => d::AdoptionRefusal::InvalidAnswer,
        w::ForgeReason::AnotherProject => d::AdoptionRefusal::AnotherProject,
        w::ForgeReason::HomeHeld => d::AdoptionRefusal::HomeHeld,
        w::ForgeReason::PrefixOccupied => d::AdoptionRefusal::PrefixOccupied,
    }
}
