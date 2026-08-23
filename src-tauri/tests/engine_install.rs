//! Actually downloads and installs the engine. Run explicitly; it hits the
//! network and writes into the real application data directory.
#[test]
#[ignore]
fn install_for_real() {
    plume_lib::engine::remove().expect("clean slate");
    assert!(plume_lib::engine::installed().is_none());

    let path = plume_lib::engine::install(&|step| eprintln!("STEP {step}"))
        .expect("install must succeed");

    eprintln!("INSTALLED {}", path.display());
    assert!(plume_lib::engine::installed().is_some());
}
