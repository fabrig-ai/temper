//! The synchronous forge configuration shared by git's fixtures and worlds.
use skein_fake_forge_domain::{Config, Limits, Skew};
use skein_lib::Duration;

const CI: u64 = 3;

/// Room for what the worlds put on the forge: a few repositories, the
/// branches of their workstreams, and every commit their runs make. Nothing
/// fills: a forge that refused for want of room would fail the world.
pub const LIMITS: Limits = Limits {
    repositories: 16,
    users: 4,
    labels: 1,
    items: 1,
    comments: 1,
    dependencies: 1,
    reviews: 1,
    branches: 64,
    commits: 4096,
    files: 256,
    statuses: 1,
    contexts: 1,
    bypassing_users: 0,
    pages: 1,
    name_bytes: 256,
    title_bytes: 16,
    body_bytes: 32,
    content_bytes: 65_536,
    page_size: 1,
    calls: 1,
    hooks: 1,
    observations: 16,
};

/// A forge that answers at once and fails only as a world scripts it.
pub const CONFIG: Config = Config {
    limits: LIMITS,
    latency_min: Duration::ZERO,
    latency_max: Duration::ZERO,
    settle_min: Duration::from_secs(2),
    settle_max: Duration::from_secs(2),
    late: 0,
    late_min: Duration::ZERO,
    late_max: Duration::ZERO,
    unavailable: 0,
    timeouts: 0,
    landing: 0,
    land_min: Duration::ZERO,
    land_max: Duration::ZERO,
    rate_limit: 0,
    rate_window: Duration::ZERO,
    ci: CI,
    hook_min: Duration::ZERO,
    hook_max: Duration::ZERO,
    hooks_late: 0,
    hooks_lost: 0,
    resolution: Duration::ZERO,
    skew: Skew::None,
    status_updates: false,
    edit_updates: false,
};
