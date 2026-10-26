//! temper's own page content.
use alloc::boxed::Box;
mod account_programs;
pub(crate) mod accounts;
pub(crate) mod agent;
pub(crate) mod change;
pub(crate) mod chat;
pub(crate) mod chats;
pub(crate) mod connect_model;
pub(crate) mod inbox;
pub(crate) mod task;
pub(crate) mod welcome;

pub(crate) fn project_name(
    me: Option<&jig_client::contract::Me>,
    demo: Option<&temper_client_domain::forge::Demo>,
    project: u32,
    words: &crate::words::Words,
) -> Option<Box<[u8]>> {
    if let Some(demo) = demo
        && demo.project == project
    {
        return Some(words.say(crate::words::Word::Demo, &[]));
    }
    for row in &me?.projects {
        if row.project == project {
            return Some(skein_lib::bytes::copy_of(row.name.bytes()));
        }
    }
    None
}
