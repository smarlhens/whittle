#![allow(clippy::unwrap_used, clippy::tests_outside_test_module)]

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

fn write_msg(dir: &Path, msg: &str) -> std::path::PathBuf {
    let p = dir.join("COMMIT_EDITMSG");
    fs::write(&p, msg).unwrap();
    p
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn fix_lowercases_and_replaces_and_with_amp() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "Chore: Bump oxfmt to 0.51.0 and oxlint to 1.66.0.\n",
    );

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();

    assert_eq!(read(&p), "chore: bump oxfmt to 0.51.0 & oxlint to 1.66.0\n");
}

#[test]
fn fix_normalizes_scope_separator() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "fix(api/users): Handle [null] / undefined ids\n",
    );

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();

    assert_eq!(read(&p), "fix(api-users): handle null undefined ids\n");
}

#[test]
fn fix_keeps_version_dots() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "chore: bump foo 1.2.3 and bar 4.5.6\n");

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();

    assert_eq!(read(&p), "chore: bump foo 1.2.3 & bar 4.5.6\n");
}

#[test]
fn fix_drops_body_and_trailers() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "feat(api): add health probe\n\nLonger body that explains why.\n\nCo-Authored-By: Foo <foo@bar.com>\n",
    );

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();

    assert_eq!(read(&p), "feat(api): add health probe\n");
}

#[test]
fn check_fails_on_too_long_subject() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "feat: this is a really long subject line that should exceed seventy two chars in total\n",
    );

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure()
        .stderr(predicates::str::contains("subject-too-long"));
}

#[test]
fn check_fails_on_disallowed_type() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "wip: hack something\n");

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure()
        .stderr(predicates::str::contains("disallowed-type"));
}

#[test]
fn check_fails_on_non_conventional() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "just update stuff\n");

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a conventional commit"));
}

#[test]
fn fix_then_check_round_trip() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Refactor: Split Module and Tidy.\n");

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();

    assert_eq!(read(&p), "refactor: split module & tidy\n");
}

#[test]
fn comments_are_ignored() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "# please enter the commit message for your changes\nfeat: add thing\n# Lines starting with # will be ignored.\n",
    );

    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();

    assert_eq!(read(&p), "feat: add thing\n");
}

#[test]
fn empty_file_is_noop() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "");
}

#[test]
fn whitespace_only_file_is_noop() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "   \n\n  \t \n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn comment_only_file_is_noop() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "# only a comment\n# another comment\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn subject_at_72_chars_passes() {
    let dir = TempDir::new().unwrap();
    let suffix: String = "x".repeat(72 - "feat: ".len());
    let p = write_msg(dir.path(), &format!("feat: {suffix}\n"));
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn subject_at_73_chars_fails() {
    let dir = TempDir::new().unwrap();
    let suffix: String = "x".repeat(73 - "feat: ".len());
    let p = write_msg(dir.path(), &format!("feat: {suffix}\n"));
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
}

#[test]
fn breaking_change_bang_preserved() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Feat!: Drop legacy API.\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "feat!: drop legacy api\n");
}

#[test]
fn breaking_change_with_scope_bang_preserved() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Feat(API)!: drop /v1 and /v2\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "feat(api)!: drop v1 & v2\n");
}

#[test]
fn check_does_not_modify_file() {
    let dir = TempDir::new().unwrap();
    let original = "Chore: Bump A and B.\n";
    let p = write_msg(dir.path(), original);
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), original);
}

#[test]
fn fix_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Refactor: Split MODULE and Tidy./\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let first = read(&p);
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(first, read(&p));
}

#[test]
fn unicode_description_preserved() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "feat: café résumé naïve\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "feat: café résumé naïve\n");
}

#[test]
fn missing_file_errors() {
    let dir = TempDir::new().unwrap();
    let p = dir.path().join("does-not-exist");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .failure();
}

#[test]
fn custom_config_keeps_body() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "[body]\nkeep = true\n").unwrap();
    let p = write_msg(dir.path(), "feat: x\n\nThis body must survive.\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let out = read(&p);
    assert!(out.contains("This body must survive."), "got: {out:?}");
}

#[test]
fn custom_config_allows_lower_max_length() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "[rules]\nmax_subject_length = 10\n").unwrap();
    let p = write_msg(dir.path(), "feat: hello world\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
}

#[test]
fn custom_config_keeps_footers_except_denied() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[footers]\nkeep = true\ndeny = [\"Co-Authored-By\"]\n",
    )
    .unwrap();
    let p = write_msg(
        dir.path(),
        "feat: x\n\nbody\n\nCo-Authored-By: a <a@x>\nRefs: #1\n",
    );
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let out = read(&p);
    assert!(!out.contains("Co-Authored-By"), "got: {out:?}");
    assert!(out.contains("Refs"), "got: {out:?}");
}

#[test]
fn custom_config_allow_extra_type() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[rules]\nallowed_types = [\"feat\", \"fix\", \"wip\"]\n",
    )
    .unwrap();
    let p = write_msg(dir.path(), "wip: experimental thing\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn custom_config_disables_lowercase() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[scope]\nlowercase = false\n[description]\nlowercase = false\n",
    )
    .unwrap();
    let p = write_msg(dir.path(), "fix(API): Handle Null\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let out = read(&p);
    assert!(out.contains("API"), "got: {out:?}");
    assert!(out.contains("Handle Null"), "got: {out:?}");
}

#[test]
fn invalid_config_path_errors() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "feat: x\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config", "/nonexistent/whittle.toml"])
        .args(["fix"])
        .arg(&p)
        .assert()
        .failure();
}

#[test]
fn malformed_config_errors() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "this is = not valid = toml\n").unwrap();
    let p = write_msg(dir.path(), "feat: x\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .args(["fix"])
        .arg(&p)
        .assert()
        .failure();
}

#[test]
fn file_with_no_trailing_newline_handled() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "feat: x");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "feat: x\n");
}

#[test]
fn help_flag_works() {
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("check"))
        .stdout(predicates::str::contains("fix"));
}

#[test]
fn version_flag_works() {
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicates::str::contains("whittle"));
}

#[test]
fn missing_subcommand_errors() {
    Command::cargo_bin("whittle").unwrap().assert().failure();
}

#[test]
fn check_passes_already_normalized() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "feat: add the thing\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn long_running_realistic_message() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "Feat(API/Users)!: Add /v2 endpoint and migrate clients.\n\nBody explaining motivation.\n\nCloses #128\nCo-Authored-By: A <a@x>\n",
    );
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let out = read(&p);
    assert!(
        out.starts_with("feat(api-users)!: add v2 endpoint & migrate clients"),
        "got: {out:?}"
    );
    assert!(!out.contains("Co-Authored-By"), "got: {out:?}");
    assert!(!out.contains("Body explaining"), "got: {out:?}");
}
