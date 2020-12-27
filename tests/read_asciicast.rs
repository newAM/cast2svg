use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{path::PathBuf, process::Command};

fn test_file(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push(name);
    path.to_string_lossy().to_string()
}

fn base_cmd() -> Command {
    Command::cargo_bin("cast2svg").unwrap()
}

#[test]
fn file_doesnt_exist() {
    let mut cmd: Command = base_cmd();
    cmd.arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

#[test]
fn no_header() {
    let mut cmd: Command = base_cmd();
    cmd.arg(test_file("no_header.cast"));
    cmd.assert().failure().stderr(predicate::str::contains(
        "Failed to deserialize header from asciicast",
    ));
}

#[test]
fn min_events() {
    let mut cmd: Command = base_cmd();
    cmd.arg(test_file("min_events.cast"));
    cmd.assert().failure().stderr(predicate::str::contains(
        "asciicast must have at least 2 events",
    ));
}

#[test]
fn bad_event() {
    let mut cmd: Command = base_cmd();
    cmd.arg(test_file("bad_event.cast"));
    cmd.assert().failure().stderr(predicate::str::contains(
        "Failed to deserialize line 4 from asciicast",
    ));
}

#[test]
fn time_travel() {
    let mut cmd: Command = base_cmd();
    cmd.arg(test_file("time_travel.cast"));
    cmd.assert().failure().stderr(predicate::str::contains(
        "asciicast event on line 5 went backwards in time",
    ));
}
