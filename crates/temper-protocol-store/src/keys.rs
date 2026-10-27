//! Handwritten keys preserve the forge's derived order (protocol/store.md, section 3).
use alloc::boxed::Box;
use skein_kv::{KeyReader, KeyWriter};
use skein_lib::List;
use temper_engine_domain_forge as forge;

/// The `Binding` family's byte, taken at landing as a version is (protocol/store.md, 2).
pub const BINDING: u8 = 18;
/// The `DemoAct` family's byte (protocol/setup.md, 8.4).
pub const DEMO_ACT: u8 = 19;
/// A write with a committed attempt (protocol/store.md, 2).
pub const FLIGHT: u8 = 21;
/// One landing's verification (protocol/store.md, 2; domain/after-landing.md, 10).
pub const VERIFICATION: u8 = 26;
/// One repository tier (domain/after-landing.md, 10).
pub const TIER: u8 = 25;

fn write_name(writer: &mut KeyWriter, name: &forge::Name) {
    writer.u64(u64::from(name.forge)).u64(u64::from(name.repository));
    match &name.what {
        forge::What::Repository => {
            writer.tag(0);
        }
        forge::What::Branch(segments) => {
            writer.tag(1);
            for segment in segments {
                writer.tag(1).bytes(segment);
            }
            writer.tag(0);
        }
        forge::What::Pull(number) => {
            writer.tag(2).u64(*number);
        }
        forge::What::Issue(number) => {
            writer.tag(3).u64(*number);
        }
    }
}

fn read_name(reader: &mut KeyReader<'_>, limits: &forge::Limits) -> Option<forge::Name> {
    let forge = u16::try_from(reader.u64()?).ok()?;
    let repository = u32::try_from(reader.u64()?).ok()?;
    let what = match reader.tag()? {
        0 => forge::What::Repository,
        1 => forge::What::Branch(read_segments(reader, limits.name_bytes)?),
        2 => forge::What::Pull(reader.u64()?),
        3 => forge::What::Issue(reader.u64()?),
        _ => return None,
    };
    Some(forge::Name { forge, repository, what })
}

fn read_segments(reader: &mut KeyReader<'_>, limit: u32) -> Option<Box<[Box<[u8]>]>> {
    let mut segments = List::with_capacity(limit);
    let mut size = 0_u32;
    for _ in 0..=limit {
        match reader.tag()? {
            0 => return Some(segments.into_boxed()),
            1 => {
                let segment = reader.bytes()?;
                size = size.checked_add(u32::try_from(segment.len()).ok()?.checked_add(1)?)?;
                if size > limit {
                    return None;
                }
                segments.push(segment).ok()?;
            }
            _ => return None,
        }
    }
    None
}

/// The longest relative key under the forge's bounds.
#[must_use]
pub fn largest(limits: &forge::Limits) -> Option<u32> {
    let name = limits.name_bytes.checked_mul(3)?.checked_add(19)?;
    let topic = limits.name_bytes.checked_mul(2)?.checked_add(28)?;
    // Writer is one family byte and an eight-byte forge number: 9 bytes.
    // Binding: family byte, visible address bytes (no zero escapes), and byte-string terminator.
    Some(name.max(topic).max(159).max(1 + 254 + 2))
}

/// Encode one relative key, or refuse it when it exceeds `limit`.
#[must_use]
pub fn encode(key: &forge::Key, limit: u32) -> Option<Box<[u8]>> {
    let mut writer = KeyWriter::new(limit);
    match key {
        forge::Key::Landing(task) => {
            writer.tag(23).u64(*task);
        }
        forge::Key::Receipt(key) => {
            writer.tag(24).u64(u64::from(key.project));
            write_repository(&mut writer, key.repository);
            match &key.check {
                forge::Check::Gate(name) => {
                    writer.tag(0).bytes(name);
                }
                forge::Check::Tier(name) => {
                    writer.tag(1).bytes(name);
                }
            }
            writer.bytes(&key.key);
        }
        forge::Key::Call(call) => {
            writer
                .tag(0)
                .u64(call.task)
                .u64(call.attempt)
                .u64(u64::from(call.completion))
                .u64(u64::from(call.position));
        }
        forge::Key::ProposedEffect(number) => {
            writer.tag(1).u64(*number);
        }
        forge::Key::Repository(repository) => {
            writer.tag(2);
            write_repository(&mut writer, *repository);
        }
        forge::Key::Hold(name) => {
            writer.tag(3);
            write_name(&mut writer, name);
        }
        forge::Key::Names(task) => {
            writer.tag(4).u64(*task);
        }
        forge::Key::Subscription { task, topic } => {
            writer.tag(5).u64(*task);
            write_topic(&mut writer, topic);
        }
        forge::Key::BranchHead(name) => {
            writer.tag(6);
            write_name(&mut writer, name);
        }
        forge::Key::PullState(name) => {
            writer.tag(7);
            write_name(&mut writer, name);
        }
        forge::Key::Ci { repository, head } => {
            writer.tag(8);
            write_repository(&mut writer, *repository);
            writer.bytes(head);
        }
        forge::Key::Landed(commit) => {
            writer.tag(9).bytes(commit);
        }
        forge::Key::Entry(number) => {
            writer.tag(10).u64(*number);
        }
        forge::Key::Client(key) => {
            writer.tag(11);
            write_client(&mut writer, key);
        }
        forge::Key::Change(task) => {
            writer.tag(12).u64(*task);
        }
        forge::Key::Issue(goal) => {
            writer.tag(13).u64(*goal);
        }
        forge::Key::Release(task) => {
            writer.tag(14).u64(*task);
        }
        forge::Key::Application(token) => {
            writer.tag(15).u64(*token);
        }
        forge::Key::Binding(address) => {
            if !forge::binding_address(address) {
                return None;
            }
            writer.tag(BINDING).bytes(address);
        }
        forge::Key::Tier { repository, tier } => {
            writer.tag(TIER);
            write_repository(&mut writer, *repository);
            writer.bytes(tier);
        }
        forge::Key::Verification { repository, commit } => {
            writer.tag(VERIFICATION);
            write_repository(&mut writer, *repository);
            writer.bytes(commit);
        }
        forge::Key::Flight { task, entry } => {
            writer.tag(FLIGHT).u64(*task).u64(*entry);
        }
        forge::Key::DemoAct(key) => {
            writer.tag(DEMO_ACT).bytes(key);
        }
        forge::Key::Demo(forge) => {
            writer.tag(17).u64(u64::from(*forge));
        }
        forge::Key::Writer(forge) => {
            writer.tag(16).u64(u64::from(*forge));
        }
    }
    writer.finish()
}

/// Decode a whole key, refusing unknown tags, widths, bounds and trailing bytes.
#[must_use]
pub fn decode(bytes: &[u8], limits: &forge::Limits) -> Option<forge::Key> {
    let mut reader = KeyReader::new(bytes);
    let key = match reader.tag()? {
        0 => forge::Key::Call(forge::NamedCall {
            task: reader.u64()?,
            attempt: reader.u64()?,
            completion: u32::try_from(reader.u64()?).ok()?,
            position: u32::try_from(reader.u64()?).ok()?,
        }),
        1 => forge::Key::ProposedEffect(reader.u64()?),
        2 => forge::Key::Repository(read_repository(&mut reader)?),
        3 => forge::Key::Hold(read_name(&mut reader, limits)?),
        4 => forge::Key::Names(reader.u64()?),
        5 => forge::Key::Subscription { task: reader.u64()?, topic: read_topic(&mut reader, limits)? },
        6 => forge::Key::BranchHead(read_name(&mut reader, limits)?),
        7 => forge::Key::PullState(read_name(&mut reader, limits)?),
        8 => forge::Key::Ci { repository: read_repository(&mut reader)?, head: read_commit(&mut reader)? },
        9 => forge::Key::Landed(read_commit(&mut reader)?),
        10 => forge::Key::Entry(reader.u64()?),
        11 => forge::Key::Client(read_client(&mut reader, limits)?),
        12 => forge::Key::Change(reader.u64()?),
        13 => forge::Key::Issue(reader.u64()?),
        14 => forge::Key::Release(reader.u64()?),
        15 => forge::Key::Application(reader.u64()?),
        16 => forge::Key::Writer(u16::try_from(reader.u64()?).ok()?),
        TIER => {
            let repository = read_repository(&mut reader)?;
            let tier = reader.bytes()?;
            if tier.is_empty()
                || tier.len() > usize::try_from(forge::CHECK_NAME_BYTES).expect("tier name bound fits usize")
            {
                return None;
            }
            for byte in &tier {
                if !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_' {
                    return None;
                }
            }
            forge::Key::Tier { repository, tier }
        }
        VERIFICATION => {
            forge::Key::Verification { repository: read_repository(&mut reader)?, commit: read_commit(&mut reader)? }
        }
        FLIGHT => forge::Key::Flight { task: reader.u64()?, entry: reader.u64()? },
        DEMO_ACT => forge::Key::DemoAct(reader.bytes()?.as_ref().try_into().ok()?),
        17 => forge::Key::Demo(u16::try_from(reader.u64()?).ok()?),
        BINDING => {
            let address = reader.bytes()?;
            if !forge::binding_address(&address) {
                return None;
            }
            forge::Key::Binding(address)
        }
        23 => forge::Key::Landing(reader.u64()?),
        24 => {
            let project = u32::try_from(reader.u64()?).ok()?;
            let repository = read_repository(&mut reader)?;
            let tag = reader.tag()?;
            let name = reader.bytes()?;
            if name.is_empty() || name.len() > 32 {
                return None;
            }
            let check = match tag {
                0 => forge::Check::Gate(name),
                1 => forge::Check::Tier(name),
                _ => return None,
            };
            forge::Key::Receipt(forge::ReceiptKey { project, repository, check, key: read_commit(&mut reader)? })
        }
        _ => return None,
    };
    if !reader.is_empty() {
        return None;
    }
    Some(key)
}

fn write_repository(writer: &mut KeyWriter, repository: skein_forge::api::Repository) {
    writer.u64(u64::from(repository.forge)).u64(u64::from(repository.repository));
}
fn read_repository(reader: &mut KeyReader<'_>) -> Option<skein_forge::api::Repository> {
    Some(skein_forge::api::Repository {
        forge: u16::try_from(reader.u64()?).ok()?,
        repository: u32::try_from(reader.u64()?).ok()?,
    })
}
fn read_commit(reader: &mut KeyReader<'_>) -> Option<skein_forge::api::Commit> {
    reader.bytes()?.as_ref().try_into().ok()
}
fn read_branch(reader: &mut KeyReader<'_>, limits: &forge::Limits) -> Option<Box<[u8]>> {
    let branch = reader.bytes()?;
    if branch.len() > usize::try_from(limits.name_bytes).ok()? {
        return None;
    }
    Some(branch)
}
fn write_topic(writer: &mut KeyWriter, topic: &forge::Topic) {
    match topic {
        forge::Topic::Landings { repository, branch } => {
            writer.tag(0);
            write_repository(writer, *repository);
            writer.bytes(branch);
        }
        forge::Topic::Ci { repository, head } => {
            writer.tag(1);
            write_repository(writer, *repository);
            writer.bytes(head);
        }
        forge::Topic::Pull { repository, number } => {
            writer.tag(2);
            write_repository(writer, *repository);
            writer.u64(*number);
        }
        forge::Topic::Participation { repository, number } => {
            writer.tag(3);
            write_repository(writer, *repository);
            writer.u64(*number);
        }
    }
}
fn read_topic(reader: &mut KeyReader<'_>, limits: &forge::Limits) -> Option<forge::Topic> {
    let tag = reader.tag()?;
    let repository = read_repository(reader)?;
    match tag {
        0 => Some(forge::Topic::Landings { repository, branch: read_branch(reader, limits)? }),
        1 => Some(forge::Topic::Ci { repository, head: read_commit(reader)? }),
        2 => Some(forge::Topic::Pull { repository, number: reader.u64()? }),
        3 => Some(forge::Topic::Participation { repository, number: reader.u64()? }),
        _ => None,
    }
}
fn write_client(writer: &mut KeyWriter, key: &skein_forge_client::Key) {
    match key {
        skein_forge_client::Key::Writer(_) => unreachable!("the connector keeps the client's writer as Stored::Writer"),
        skein_forge_client::Key::Live(resource) => {
            writer.tag(0);
            write_repository(writer, resource.repository);
            match &resource.what {
                skein_forge_client::What::Repository => {
                    writer.tag(0);
                }
                skein_forge_client::What::Branch(branch) => {
                    writer.tag(1).bytes(branch);
                }
                skein_forge_client::What::Pull(number) => {
                    writer.tag(2).u64(*number);
                }
                skein_forge_client::What::Issue(number) => {
                    writer.tag(3).u64(*number);
                }
            }
        }
        skein_forge_client::Key::Repository(repository) => {
            writer.tag(1);
            write_repository(writer, *repository);
        }
    }
}
fn read_client(reader: &mut KeyReader<'_>, limits: &forge::Limits) -> Option<skein_forge_client::Key> {
    match reader.tag()? {
        0 => {
            let repository = read_repository(reader)?;
            let what = match reader.tag()? {
                0 => skein_forge_client::What::Repository,
                1 => skein_forge_client::What::Branch(read_branch(reader, limits)?),
                2 => skein_forge_client::What::Pull(reader.u64()?),
                3 => skein_forge_client::What::Issue(reader.u64()?),
                _ => return None,
            };
            Some(skein_forge_client::Key::Live(skein_forge_client::Resource { repository, what }))
        }
        1 => Some(skein_forge_client::Key::Repository(read_repository(reader)?)),
        _ => None,
    }
}

/// Decode a Flight key alone, before startup has any application limits.
#[must_use]
pub fn flight(bytes: &[u8]) -> Option<(u64, u64)> {
    let mut reader = KeyReader::new(bytes);
    if reader.tag()? != FLIGHT {
        return None;
    }
    let task = reader.u64()?;
    let entry = reader.u64()?;
    if !reader.is_empty() {
        return None;
    }
    Some((task, entry))
}
