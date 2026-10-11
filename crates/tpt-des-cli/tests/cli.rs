//! CLI tests: each command on an inline model, the error paths, and one run of
//! the binary through the operating system.

use std::process::Command;
use tpt_des_cli::execute;

const QUEUE: &str = r#"
kind = "queue"
lambda = 0.5
mu = 1.0
horizon = 20000.0
warmup = 2000.0
seed = 42
replications = 4
"#;

#[test]
fn run_reports_mean_and_standard_error() {
    let out = execute("run", QUEUE).unwrap();
    assert!(out.ok);
    assert!(out.output.contains("Wq"), "{}", out.output);
    assert!(out.output.contains("std error"), "{}", out.output);
    assert!(out.output.contains("4 replications"), "{}", out.output);
}

#[test]
fn run_rejects_a_single_replication() {
    let text = QUEUE.replace("replications = 4", "replications = 1");
    let err = execute("run", &text).unwrap_err();
    assert!(err.contains("at least 2"), "{err}");
}

#[test]
fn validate_passes_for_a_correct_mmc_station() {
    let text = QUEUE
        .replace("lambda = 0.5", "lambda = 2.4")
        .replace("horizon = 20000.0", "horizon = 10000.0")
        .replace("warmup = 2000.0", "warmup = 1000.0")
        .replace("replications = 4", "replications = 6")
        + "servers = 3\n";
    let out = execute("validate", &text).unwrap();
    assert!(out.ok, "{}", out.output);
    assert!(out.output.contains("c = 3"), "{}", out.output);
}

#[test]
fn size_finds_the_fewest_servers() {
    let text = "kind = \"queue\"\nlambda = 4.0\nmu = 1.0\nhorizon = 1.0\nmax_wait = 0.2\n";
    let out = execute("size", text).unwrap();
    let servers: u32 = out
        .output
        .lines()
        .find_map(|l| l.strip_prefix("servers: "))
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        servers >= 5,
        "λ = 4 needs at least 5 servers, got {servers}"
    );
}

#[test]
fn network_solves_a_jackson_network() {
    let text = r#"
kind = "jackson"
external = [1.0, 0.0]
routing = [[0.0, 0.5], [0.0, 0.0]]
service_rates = [2.0, 1.0]
"#;
    let out = execute("network", text).unwrap();
    assert!(out.output.contains("total L = 2.000000"), "{}", out.output);
}

#[test]
fn network_solves_a_multiclass_closed_network() {
    let text = r#"
kind = "multiclass"

[[classes]]
population = 2
routing = [[0.0, 1.0], [1.0, 0.0]]
service_times = [1.0, 1.0]

[[classes]]
population = 1
routing = [[0.0, 1.0], [1.0, 0.0]]
service_times = [1.0, 1.0]
"#;
    let out = execute("network", text).unwrap();
    assert!(out.output.contains("throughput"), "{}", out.output);
    assert!(out.output.contains("node utilizations"), "{}", out.output);
}

#[test]
fn wrong_kind_for_a_command_is_an_error() {
    let jackson =
        "kind = \"jackson\"\nexternal = [1.0]\nrouting = [[0.0]]\nservice_rates = [2.0]\n";
    assert!(execute("run", jackson)
        .unwrap_err()
        .contains("kind = \"queue\""));
    assert!(execute("network", QUEUE).unwrap_err().contains("jackson"));
}

#[test]
fn unknown_command_and_bad_discipline_are_errors() {
    assert!(execute("plot", QUEUE)
        .unwrap_err()
        .contains("unknown command"));
    let text = format!("{QUEUE}discipline = \"random\"\n");
    assert!(execute("run", &text)
        .unwrap_err()
        .contains("unknown discipline"));
}

#[test]
fn processor_sharing_with_several_servers_is_refused() {
    let text = format!("{QUEUE}discipline = \"ps\"\nservers = 2\n");
    assert!(execute("run", &text).unwrap_err().contains("single-server"));
}

#[test]
fn unknown_keys_and_bad_toml_are_errors() {
    let typo = format!("{QUEUE}lamda = 1.0\n");
    assert!(execute("run", &typo)
        .unwrap_err()
        .contains("invalid model file"));
    assert!(execute("run", "kind = [")
        .unwrap_err()
        .contains("invalid model file"));
}

#[test]
fn non_finite_times_are_rejected_before_simulating() {
    // TOML accepts `inf`; the CLI must report it, not panic.
    let text = QUEUE.replace("horizon = 20000.0", "horizon = inf");
    assert!(execute("run", &text).unwrap_err().contains("not finite"));
}

#[test]
fn binary_runs_end_to_end_with_exit_status() {
    let path = std::env::temp_dir().join("tpt-des-cli-size-test.toml");
    std::fs::write(
        &path,
        "kind = \"queue\"\nlambda = 0.8\nmu = 1.0\nhorizon = 1.0\nmax_wait = 1.0\n",
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_tpt-des");
    let ok = Command::new(bin).arg("size").arg(&path).output().unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(String::from_utf8_lossy(&ok.stdout).contains("servers:"));

    let bad = Command::new(bin).arg("plot").arg(&path).output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let _ = std::fs::remove_file(&path);
}
