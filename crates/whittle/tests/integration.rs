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
        .stderr(predicates::str::contains("subject.too-long"));
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
        .stderr(predicates::str::contains("type.disallowed"));
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
    // Pending rewrites make `check` fail, but it must never write.
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    assert_eq!(read(&p), original);
}

#[test]
fn check_passes_on_already_normalized_message() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "chore: bump a & b\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn check_reports_pending_rewrites_and_suggests_message() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("description.lowercased"), "got: {stderr}");
    assert!(stderr.contains("type.lowercased"), "got: {stderr}");
    assert!(stderr.contains("suggested message:"), "got: {stderr}");
    assert!(stderr.contains("chore: bump a & b"), "got: {stderr}");
}

#[test]
fn check_reports_dropped_body_and_denied_footer() {
    let dir = TempDir::new().unwrap();
    let msg = "feat: add probe\n\nwhy this change\n\nCo-Authored-By: a <a@b>\n";
    let p = write_msg(dir.path(), msg);
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("body.dropped"), "got: {stderr}");
    assert!(stderr.contains("would be removed"), "got: {stderr}");
    assert_eq!(read(&p), msg, "check must not write");
}

#[test]
fn fix_reports_what_it_changed_instead_of_staying_silent() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "feat: add probe\n\nwhy this change\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    // Past tense: the rewrite is already on disk.
    assert!(stderr.contains("body.dropped"), "got: {stderr}");
    assert!(stderr.contains("body removed"), "got: {stderr}");
    assert!(!stderr.contains("would be removed"), "got: {stderr}");
    assert_eq!(read(&p), "feat: add probe\n");
}

#[test]
fn json_report_carries_typed_params_and_normalized_message() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n\nbody text\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(v["version"], 1);
    assert_eq!(v["mode"], "check");
    assert_eq!(v["ok"], false);
    assert_eq!(v["normalized"], "chore: bump a & b");

    let diags = v["diagnostics"].as_array().unwrap();
    let body = diags
        .iter()
        .find(|d| d["code"] == "body.dropped")
        .expect("body.dropped diagnostic");
    assert_eq!(body["severity"], "warning");
    assert_eq!(body["field"], "body");
    assert_eq!(body["before"], "body text");
    assert!(body["after"].is_null(), "removal has no `after`");
    assert_eq!(body["message"], "body would be removed (1 line)");
}

#[test]
fn json_report_is_ok_for_a_clean_message() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "chore: bump a & b\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert!(v["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn json_report_marks_rule_violations_as_errors() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "wip: hack\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let diags = v["diagnostics"].as_array().unwrap();
    let d = diags
        .iter()
        .find(|d| d["code"] == "type.disallowed")
        .expect("type.disallowed diagnostic");
    assert_eq!(d["severity"], "error");
    assert_eq!(d["field"], "type");
}

#[test]
fn json_report_covers_non_conventional_messages() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "just some words\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let diags = v["diagnostics"].as_array().unwrap();
    assert_eq!(diags[0]["code"], "commit.not-conventional");
    assert_eq!(diags[0]["severity"], "error");
    // No valid normalization exists, so there is nothing to adopt. Echoing the
    // input back would send an agent into an infinite retry loop.
    assert!(v["normalized"].is_null(), "got: {}", v["normalized"]);
}

// --- git commit -v / scissors ------------------------------------------------

/// Reproduces what a real `commit-msg` hook receives under `commit.verbose`:
/// git appends the diff below the scissors line *without* commenting it out.
fn verbose_commit_msg(subject: &str) -> String {
    format!(
        "{subject}\n\n\
         # Please enter the commit message for your changes.\n\
         # ------------------------ >8 ------------------------\n\
         # Do not modify or remove the line above.\n\
         # Everything below it will be ignored.\n\
         diff --git a/f.txt b/f.txt\n\
         new file mode 100644\n\
         index 0000000..ce01362\n\
         --- /dev/null\n\
         +++ b/f.txt\n\
         @@ -0,0 +1 @@\n\
         +hello\n"
    )
}

#[test]
fn check_passes_a_verbose_commit_whose_subject_is_clean() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), &verbose_commit_msg("feat: add f"));
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
}

#[test]
fn verbose_commit_diff_is_not_treated_as_a_body() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), &verbose_commit_msg("feat: add f"));
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert_eq!(v["original"], "feat: add f");
    assert!(
        !v.to_string().contains("diff --git"),
        "the diff must not leak into the report"
    );
    assert!(v["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn verbose_commit_still_reports_a_real_subject_problem() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), &verbose_commit_msg("Feat: Add F."));
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("suggested message:"), "got: {stderr}");
    assert!(stderr.contains("feat: add f"), "got: {stderr}");
    assert!(!stderr.contains("body.dropped"), "got: {stderr}");
}

// --- ref-style trailers ------------------------------------------------------

#[test]
fn ref_style_trailers_are_not_corrupted_by_fix() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[body]\nkeep = true\n[footers]\nkeep = true\ndeny = []\n",
    )
    .unwrap();
    let p = write_msg(dir.path(), "feat: x\n\nbody here\n\nCloses #128\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("fix")
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "feat: x\n\nbody here\n\nCloses #128\n");
}

#[test]
fn check_agrees_with_fix_on_ref_style_trailers() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[body]\nkeep = true\n[footers]\nkeep = true\ndeny = []\n",
    )
    .unwrap();
    let msg = "feat: x\n\nbody here\n\nCloses #128\n";
    let p = write_msg(dir.path(), msg);
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), msg);
}

// --- normalization that would not parse --------------------------------------

#[test]
fn fix_refuses_to_write_a_message_that_would_not_parse() {
    let dir = TempDir::new().unwrap();
    let original = "fix: [...]\n";
    let p = write_msg(dir.path(), original);
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("normalize.invalid-result"), "got: {stderr}");
    assert_eq!(read(&p), original, "file must be left untouched");
}

#[test]
fn check_offers_no_suggestion_when_normalization_would_not_parse() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: [...]\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert!(v["normalized"].is_null());
    assert_eq!(v["ok"], false);
}

#[test]
fn invalid_result_path_still_reports_a_rule_violation() {
    // Before this fix, an invalid-result return skipped lint() entirely: the
    // author fixed the brackets, resubmitted, and was rejected a second time
    // for `type.disallowed` — a violation whittle already knew about.
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "wip: [...]\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("normalize.invalid-result"), "got: {stderr}");
    assert!(stderr.contains("type.disallowed"), "got: {stderr}");
}

#[test]
fn json_normalized_is_withheld_while_a_rule_violation_remains() {
    // `Wip:` normalizes to `wip:`, which still violates `type.disallowed` —
    // adopting `normalized` verbatim would fail check again immediately.
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Wip: Hack Something.\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert!(v["normalized"].is_null(), "got: {}", v["normalized"]);
}

#[test]
fn structural_only_reformatting_gets_a_diagnostic_and_guidance() {
    // Extra blank lines before the body change `render()`'s output with no
    // `transform_*` step reporting why — previously this made `check` fail
    // with empty diagnostics and empty guidance, an unexplained rejection.
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "[body]\nkeep = true\n[footers]\nkeep = true\n").unwrap();
    let p = write_msg(dir.path(), "feat: add probe\n\n\n\nwhy this matters\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("message.reformatted"), "got: {stderr}");
    assert!(
        stderr.contains("how to write a message whittle accepts:"),
        "got: {stderr}"
    );
    assert!(stderr.contains("suggested message:"), "got: {stderr}");
}

#[test]
fn json_report_is_never_empty_on_the_structural_reformat_path() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "[body]\nkeep = true\n[footers]\nkeep = true\n").unwrap();
    let p = write_msg(dir.path(), "feat: add probe\n\n\n\nwhy this matters\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("--format")
        .arg("json")
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert!(!v["diagnostics"].as_array().unwrap().is_empty());
    assert!(!v["guidance"].as_array().unwrap().is_empty());
}

// --- output survives a reader that hangs up early -----------------------------

#[test]
fn json_output_does_not_panic_on_a_broken_pipe() {
    // `println!` panics on EPIPE because Rust ignores SIGPIPE; a CLI meant to
    // be piped into `head`/`grep -q` must not crash with exit code 101.
    //
    // Reading a single byte then dropping the handle (rather than dropping it
    // unread right after spawn) closes the pipe deterministically once the
    // child has *started* writing, without racing spawn latency — a `head -c
    // 1`-style reader, done in-process so the child's own exit status is the
    // one under test rather than a shell pipeline's.
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n");
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("whittle"))
        .args(["--format", "json", "check"])
        .arg(&p)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Read;
        let mut buf = [0_u8; 1];
        let _ = child.stdout.take().unwrap().read_exact(&mut buf);
    } // dropped here, closing our end of the pipe
    let status = child.wait().unwrap();
    assert_ne!(status.code(), Some(101), "must not panic on a closed pipe");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_ne!(status.signal(), Some(13), "must not be killed by SIGPIPE");
    }
}

// --- transforms that cancel out ----------------------------------------------

#[test]
fn check_passes_when_replace_rules_cancel_out() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[description]\nreplace = [\n  { from = \"foo\", to = \"bar\" },\n  \
         { from = \"bar\", to = \"foo\" },\n]\n",
    )
    .unwrap();
    // Net effect is zero, so there is nothing for the caller to change and the
    // hook must not reject the commit forever.
    let p = write_msg(dir.path(), "feat: foo\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .success();
}

// --- suggestion is only offered when it would be accepted --------------------

#[test]
fn check_withholds_suggestion_when_a_rule_violation_remains() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Wip: Hack Something.\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("type.disallowed"), "got: {stderr}");
    assert!(
        !stderr.contains("suggested message:"),
        "a suggestion that fails again must not be offered: {stderr}"
    );
}

// --- json error paths --------------------------------------------------------

#[test]
fn json_report_is_emitted_for_a_missing_file() {
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check", "/nonexistent/COMMIT_EDITMSG"])
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("stdout must be valid json");
    assert_eq!(v["ok"], false);
    assert_eq!(v["diagnostics"][0]["code"], "whittle.failed");
}

#[test]
fn json_report_is_emitted_for_a_malformed_config() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "this is not valid toml = = =\n").unwrap();
    let p = write_msg(dir.path(), "feat: x\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("stdout must be valid json");
    assert_eq!(v["ok"], false);
    assert_eq!(v["diagnostics"][0]["code"], "whittle.failed");
}

// --- json fix mode -----------------------------------------------------------

#[test]
fn json_fix_report_agrees_with_its_exit_code() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n\nbody\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "fix"])
        .arg(&p)
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert_eq!(v["mode"], "fix");
    // Exit code was 0, so `ok` must be true even though rewrites were reported.
    assert_eq!(v["ok"], true);
    assert_eq!(v["normalized"], "chore: bump a & b");
    let diags = v["diagnostics"].as_array().unwrap();
    assert!(!diags.is_empty());
    assert!(
        diags.iter().all(|d| d["applied"] == true),
        "fix-mode diagnostics are already on disk"
    );
    assert_eq!(read(&p), "chore: bump a & b\n");
}

#[test]
fn json_fix_report_is_not_ok_when_a_rule_is_violated() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "wip: hack\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "fix"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert_eq!(v["ok"], false);
}

// --- diagnostic shape --------------------------------------------------------

#[test]
fn json_diagnostics_distinguish_removal_from_violation() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "wip: Hack.\n\nbody text\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    let diags = v["diagnostics"].as_array().unwrap();

    let body = diags.iter().find(|d| d["code"] == "body.dropped").unwrap();
    assert_eq!(body["kind"], "removal");
    assert!(body["after"].is_null());

    let violation = diags
        .iter()
        .find(|d| d["code"] == "type.disallowed")
        .unwrap();
    // Same `after: null`, but nothing is deleted — `kind` is what tells them apart.
    assert_eq!(violation["kind"], "violation");
    assert!(violation["after"].is_null());

    let rewrite = diags
        .iter()
        .find(|d| d["code"] == "description.lowercased")
        .unwrap();
    assert_eq!(rewrite["kind"], "rewrite");
    assert!(rewrite["after"].is_string());
}

#[test]
fn rewrite_messages_distinguish_pending_from_applied() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "feat: Add Thing\n");
    let checked = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let pending = String::from_utf8_lossy(&checked.get_output().stderr).to_string();

    let fixed = Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let applied = String::from_utf8_lossy(&fixed.get_output().stderr).to_string();

    assert!(pending.contains("description:"), "got: {pending}");
    assert!(applied.contains("description rewritten:"), "got: {applied}");
    assert_ne!(pending, applied, "check and fix must not read identically");
}

// --- guidance: teaching the rules ---------------------------------------------

#[test]
fn check_explains_the_rules_it_tripped() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "ci: restore actions/checkout@v6 in release.yml\n\nwhy\n\nCo-Authored-By: A <a@b>\n",
    );
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        stderr.contains("how to write a message whittle accepts:"),
        "got: {stderr}"
    );
    // Prescriptive, not just a diff of what changed.
    assert!(stderr.contains("only between digits"), "got: {stderr}");
    assert!(
        stderr.contains("do not use these characters"),
        "got: {stderr}"
    );
    assert!(stderr.contains("whittle rules"), "got: {stderr}");
}

#[test]
fn lossy_transforms_are_spelled_out_to_the_caller() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "feat: add probe\n\nlong rationale\n\nCo-Authored-By: A <a@b>\n",
    );
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("body is discarded"), "got: {stderr}");
    assert!(stderr.contains("Co-Authored-By"), "got: {stderr}");
}

#[test]
fn fix_also_explains_the_rules_so_the_caller_learns() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Chore: Bump readme.md and stuff.\n");
    // `fix` succeeds, but the caller still needs to know why it was rewritten:
    // the Bash tool result carries stderr even on exit 0.
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        stderr.contains("how to write a message whittle accepts:"),
        "got: {stderr}"
    );
}

#[test]
fn a_clean_message_gets_no_guidance_noise() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "chore: bump a & b\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert_eq!(stderr, "", "a compliant message must print nothing");
}

#[test]
fn json_report_carries_guidance() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "Chore: Bump readme.md.\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    let g = v["guidance"].as_array().expect("guidance array");
    assert!(!g.is_empty());
    assert!(
        g.iter().any(|l| l.as_str().unwrap().contains("lowercase")),
        "got: {g:?}"
    );
}

// --- check demands a rewording instead of offering a mangled subject ---------

#[test]
fn check_demands_rewording_and_offers_no_mangled_suggestion() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: handle src/lib.rs parse error\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        stderr.contains("`src/lib.rs` would become `srclibrs`"),
        "the ruined token must be named: {stderr}"
    );
    assert!(
        !stderr.contains("suggested message:"),
        "an illegible subject must never be offered for adoption: {stderr}"
    );
}

#[test]
fn check_names_every_mangled_token() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(
        dir.path(),
        "ci: restore actions/checkout@v6 in release.yml\n",
    );
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("`actions/checkout@v6`"), "got: {stderr}");
    assert!(stderr.contains("`release.yml`"), "got: {stderr}");
}

#[test]
fn cosmetic_rewrites_still_get_a_suggestion() {
    let dir = TempDir::new().unwrap();
    // Lowercasing, `and` -> `&` and a trailing dot are all legible outcomes, so
    // adopting the suggestion is the right move here.
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("suggested message:"), "got: {stderr}");
    assert!(stderr.contains("chore: bump a & b"), "got: {stderr}");
    assert!(!stderr.contains("needs-rewording"), "got: {stderr}");
}

#[test]
fn fix_still_applies_the_configured_rules_unchanged() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: handle src/lib.rs parse error\n");
    // The rewording demand is a `check` concern. `fix` is the human path and
    // keeps enforcing the house style exactly as before.
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "fix: handle srclibrs parse error\n");
}

#[test]
fn json_report_marks_rewording_as_a_violation_per_token() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: handle src/lib.rs parse error\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "subject.needs-rewording")
        .expect("subject.needs-rewording");
    assert_eq!(d["severity"], "error");
    // A violation, not a rewrite: `after` shows the ruin, it is not adoptable.
    assert_eq!(d["kind"], "violation");
    assert_eq!(d["before"], "src/lib.rs");
    assert_eq!(d["after"], "srclibrs");
}

#[test]
fn version_pins_are_not_treated_as_mangling() {
    // These normalize to something readable and have no compliant rewording that
    // keeps the version, so blocking them would make the commit unpassable.
    for msg in [
        "ci: pin node 24.x\n",
        "chore: bump serde to 1.2.3-rc.1\n",
        "fix: drop the 0.x fallback\n",
    ] {
        let dir = TempDir::new().unwrap();
        let p = write_msg(dir.path(), msg);
        let out = Command::cargo_bin("whittle")
            .unwrap()
            .args(["check"])
            .arg(&p)
            .assert();
        let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
        assert!(
            !stderr.contains("needs-rewording"),
            "{msg:?} must not demand a rewording: {stderr}"
        );
    }
}

#[test]
fn a_cosmetic_whole_span_change_is_not_mangling() {
    // Token counts differ (the lone `.` vanishes), which must not bypass the
    // legibility test nor quote a mid-pipeline value.
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: bump a & b .\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(!stderr.contains("needs-rewording"), "got: {stderr}");
    assert!(stderr.contains("suggested message:"), "got: {stderr}");
    assert!(stderr.contains("fix: bump a & b"), "got: {stderr}");
}

#[test]
fn reworded_tokens_quote_the_authored_spelling() {
    // Lowercasing runs before the stripping, so a naive implementation would
    // quote `some/path.md` — a string the author never wrote.
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "docs: DESCRIBE Some/Path.md\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("`Some/Path.md`"), "got: {stderr}");
}

#[test]
fn json_withholds_normalized_when_a_rewording_is_needed() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: handle src/lib.rs parse error\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    // The human output refuses to print it, so the JSON contract must not ship it.
    assert!(v["normalized"].is_null(), "got: {}", v["normalized"]);
}

#[test]
fn a_replace_rule_causing_fusion_gets_an_ordinary_suggestion() {
    // A `.replace` rule is the project's own deliberate choice — the author
    // wrote the (possibly empty) `to` value — so the rewording gate does not
    // second-guess it the way it does whittle's own strip_chars/internal_dots.
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[description]\nstrip_chars = []\nreplace = [{ from = \":\", to = \"\" }]\n",
    )
    .unwrap();
    let p = write_msg(dir.path(), "fix: handle src:lib error\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(!stderr.contains("needs-rewording"), "got: {stderr}");
    assert!(stderr.contains("suggested message:"), "got: {stderr}");
    assert!(stderr.contains("fix: handle srclib error"), "got: {stderr}");
}

// --- guidance is derived from config, never hardcoded -------------------------

#[test]
fn guidance_describes_a_custom_scope_replace_rule() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "[scope]\nreplace = [{ from = \"_\", to = \"-\" }]\n").unwrap();
    let p = write_msg(dir.path(), "chore(a_b): bump a & b\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    // Reflects this specific diagnostic's own before/after, not the whole
    // configured table — a project with several scope rules should not be
    // told about ones that did not fire on this message.
    assert!(
        stderr.contains("in the scope, `a_b` becomes `a-b`"),
        "got: {stderr}"
    );
    // The default `/` and `\` rule does not apply to this config.
    assert!(
        !stderr.contains("do not use `/` or `\\` in the scope"),
        "got: {stderr}"
    );
}

#[test]
fn guidance_does_not_contradict_an_inverted_replace_rule() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[description]\nreplace = [{ from = \"&\", to = \"and\" }]\n",
    )
    .unwrap();
    let p = write_msg(dir.path(), "chore: bump a & b\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        stderr.contains("in the description, `bump a & b` becomes `bump a and b`"),
        "got: {stderr}"
    );
    assert!(
        !stderr.contains("`&` not `and`"),
        "must not advise the opposite of the config: {stderr}"
    );
}

#[test]
fn denied_footer_guidance_does_not_claim_all_footers_are_dropped() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[body]\nkeep = true\n[footers]\nkeep = true\ndeny = [\"Co-Authored-By\"]\n",
    )
    .unwrap();
    let p = write_msg(
        dir.path(),
        "feat: x\n\nbody\n\nRefs: #12\nCo-Authored-By: A <a@b>\n",
    );
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(
        stderr.contains("these footers are discarded: Co-Authored-By"),
        "got: {stderr}"
    );
    assert!(
        !stderr.contains("do not rely on trailers"),
        "Refs: trailers are kept by this config: {stderr}"
    );
}

#[test]
fn the_ruleset_hint_carries_the_active_config() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(&cfg, "[rules]\nmax_subject_length = 50\n").unwrap();
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("check")
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(stderr.contains("--config"), "got: {stderr}");
    assert!(stderr.contains("rules"), "got: {stderr}");
}

#[test]
fn invalid_result_guidance_names_the_offending_characters() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "fix: [...]\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["check"])
        .arg(&p)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    // Without this the author is told the description is empty but never which
    // characters emptied it, and there is no suggestion to infer it from.
    assert!(
        stderr.contains("do not use these characters"),
        "got: {stderr}"
    );
    assert!(stderr.contains("only between digits"), "got: {stderr}");
}

// --- whittle rules -----------------------------------------------------------

#[test]
fn rules_subcommand_lists_the_active_ruleset() {
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["rules"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains("Conventional Commit"), "got: {stdout}");
    assert!(stdout.contains("72 characters"), "got: {stdout}");
    assert!(stdout.contains("body is discarded"), "got: {stdout}");
}

#[test]
fn rules_subcommand_reflects_a_custom_config() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[rules]\nmax_subject_length = 50\nallowed_types = [\"feat\", \"fix\"]\n[body]\nkeep = true\n",
    )
    .unwrap();
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("rules")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains("50 characters"), "got: {stdout}");
    assert!(stdout.contains("feat, fix"), "got: {stdout}");
    assert!(!stdout.contains("body is discarded"), "got: {stdout}");
}

#[test]
fn rules_subcommand_emits_json() {
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "rules"])
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert_eq!(v["version"], 1);
    assert_eq!(v["ok"], true);
    assert!(!v["rules"].as_array().unwrap().is_empty());
}

#[test]
fn rules_subcommand_lists_the_replace_tables() {
    // Omitting these was a silent drift: an agent following the printed rules
    // wrote `and`, which check then rejected under description.replaced.
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["rules"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains("in the description"), "got: {stdout}");
    assert!(stdout.contains("in the scope"), "got: {stdout}");
}

#[test]
fn rules_subcommand_omits_checks_the_config_disables() {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("whittle.toml");
    fs::write(
        &cfg,
        "[rules]\nallowed_types = []\nrequire_conventional = false\n",
    )
    .unwrap();
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--config"])
        .arg(&cfg)
        .arg("rules")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    // An empty types list means the check is skipped, not that nothing is allowed.
    assert!(!stdout.contains("use one of these types"), "got: {stdout}");
    assert!(!stdout.contains("Conventional Commit"), "got: {stdout}");
}

#[test]
fn rules_subcommand_emits_json_for_a_bad_config() {
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "rules", "--config", "/nonexistent.toml"])
        .assert()
        .failure();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("stdout must be valid json");
    assert_eq!(v["ok"], false);
    assert_eq!(v["diagnostics"][0]["code"], "whittle.failed");
}

#[test]
fn json_report_normalized_is_trimmed_on_the_empty_path() {
    let dir = TempDir::new().unwrap();
    let p = write_msg(dir.path(), "# only comments\n#\n\n\n");
    let out = Command::cargo_bin("whittle")
        .unwrap()
        .args(["--format", "json", "check"])
        .arg(&p)
        .assert()
        .success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("valid json");
    assert_eq!(v["ok"], true);
    assert_eq!(v["original"], "");
    assert_eq!(v["normalized"], "");
}

#[test]
fn fix_exits_zero_despite_reporting_rewrites() {
    let dir = TempDir::new().unwrap();
    // Rewrites alone must not fail `fix` — only unfixable rule violations do.
    let p = write_msg(dir.path(), "Chore: Bump A and B.\n");
    Command::cargo_bin("whittle")
        .unwrap()
        .args(["fix"])
        .arg(&p)
        .assert()
        .success();
    assert_eq!(read(&p), "chore: bump a & b\n");
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
