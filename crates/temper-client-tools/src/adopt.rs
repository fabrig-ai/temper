//! Adoption's syntax; the forge form owns its semantic checks (client/tools.md, 5.3).
use crate::jig::table::{
    Keyed, Kind,
    Occurs::{Optional, Required},
    Tool, flag, repeated, switch,
};
pub const ROW: Tool = Tool {
    name: b"repository adopt",
    line: b"Adopt a forge's repository into a project",
    prints: b"`Adopted OWNER/NAME on forge N as ROLE.`, whether it is the project's home, and what temper may do there",
    result: b"adopt",
    forms: &[&[
        flag(b"project", b"PROJECT", Kind::Project { empty: false }, Required, b"The project"),
        flag(b"forge", b"FORGE", Kind::Number, Optional, b"The forge's number; 1 when absent"),
        flag(b"owner", b"OWNER", Kind::Text, Required, b"The repository's owner on the forge"),
        flag(b"name", b"REPOSITORY", Kind::Text, Required, b"The repository's name"),
        flag(b"landing", b"BRANCH", Kind::Text, Required, b"The branch its changes land on"),
        flag(b"role", b"owned|fork|context", Kind::Word { set: &[b"owned", b"fork", b"context"] }, Optional, b"owned, fork or context; owned when absent"),
        flag(b"prefix", b"PREFIX", Kind::Text, Optional, b"The prefix of temper's branches; temper/ when absent"),
        switch(b"no-home", b"Do not make it the project's home"),
        switch(b"no-ci", b"Do not use the forge's CI"),
        repeated(b"check", b"GATE", Kind::Number, 8, b"A gate, by its number, each once"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"A project has one home; a context repository is read, never\nwritten, and never the home. A refusal says what mends it: another\nproject writes the repository (adopt it as context there), the\nproject has a home already (add --no-home, or choose a new\nproject), or the branch prefix is another deployment's (choose an\nunused one); the last two print the call to make, with a new key.\n`It is not known whether OWNER/NAME was adopted.` (exit 1): make\nthe same call again, with the same key.\n",
};
