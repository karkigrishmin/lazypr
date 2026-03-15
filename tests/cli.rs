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
fn split_prints_not_yet_implemented() {
    lazypr()
        .arg("split")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn ghost_prints_not_yet_implemented() {
    lazypr()
        .arg("ghost")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn impact_prints_not_yet_implemented() {
    lazypr()
        .args(["impact", "src/main.rs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn inbox_prints_not_yet_implemented() {
    lazypr()
        .arg("inbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn stats_prints_not_yet_implemented() {
    lazypr()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn notes_prints_not_yet_implemented() {
    lazypr()
        .arg("notes")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn unknown_subcommand_fails() {
    lazypr().arg("nonexistent").assert().failure();
}
