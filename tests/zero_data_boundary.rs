//! Cargo-native required-data gate for the zero-data validation boundary.
//!
//! `ac_port` is a binary crate, so an integration test cannot import the
//! crate's internal `assets::find_game_dir`. This test therefore uses the same
//! public contract as `scripts/check-zero-data.sh`: a valid install is a
//! directory named by `AC_GAME_DIR` that contains `DataPC.forge`.
//!
//! In the default zero-data run this test is a no-op and the suite stays green.
//! With `AC_REQUIRE_GAME_DATA=1` it hard-fails when no install is present, so a
//! green required-data run cannot silently skip the native-data tests.
//!
//! See docs/VALIDATION.md for exactly what a green run certifies.

use std::path::PathBuf;

fn install_dir() -> Option<PathBuf> {
    std::env::var_os("AC_GAME_DIR").map(PathBuf::from)
}

fn install_present() -> bool {
    install_dir()
        .map(|dir| dir.join("DataPC.forge").is_file())
        .unwrap_or(false)
}

#[test]
fn required_data_mode_hard_fails_without_install() {
    let required = std::env::var("AC_REQUIRE_GAME_DATA").as_deref() == Ok("1");
    if !required {
        return;
    }
    assert!(
        install_present(),
        "AC_REQUIRE_GAME_DATA=1 but no valid AC1 install was found \
         (expected AC_GAME_DIR/DataPC.forge); refusing to pass without \
         exercising native-data tests"
    );
}
