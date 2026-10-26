//! Kept deployments and offline reads (protocol/README.md, 9; draft/upgrades.md, 13).
use crate::{fixtures, referee::durable};
use jig_domain_core as core;
use jig_protocol_store::{codec, keys, secrets_v1 as secrets, transcripts};
use skein_fake_machine::{How, Is, Item, Machine, Opened};
use skein_lib::{List, Reader, Secret};
use std::collections::BTreeMap;

/// Every entry beneath the deployment root, and the inputs handed to startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployment {
    pub files: BTreeMap<Box<[u8]>, Entry>,
    pub configuration: Box<[u8]>,
    pub startup: BTreeMap<Box<[u8]>, Box<[u8]>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: Is,
    pub mode: u32,
    pub owner: u32,
    pub bytes: Box<[u8]>,
}
pub(crate) struct Startup {
    pub configuration: Box<[u8]>,
    pub entries: BTreeMap<Box<[u8]>, Box<[u8]>>,
}
impl Startup {
    pub(crate) fn fixture() -> Self {
        Self {
            configuration: fixtures::document().into(),
            entries: fixtures::files()
                .entries
                .into_boxed()
                .into_vec()
                .into_iter()
                .map(|(name, value)| (name, value.expose().into()))
                .collect(),
        }
    }
    pub(crate) fn files(&self) -> temper_service::settings::Files {
        let mut entries = List::with_capacity(u32::try_from(self.entries.len()).expect("startup file count"));
        for (name, bytes) in &self.entries {
            entries.push((name.clone(), Secret::new(bytes.clone()))).expect("startup file room");
        }
        temper_service::settings::Files { entries }
    }
}
impl Deployment {
    pub(crate) fn capture(machine: &mut Machine, startup: &Startup) -> Self {
        let root = machine.reopen_root(0);
        let mut files = BTreeMap::new();
        tree(machine, root, b"", &mut files);
        machine.close(root);
        Self { files, configuration: startup.configuration.clone(), startup: startup.entries.clone() }
    }
    pub(crate) fn startup(&self) -> Startup {
        Startup { configuration: self.configuration.clone(), entries: self.startup.clone() }
    }
    pub(crate) fn machine(&self) -> Machine {
        let items: Vec<_> = self
            .files
            .iter()
            .map(|(path, entry)| {
                match entry.kind {
                    Is::File => Item::file(path, &entry.bytes),
                    Is::Directory => Item::directory(path),
                    Is::Link | Is::Fifo | Is::Device => panic!("deployment contains an unsupported entry"),
                }
                .mode(entry.mode)
                .owner(entry.owner)
            })
            .collect();
        let mut machine = Machine::for_user(1000);
        let root = machine.lay(&items);
        machine.close(root);
        // Worker and client roots belong to the new machine, outside the deployment.
        for items in [vec![Item::directory(b"workspaces"), Item::directory(b"trash"), Item::directory(b"home")], vec![]]
        {
            let root = machine.lay(&items);
            machine.close(root);
        }
        machine
    }
    fn bytes(&self, path: &[u8]) -> &[u8] {
        let entry = self.files.get(path).expect("kept file");
        assert_eq!(entry.kind, Is::File, "kept path is a regular file");
        &entry.bytes
    }
}
fn tree(machine: &mut Machine, root: Opened, path: &[u8], files: &mut BTreeMap<Box<[u8]>, Entry>) {
    let dir = machine
        .open(root, if path.is_empty() { b"." } else { path }, How::DirectoryNoFollow)
        .expect("capture directory");
    loop {
        let entries = machine.list(dir, 32, 8192).expect("capture page");
        if entries.is_empty() {
            break;
        }
        for (kind, name) in entries {
            assert!(matches!(kind, Is::File | Is::Directory), "deployment entry must be a file or directory");
            let path = if path.is_empty() { name } else { [path, b"/", &name].concat().into() };
            let file = machine.open(root, &path, How::ReadNoFollow).expect("capture entry");
            let stat = machine.stat(file);
            let bytes = if kind == Is::File {
                machine.read(file, 0, u32::MAX).expect("capture bytes").into()
            } else {
                Box::default()
            };
            machine.close(file);
            files.insert(path.clone(), Entry { kind, mode: stat.mode, owner: stat.owner, bytes });
            if kind == Is::Directory {
                tree(machine, root, &path, files);
            }
        }
    }
    machine.close(dir);
}

pub type Rows = BTreeMap<Box<[u8]>, Box<[u8]>>;
/// Jig's secret envelopes, decoded with their owning generated codecs.
#[derive(Debug, PartialEq, Eq)]
pub enum JigSecret {
    Digest(secrets::Digest),
    Source(secrets::Source),
    Carried(secrets::Carried),
    Next(secrets::NextCarried),
}
/// What the active generation holds, independent of a running service.
#[derive(Debug, PartialEq, Eq)]
pub struct Held {
    pub generation: u64,
    pub format: codec::component::Format,
    pub manifest: codec::component::Manifest,
    pub store: Rows,
    pub secrets: Rows,
    pub jig: BTreeMap<core::Key, core::Record>,
    pub application: BTreeMap<temper_engine_domain_forge::Key, temper_engine_domain_forge::Stored>,
    pub jig_secrets: BTreeMap<Box<[u8]>, JigSecret>,
    /// Decoded application credentials; their original encodings remain in `secrets`.
    pub application_secrets: BTreeMap<u64, Box<[u8]>>,
    pub turns: BTreeMap<u64, Vec<core::TurnRecord>>,
}
impl Held {
    #[must_use]
    pub fn read(deployment: &Deployment) -> Self {
        let generation = std::str::from_utf8(deployment.bytes(b"active"))
            .expect("active text")
            .trim_end_matches('\n')
            .parse::<u64>()
            .expect("active generation");
        let prefix = format!("generations/{generation}");
        let store = rows(deployment, format!("{prefix}/store").as_bytes());
        let secret_rows = rows(deployment, format!("{prefix}/secrets").as_bytes());
        let format_key = keys::encode_own(&keys::Own::Format, u32::MAX).expect("format key");
        let format = codec::component::Format::decode(store.get(&format_key).expect("format row")).expect("format");
        let manifest = codec::component::Manifest::decode(deployment.bytes(format!("{prefix}/manifest").as_bytes()))
            .expect("manifest");
        let limits = fixtures::codec_for(&deployment.configuration);
        let names = fixtures::names_for(&deployment.configuration);
        let bounds = temper_protocol_store::bounds(
            &temper_service::limits::profile(temper_service::limits::Profile::Small).forge,
        )
        .expect("application bounds");
        let mut jig = BTreeMap::new();
        let mut application = BTreeMap::new();
        let mut turns = BTreeMap::new();
        for (key, value) in &store {
            if let Some(key) = keys::decode(key) {
                let row = codec::decode(&key, value, &limits, &names).expect("jig row");
                jig.insert(key, row);
            } else if key.first() == Some(&keys::APPLICATION) {
                let row = temper_protocol_store::records::decode(value, &bounds).expect("application row");
                application.insert(temper_engine_domain_forge::stored_key(&row), row);
            } else if let Some(keys::Own::Transcript { task }) = keys::decode_own(key) {
                let place = codec::component::Transcript::decode(value).expect("transcript place");
                let path = format!("transcripts/{:016x}/{task:016x}", task >> 12);
                let bytes =
                    &deployment.bytes(path.as_bytes())[..usize::try_from(place.length).expect("transcript size")];
                let mut at = 0;
                let mut records = Vec::new();
                while at < bytes.len() {
                    let length = u32::from_be_bytes(bytes[at..at + 4].try_into().expect("frame length")) as usize;
                    records.push(transcripts::frame::decode(&bytes[at..at + length], &limits).expect("turn"));
                    at += length;
                }
                turns.insert(task, records);
            }
        }
        let (jig_secrets, application_secrets) = decoded_secrets(&secret_rows);
        Self {
            generation,
            format,
            manifest,
            store,
            secrets: secret_rows,
            jig,
            application,
            jig_secrets,
            application_secrets,
            turns,
        }
    }
}
fn rows(deployment: &Deployment, directory: &[u8]) -> Rows {
    let mut result = Rows::new();
    let snapshot = [directory, b"/snapshot"].concat();
    let mut after = 0;
    if let Some(entry) = deployment.files.get(snapshot.as_slice()) {
        let (number, ops) = durable::snapshots::read(&entry.bytes).expect("complete snapshot");
        after = number;
        apply(&mut result, ops);
    }
    let prefix = [directory, b"/log-"].concat();
    for (path, entry) in &deployment.files {
        if let Some(number) = path.strip_prefix(prefix.as_slice())
            && number.len() == 20
            && number.iter().all(u8::is_ascii_digit)
        {
            for (number, ops) in durable::frames(&entry.bytes, after).0 {
                if number > after {
                    apply(&mut result, ops);
                    after = number;
                }
            }
        }
    }
    result
}
fn apply(rows: &mut Rows, ops: Box<[skein_kv::Op]>) {
    for op in ops {
        match op {
            skein_kv::Op::Put { key, value } => {
                rows.insert(key, value);
            }
            skein_kv::Op::Erase { key } => {
                rows.remove(&key);
            }
        }
    }
}
type DecodedSecrets = (BTreeMap<Box<[u8]>, JigSecret>, BTreeMap<u64, Box<[u8]>>);
fn decoded_secrets(rows: &Rows) -> DecodedSecrets {
    let mut jig = BTreeMap::new();
    let mut application = BTreeMap::new();
    for (key, value) in rows {
        let mut reader = Reader::new(value);
        let limits = &secrets::CEILINGS;
        let row = if key.starts_with(&[1, 0]) || key.starts_with(&[1, 1]) {
            JigSecret::Digest(secrets::Digest::decode(limits, &mut reader).expect("credential digest"))
        } else if key.starts_with(&[3, 0]) {
            JigSecret::Source(secrets::Source::decode(limits, &mut reader).expect("account source"))
        } else if key.starts_with(&[2, 0]) {
            let mut key_reader = skein_kv::KeyReader::new(key);
            key_reader.tag().expect("application tag");
            key_reader.tag().expect("owner tag");
            let number = key_reader.u64().expect("secret number");
            if number >= temper_protocol_records::CARRIED {
                JigSecret::Carried(secrets::Carried::decode(limits, &mut reader).expect("carried value"))
            } else {
                let (_, _, item) = temper_protocol_records::parse(number).expect("application secret item");
                let secret = temper_protocol_records::decode(item, value).expect("application secret");
                application.insert(number, secret.expose().into());
                continue;
            }
        } else if key.as_ref() == [2, 1] {
            JigSecret::Next(secrets::NextCarried::decode(limits, &mut reader).expect("next carried number"))
        } else {
            panic!("unknown secrets key");
        };
        assert!(reader.is_empty(), "whole secret row");
        jig.insert(key.clone(), row);
    }
    (jig, application)
}

#[cfg(test)]
mod tests;
