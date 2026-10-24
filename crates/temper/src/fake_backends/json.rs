//! Startup-only JSON inspection. The bounded protocol parser admits bytes before this tree is made.
use skein_json::{
    Document,
    document::Kind,
    writer::{Encoder, Limits},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Json {
    Object(Vec<(Box<[u8]>, Json)>),
    Array(Vec<Json>),
    String(Box<[u8]>),
    Number(Box<[u8]>),
    Bool(bool),
    Null,
}
impl Json {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let d = jig_protocol_config::document(
            bytes,
            &jig_protocol_config::DocumentLimits { bytes: crate::CONFIG_BYTES, tokens: 65_536, depth: 32 },
        )
        .map_err(|why| format!("public document: {:?}: {:?}", why.path, why.what))?;
        Ok(Self::read(&d, 0)?.0)
    }
    fn read(d: &Document, at: u32) -> Result<(Self, u32), String> {
        let t = d.token(at).ok_or("public document: missing value")?;
        let bytes = d.text(t).ok_or("public document: missing text")?;
        let next = at.checked_add(1).expect("bounded token count");
        match t.kind {
            Kind::ObjectStart => {
                let mut entries: Vec<(Box<[u8]>, Self)> = Vec::new();
                let mut cursor = next;
                while d.token(cursor).is_some_and(|t| t.kind != Kind::ObjectEnd) {
                    let key = d.token(cursor).ok_or("public document: missing key")?;
                    let key = d.text(key).ok_or("public document: missing key text")?;
                    if entries.iter().any(|(name, _)| name.as_ref() == key) {
                        return Err(format!("public document: {}: duplicate", String::from_utf8_lossy(key)));
                    }
                    let (value, after) = Self::read(d, cursor.checked_add(1).expect("bounded object value"))?;
                    entries.push((key.into(), value));
                    cursor = after;
                }
                Ok((Self::Object(entries), cursor.checked_add(1).expect("bounded object close")))
            }
            Kind::ArrayStart => {
                let mut entries = Vec::new();
                let mut cursor = next;
                while d.token(cursor).is_some_and(|t| t.kind != Kind::ArrayEnd) {
                    let (value, after) = Self::read(d, cursor)?;
                    entries.push(value);
                    cursor = after;
                }
                Ok((Self::Array(entries), cursor.checked_add(1).expect("bounded array close")))
            }
            Kind::String => Ok((Self::String(bytes.into()), next)),
            Kind::Number => Ok((Self::Number(bytes.into()), next)),
            Kind::True => Ok((Self::Bool(true), next)),
            Kind::False => Ok((Self::Bool(false), next)),
            Kind::Null => Ok((Self::Null, next)),
            Kind::ObjectEnd | Kind::ArrayEnd | Kind::Key | Kind::Long => Err("public document: invalid value".into()),
        }
    }
    pub(super) fn at(&self, pointer: &str) -> Result<&Self, String> {
        let mut node = self;
        for part in pointer.split('/').filter(|part| !part.is_empty()) {
            node = match node {
                Self::Object(entries) => {
                    entries.iter().find(|(name, _)| name.as_ref() == part.as_bytes()).map(|(_, value)| value)
                }
                Self::Array(entries) => part.parse::<usize>().ok().and_then(|index| entries.get(index)),
                Self::String(_) | Self::Number(_) | Self::Bool(_) | Self::Null => None,
            }
            .ok_or_else(|| format!("{pointer}: missing or wrong kind"))?;
        }
        Ok(node)
    }
    pub(super) fn at_mut(&mut self, pointer: &str) -> Result<&mut Self, String> {
        let mut node = self;
        for part in pointer.split('/').filter(|part| !part.is_empty()) {
            node = match node {
                Self::Object(entries) => {
                    entries.iter_mut().find(|(name, _)| name.as_ref() == part.as_bytes()).map(|(_, value)| value)
                }
                Self::Array(entries) => part.parse::<usize>().ok().and_then(|index| entries.get_mut(index)),
                Self::String(_) | Self::Number(_) | Self::Bool(_) | Self::Null => None,
            }
            .ok_or_else(|| format!("{pointer}: missing or wrong kind"))?;
        }
        Ok(node)
    }
    pub(super) fn set(&mut self, pointer: &str, value: Self) -> Result<(), String> {
        *self.at_mut(pointer)? = value;
        Ok(())
    }
    pub(super) fn string(&self, pointer: &str) -> Result<&[u8], String> {
        match self.at(pointer)? {
            Self::String(bytes) => Ok(bytes),
            Self::Object(_) | Self::Array(_) | Self::Number(_) | Self::Bool(_) | Self::Null => {
                Err(format!("{pointer}: expected text"))
            }
        }
    }
    pub(super) fn encode(&self) -> Result<Box<[u8]>, String> {
        let limits = Limits { depth: 32, length: crate::CONFIG_BYTES };
        let mut encoder = Encoder::measure(&limits);
        self.write(&mut encoder);
        let length = encoder.measured().map_err(|_| "public document: past byte bound")?;
        let mut encoder = Encoder::write(length, &limits);
        self.write(&mut encoder);
        Ok(encoder.finish())
    }
    fn write(&self, e: &mut Encoder) {
        match self {
            Self::Object(entries) => {
                e.object_start();
                for (key, value) in entries {
                    e.key(key);
                    value.write(e);
                }
                e.object_end();
            }
            Self::Array(entries) => {
                e.array_start();
                for value in entries {
                    value.write(e);
                }
                e.array_end();
            }
            Self::String(bytes) => e.string(bytes),
            Self::Number(bytes) => e.number(bytes),
            Self::Bool(value) => e.boolean(*value),
            Self::Null => e.null(),
        }
    }
    /// Equality without depending on object order. The first mismatch names its public path.
    pub(super) fn same(&self, expected: &Self, pointer: &str) -> Result<(), String> {
        match (self, expected) {
            (Self::Object(entries), Self::Object(wanted)) => {
                for (name, value) in entries {
                    let path = format!("{pointer}/{}", String::from_utf8_lossy(name));
                    let other = wanted
                        .iter()
                        .find(|(key, _)| key == name)
                        .ok_or_else(|| format!("{path}: not in current init shape"))?;
                    value.same(&other.1, &path)?;
                }
                for (name, _) in wanted {
                    if !entries.iter().any(|(key, _)| key == name) {
                        return Err(format!(
                            "{pointer}/{}: missing from current init shape",
                            String::from_utf8_lossy(name)
                        ));
                    }
                }
                Ok(())
            }
            (Self::Array(entries), Self::Array(wanted)) if entries.len() == wanted.len() => {
                for (index, (value, other)) in entries.iter().zip(wanted).enumerate() {
                    value.same(other, &format!("{pointer}/{index}"))?;
                }
                Ok(())
            }
            _ if self == expected => Ok(()),
            _ => Err(format!("{pointer}: unsupported change to current init configuration")),
        }
    }
}
