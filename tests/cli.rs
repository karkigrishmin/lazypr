use assert_cmd::Command;
use predicates::prelude::*;

fn lazypr() -> Command {
    Command::cargo_bin("lazypr").expect("binary exists")
}

#[test]
fn help_exits_zero_and_shows_subcommands() {
    lazypr()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("review"))
        .stdout(predicate::str::contains("split"))
        .stdout(predicate::str::contains("ghost"))
        .stdout(predicate::str::contains("impact"))
        .stdout(predicate::str::contains("inbox"))
        .stdout(predicate::str::contains("stats"))
        .stdout(predicate::str::contains("notes"));
}

#[test]
fn version_exits_zero() {
    lazypr()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("lazypr"));
}

#[test]
fn split_runs_plan_generation() {
    // Split now runs a real analysis; verify it produces plan output.
    let output = lazypr().arg("split").output().expect("split should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Split plan:"),
        "expected split plan output, got: {}",
        stdout
    );
}

#[test]
fn ghost_runs_analysis() {
    // Ghost runs a real analysis; it may find warnings (exit 0) or errors (exit 1).
    // We just verify it produces ghost-related output, not "not yet implemented".
    let output = lazypr().arg("ghost").output().expect("ghost should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Ghost analysis") || stdout.contains("No issues found"),
        "expected ghost analysis output, got: {}",
        stdout
    );
}

#[test]
fn impact_runs_analysis() {
    // Impact runs a real analysis on the given file.
    let output = lazypr()
        .args(["impact", "src/main.rs"])
        .output()
        .expect("impact should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Impact analysis for"),
        "expected impact analysis output, got: {}",
        stdout
    );
}

#[test]
fn inbox_runs_or_reports_missing_remote() {
    // Inbox now attempts to connect to GitHub/GitLab.
    // Without a token it will fail with a remote-detection error.
    // With a token and valid remote it will show the PR dashboard.
    let output = lazypr().arg("inbox").output().expect("inbox should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("PR Inbox")
            || combined.contains("No remote provider")
            || combined.contains("No open pull requests")
            || combined.contains("failed to open git repository"),
        "expected inbox output or remote error, got stdout={} stderr={}",
        stdout,
        stderr
    );
}

#[test]
fn stats_shows_coming_soon() {
    lazypr()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Coming soon"));
}

#[test]
fn notes_shows_coming_soon() {
    lazypr()
        .arg("notes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Coming soon"));
}

#[test]
fn unknown_subcommand_fails() {
    lazypr().arg("nonexistent").assert().failure();
}
