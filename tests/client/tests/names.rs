//! temper's client crates live under `temper-client` (jig's README.md, 15).

#[test]
fn client_crates_carry_the_client_prefix() {
    // Tests run from their package's directory, `tests/client`.
    let root = std::env::current_dir().expect("package directory").join("../..");
    let root = root.canonicalize().expect("temper's root");
    let breaches = jig_boundary::client_names::breaches(
        &root,
        "temper",
        // Whole-application worlds host the client beside the engine and peers.
        &[
            "temper",
            "temper-one-process-world",
            "temper-real-loop-world",
            "temper-playground-world",
            "temper-playground",
        ],
    );
    assert!(breaches.is_empty(), "client crates outside temper-client: {}", breaches.join(", "));
}
