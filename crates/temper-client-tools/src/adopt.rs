//! Adoption's syntax; the forge form owns its semantic checks (client/tools.md, 5.3).
use crate::jig::table::{
    Keyed, Kind,
    Occurs::{Optional, Required},
    Tool, flag, repeated, switch,
};
pub const ROW: Tool = Tool {
    name: b"adopt",
    line: b"Adopt a repository",
    prints: b"",
    result: b"adopt",
    forms: &[&[
        flag(b"project", b"P", Kind::Project { empty: false }, Required, b"Project name or number"),
        flag(b"forge", b"F", Kind::Number, Optional, b"Forge number, within u16"),
        flag(b"owner", b"O", Kind::Text, Required, b"Repository owner"),
        flag(b"name", b"N", Kind::Text, Required, b"Repository name"),
        flag(b"landing", b"B", Kind::Text, Required, b"Landing branch"),
        flag(b"role", b"ROLE", Kind::Word { set: &[b"owned", b"fork", b"context"] }, Optional, b"Repository role"),
        flag(b"prefix", b"X", Kind::Text, Optional, b"Branch prefix"),
        switch(b"no-home", b"Do not make the repository home"),
        switch(b"no-ci", b"Do not use forge CI"),
        repeated(b"check", b"G", Kind::Number, 8, b"Gate number"),
    ]],
    keyed: Keyed::Required,
    area: 1,
    notes: b"",
};
