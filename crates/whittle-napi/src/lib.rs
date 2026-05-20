#![deny(clippy::all)]
// NAPI exports are consumed by the C ABI, not Rust — dead_code is a false positive for cdylib.
#![allow(dead_code)]
// NAPI #[napi] functions require owned String parameters, not &str.
#![allow(clippy::needless_pass_by_value)]

use napi_derive::napi;

/// Run the `whittle` CLI in-process. `argv` must include the program name at
/// index 0 (e.g. `["whittle", "fix", ".git/COMMIT_EDITMSG"]`). Returns the exit code.
#[napi]
#[must_use]
pub fn run_cli(argv: Vec<String>) -> i32 {
    whittle::run_cli(argv)
}
