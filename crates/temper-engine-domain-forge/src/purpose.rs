//! Stable forge effect identities (domain/forge.md, section 6).
use alloc::boxed::Box;
use skein_lib::List;

/// Why an effect exists, independent of its outbox entry number or wall time
/// (jig's domain/connectors.md, section 4.4).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EffectPurpose {
    /// One task history milestone within a goal projection.
    ProjectionHistory { goal: u64, task: u64, position: u64, family: u8 },
    /// One agent call in one task attempt.
    Call { task: u64, attempt: u64, completion: u32, position: u32 },
    /// One purpose within a task's procedure.
    Step { task: u64, purpose: u16 },
    /// One part of a goal's projection.
    Projection { goal: u64, part: u8, number: u64 },
    /// A tier run's operation (domain/after-landing.md, 10).
    Tier { repository: [u8; 16], tier: u16, run: u64, operation: u8, part: u16 },
    /// Cleanup of a task's named resource.
    Release { task: u64, resource: Box<[u8]> },
}

impl EffectPurpose {
    /// Tag, then fields big-endian; history encodes family before goal, task and position.
    #[must_use]
    pub fn identity(&self) -> Option<Box<[u8]>> {
        let purpose_bytes = match self {
            EffectPurpose::ProjectionHistory { .. } => 26,
            EffectPurpose::Call { .. } => 25,
            EffectPurpose::Tier { .. } => 30,
            EffectPurpose::Step { .. } => 11,
            EffectPurpose::Projection { .. } => 18,
            EffectPurpose::Release { resource, .. } => 11_usize.checked_add(resource.len())?,
        };
        let mut bytes = List::with_capacity(u32::try_from(purpose_bytes).ok()?);
        match self {
            EffectPurpose::ProjectionHistory { goal, task, position, family } => {
                bytes.push(5).ok()?;
                bytes.push(*family).ok()?;
                for byte in goal.to_be_bytes().into_iter().chain(task.to_be_bytes()).chain(position.to_be_bytes()) {
                    bytes.push(byte).ok()?;
                }
            }
            EffectPurpose::Tier { repository, tier, run, operation, part } => {
                bytes.push(8).ok()?;
                for byte in repository
                    .iter()
                    .copied()
                    .chain(tier.to_be_bytes())
                    .chain(run.to_be_bytes())
                    .chain([*operation])
                    .chain(part.to_be_bytes())
                {
                    bytes.push(byte).ok()?;
                }
            }
            EffectPurpose::Call { task, attempt, completion, position } => {
                bytes.push(1).ok()?;
                for byte in task
                    .to_be_bytes()
                    .into_iter()
                    .chain(attempt.to_be_bytes())
                    .chain(completion.to_be_bytes())
                    .chain(position.to_be_bytes())
                {
                    bytes.push(byte).ok()?;
                }
            }
            EffectPurpose::Step { task, purpose } => {
                bytes.push(2).ok()?;
                for byte in task.to_be_bytes().into_iter().chain(purpose.to_be_bytes()) {
                    bytes.push(byte).ok()?;
                }
            }
            EffectPurpose::Projection { goal, part, number } => {
                bytes.push(3).ok()?;
                for byte in goal.to_be_bytes() {
                    bytes.push(byte).ok()?;
                }
                bytes.push(*part).ok()?;
                for byte in number.to_be_bytes() {
                    bytes.push(byte).ok()?;
                }
            }
            EffectPurpose::Release { task, resource } => {
                bytes.push(4).ok()?;
                for byte in task.to_be_bytes().into_iter().chain(u16::try_from(resource.len()).ok()?.to_be_bytes()) {
                    bytes.push(byte).ok()?;
                }
                for byte in resource {
                    bytes.push(*byte).ok()?;
                }
            }
        }
        Some(bytes.into_boxed())
    }

    /// Frame this identity within the deployment namespace and byte cap.
    #[must_use]
    pub fn key(&self, namespace: &[u8], max_bytes: u32) -> Option<Box<[u8]>> {
        skein_forge_client::effect_key(namespace, &self.identity()?, max_bytes)
    }
}
