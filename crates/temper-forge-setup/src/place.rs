//! Owned files under one absolute, private forge directory (protocol/setup.md, 3).
use crate::check::joined;
use alloc::boxed::Box;
/// Where a forge lives, and the user that runs its processes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Place {
    pub directory: Box<[u8]>, // Absolute forge directory.
    pub user: Box<[u8]>,      // Operating-system login.
}
/// One regular file of the forge's directory.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum File {
    /// Forgejo's settings.
    AppIni,
    /// Prepared versions and Forgejo's secrets.
    State,
    /// Temper's site admin's token.
    AdminToken,
    /// The person's first password.
    PersonPassword,
    /// The person's read-only API authorization header.
    PersonReadHeader,
    /// The person's writing API authorization header.
    PersonWriteHeader,
    /// The bot's API token.
    ApiToken,
    /// The bot's git token.
    GitToken,
    /// The bot's decimal user id; the User item crosses as its 8 little-endian bytes.
    User,
    /// The hooks' shared secret.
    WebhookSecret,
    /// The runner connection's token.
    RunnerSecret,
    /// The runner's connection and jobs' settings.
    RunnerConfiguration,
    /// An upgrade dump before its final rename.
    DumpPartial,
    /// Held by Forgejo and the preparation.
    ForgejoLock,
    /// Held by the runner and preparations that write its files.
    RunnerLock,
}
impl Place {
    /// The file's absolute path.
    #[must_use]
    pub fn path(&self, file: File) -> Box<[u8]> {
        let name = match file {
            File::DumpPartial => b"backups/dump.partial.zip".as_slice(),
            File::AppIni => b"forgejo/app.ini".as_slice(),
            File::State => b"state",
            File::AdminToken => b"admin_token",
            File::PersonPassword => b"person_password",
            File::PersonReadHeader => b"person_read.header",
            File::PersonWriteHeader => b"person_write.header",
            File::ApiToken => b"api_token",
            File::GitToken => b"git_token",
            File::User => b"user",
            File::WebhookSecret => b"webhook_secret",
            File::RunnerSecret => b"runner/secret",
            File::RunnerConfiguration => b"runner/config.yaml",
            File::ForgejoLock => b"forgejo.lock",
            File::RunnerLock => b"runner.lock",
        };
        joined(&[&self.directory, b"/", name])
    }
    /// A named backup under the private backup directory.
    #[must_use]
    pub fn backup(&self, name: &[u8]) -> Box<[u8]> {
        joined(&[&self.directory, b"/backups/", name])
    }
    /// Forgejo's work path.
    #[must_use]
    pub fn work_path(&self) -> Box<[u8]> {
        joined(&[&self.directory, b"/forgejo"])
    }
}

/// Maps the kit's retained item to its private file in temper.
#[must_use]
pub fn item(item: skein_forge_setup::item::Item) -> File {
    use skein_forge_setup::item::Item;
    match item {
        Item::AppId | Item::AppSlug | Item::AppKey | Item::Accepted | Item::Pasted => {
            unreachable!(
                "items come only from in-process Forgejo Reconcile, never files, stores or peers; temper-01/13 take GitHub records"
            )
        }
        Item::AdminToken => File::AdminToken,
        Item::ApiToken => File::ApiToken,
        Item::GitToken => File::GitToken,
        Item::User => File::User,
        Item::WebhookSecret => File::WebhookSecret,
    }
}
