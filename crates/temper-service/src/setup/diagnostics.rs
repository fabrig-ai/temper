//! The wanted names alone reach the operator's log (protocol/setup.md, 17.5; call 7).
use alloc::boxed::Box;
use skein_forge::github::document::Access;
use skein_forge_setup::github::Lacking;
use skein_lib::Writer;

fn parts(lack: &Lacking) -> (&'static [u8], &[u8], &'static [u8]) {
    match lack {
        Lacking::Permission { name, access } => (b"permission ", name, access_text(*access)),
        Lacking::Event { name } => (b"event ", name, b"; "),
    }
}
fn access_text(access: Access) -> &'static [u8] {
    match access {
        Access::Read => b"=read; ",
        Access::Write => b"=write; ",
        Access::Admin => b"=admin; ",
    }
}
pub(super) fn names(lacking: &[Lacking]) -> Box<[u8]> {
    let mut bytes = 0_usize;
    for lack in lacking {
        let (prefix, name, suffix) = parts(lack);
        bytes = bytes.checked_add(prefix.len()).expect("wanted diagnostic");
        bytes = bytes.checked_add(name.len()).expect("wanted names");
        bytes = bytes.checked_add(suffix.len()).expect("wanted access");
    }
    let mut out = Writer::new(bytes);
    for lack in lacking {
        let (prefix, name, suffix) = parts(lack);
        out.put(prefix).expect("measured prefix");
        out.put(name).expect("measured name");
        out.put(suffix).expect("measured access");
    }
    out.finish()
}
