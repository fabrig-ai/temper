//! Process-tier fault injection at the shell's listen/drain boundary.
//! This module may abort its own process, never create or supervise one.

/// Cut one start before the shell submits the iteration announcing its listener.
/// Both test switches are required; an exclusive fixture marker makes the
/// box's next start continue without another cut.
pub(crate) fn cut_at_listen() {
    if std::env::var_os("TEMPER_R8_WAIT_CHECK").is_none() {
        return;
    }
    let Some(marker) = std::env::var_os("TEMPER_R8_CUT_AT_LISTEN") else { return };
    match std::fs::OpenOptions::new().write(true).create_new(true).open(marker) {
        Ok(marker) => {
            drop(marker);
            std::process::abort();
        }
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(why) => panic!("listen cut marker: {why}"),
    }
}
