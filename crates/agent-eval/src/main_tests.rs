use super::*;

#[test]
fn parses_list_command_args() {
    let command = CliCommand::parse([
        "list".to_string(),
        "--scenarios".to_string(),
        "fixtures.jsonl".to_string(),
        "--tag".to_string(),
        "smoke".to_string(),
        "--exclude-tag".to_string(),
        "slow".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ])
    .expect("list command should parse");

    let CliCommand::List(args) = command else {
        panic!("expected list command");
    };

    assert_eq!(args.scenarios, PathBuf::from("fixtures.jsonl"));
    assert_eq!(args.include_tags, vec!["smoke"]);
    assert_eq!(args.exclude_tags, vec!["slow"]);
    assert!(matches!(args.format, ListFormat::Json));
}

#[test]
fn parses_fail_fast_run_arg() {
    let command = CliCommand::parse([
        "run".to_string(),
        "--scenarios".to_string(),
        "fixtures.jsonl".to_string(),
        "--output".to_string(),
        "results.jsonl".to_string(),
        "--fail-fast".to_string(),
    ])
    .expect("run command should parse");

    let CliCommand::Run(args) = command else {
        panic!("expected run command");
    };

    assert!(args.fail_fast);
}
