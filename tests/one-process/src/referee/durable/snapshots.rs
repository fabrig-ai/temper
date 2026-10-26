//! Independently decode the snapshot read on restart; snapshot chunks are not commit frames.
use jig_protocol_store::transcripts::frame::crc32c;
use skein_lib::Reader;

pub(crate) fn read(bytes: &[u8]) -> Option<(u64, Box<[skein_kv::Op]>)> {
    if bytes.len() < 46 || !bytes.ends_with(b"END!") {
        return None;
    }
    let mut reader = Reader::new(bytes);
    assert_eq!(reader.bytes(4), Some(b"SKVS".as_slice()), "snapshot magic");
    assert_eq!(reader.u16(), Some(1), "snapshot version");
    let start = reader.u64().expect("snapshot start");
    assert_eq!(reader.u32(), Some(crc32c(&bytes[..14])), "snapshot header checksum");
    let mut rows = Vec::new();
    let mut payload = 0_u64;
    loop {
        let at = bytes.len() - reader.remaining() as usize;
        if bytes.get(at..at + 4) == Some(b"SNPE".as_slice()) {
            reader.skip(4).expect("snapshot trailer");
            assert_eq!(reader.u64(), Some(rows.len() as u64), "snapshot row count");
            assert_eq!(reader.u64(), Some(payload), "snapshot payload length");
            assert_eq!(reader.u32(), Some(crc32c(&bytes[at..at + 20])), "snapshot trailer checksum");
            assert_eq!(reader.bytes(4), Some(b"END!".as_slice()), "snapshot ending");
            assert_eq!(reader.remaining(), 0, "whole snapshot");
            return Some((start.checked_sub(1).expect("positive snapshot start"), rows.into_boxed_slice()));
        }
        let count = reader.u32().expect("snapshot chunk rows");
        let len = reader.u32().expect("snapshot chunk body") as usize;
        let end = at + 8 + len;
        for _ in 0..count {
            let key_len = reader.u32().expect("snapshot key length");
            let value_len = reader.u32().expect("snapshot value length");
            let key = reader.bytes(key_len).expect("snapshot key").into();
            let value = reader.bytes(value_len).expect("snapshot value").into();
            payload += u64::from(key_len) + u64::from(value_len);
            rows.push(skein_kv::Op::Put { key, value });
        }
        assert_eq!(bytes.len() - reader.remaining() as usize, end, "whole snapshot chunk");
        assert_eq!(reader.u32(), Some(crc32c(&bytes[at..end])), "snapshot chunk checksum");
    }
}
