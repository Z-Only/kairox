use agent_eval::{
    compare_reports, filter_scenarios_by_tags, load_scenarios, write_comparison_json,
    write_report_json, write_results_jsonl, write_summary_json, EvalHarness, EvalReport,
    EvalRunOptions, EvalSummary, Result,
};
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("kairox-eval: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let command = CliCommand::parse(std::env::args().skip(1))?;

    match command {
        CliCommand::Run(args) => run_scenarios(*args).await,
        CliCommand::List(args) => list_scenarios(args),
        CliCommand::Compare(args) => compare_command(args),
    }
}

async fn run_scenarios(args: RunArgs) -> Result<()> {
    let scenarios = load_scenarios(&args.scenarios)?;
    let scenarios = filter_scenarios_by_tags(&scenarios, &args.include_tags, &args.exclude_tags);
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: args.workspace,
        default_profile: args.profile,
        config: None,
        approval_policy: args.approval_policy,
        sandbox_policy: args.sandbox_policy,
        include_trace: args.include_trace,
        enable_mcp: args.enable_mcp,
        enable_hooks: args.enable_hooks,
        auto_compact_threshold: args.auto_compact_threshold,
        fake_emit_tool_call: args.fake_emit_tool_call,
        fake_tool_id: args.fake_tool_id,
        fake_tool_arguments: args.fake_tool_arguments,
        wait_timeout_ms: args.wait_timeout_ms,
        seed_synthetic_pairs: args.seed_synthetic_pairs,
    })
    .await?;
    let results = if args.fail_fast {
        harness.run_scenarios_until_failure(&scenarios).await
    } else {
        harness.run_scenarios(&scenarios).await
    };
    let summary = EvalSummary::from_results(&results);

    write_results_jsonl(&args.output, &results)?;
    if let Some(summary_path) = args.summary {
        write_summary_json(summary_path, &summary)?;
    }
    if let Some(report_path) = args.report {
        write_report_json(
            report_path,
            &EvalReport {
                summary: summary.clone(),
                results,
            },
        )?;
    }

    println!("{}", serde_json::to_string_pretty(&summary)?);
    if summary.failed > 0 {
        std::process::exit(2);
    }
    Ok(())
}

fn list_scenarios(args: ListArgs) -> Result<()> {
    let scenarios = load_scenarios(&args.scenarios)?;
    let scenarios = filter_scenarios_by_tags(&scenarios, &args.include_tags, &args.exclude_tags);
    let ids = scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect::<Vec<_>>();

    match args.format {
        ListFormat::Text => {
            for id in ids {
                println!("{id}");
            }
        }
        ListFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&ids)?);
        }
    }

    Ok(())
}

enum CliCommand {
    Run(Box<RunArgs>),
    List(ListArgs),
    Compare(CompareArgs),
}

impl CliCommand {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut iter = args.into_iter().peekable();
        match iter.peek().map(String::as_str) {
            Some("list") => {
                iter.next();
                ListArgs::parse(iter).map(Self::List)
            }
            Some("compare") => {
                iter.next();
                CompareArgs::parse(iter).map(Self::Compare)
            }
            _ => RunArgs::parse(iter).map(Box::new).map(Self::Run),
        }
    }
}

struct RunArgs {
    scenarios: PathBuf,
    output: PathBuf,
    summary: Option<PathBuf>,
    report: Option<PathBuf>,
    workspace: PathBuf,
    profile: Option<String>,
    approval_policy: ApprovalPolicy,
    sandbox_policy: SandboxPolicy,
    include_trace: bool,
    enable_mcp: bool,
    enable_hooks: bool,
    auto_compact_threshold: Option<f32>,
    fake_emit_tool_call: bool,
    fake_tool_id: Option<String>,
    fake_tool_arguments: Option<serde_json::Value>,
    wait_timeout_ms: Option<u64>,
    seed_synthetic_pairs: Option<usize>,
    fail_fast: bool,
    include_tags: Vec<String>,
    exclude_tags: Vec<String>,
}

impl RunArgs {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut scenarios = None;
        let mut output = None;
        let mut summary = None;
        let mut report = None;
        let mut workspace = std::env::current_dir()?;
        let mut profile = None;
        let mut approval_policy = ApprovalPolicy::OnRequest;
        let mut sandbox_policy = SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        };
        let mut include_trace = false;
        let mut enable_mcp = false;
        let mut enable_hooks = false;
        let mut auto_compact_threshold: Option<f32> = None;
        let mut fake_emit_tool_call = false;
        let mut fake_tool_id: Option<String> = None;
        let mut fake_tool_arguments: Option<serde_json::Value> = None;
        let mut wait_timeout_ms: Option<u64> = None;
        let mut seed_synthetic_pairs: Option<usize> = None;
        let mut fail_fast = false;
        let mut include_tags = Vec::new();
        let mut exclude_tags = Vec::new();

        let mut iter = args.into_iter().peekable();
        if iter.peek().is_some_and(|arg| arg == "run") {
            iter.next();
        }

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--scenarios" => scenarios = Some(next_path(&mut iter, "--scenarios")?),
                "--output" => output = Some(next_path(&mut iter, "--output")?),
                "--summary" => summary = Some(next_path(&mut iter, "--summary")?),
                "--report" => report = Some(next_path(&mut iter, "--report")?),
                "--workspace" => workspace = next_path(&mut iter, "--workspace")?,
                "--profile" => profile = Some(next_value(&mut iter, "--profile")?),
                "--approval-policy" => {
                    let raw = next_value(&mut iter, "--approval-policy")?;
                    approval_policy = raw.parse().map_err(agent_eval::EvalError::Policy)?;
                }
                "--sandbox-policy" => {
                    let raw = next_value(&mut iter, "--sandbox-policy")?;
                    sandbox_policy = parse_sandbox_policy(&raw)?;
                }
                "--include-trace" => include_trace = true,
                "--enable-mcp" => enable_mcp = true,
                "--enable-hooks" => enable_hooks = true,
                "--auto-compact-threshold" => {
                    let raw = next_value(&mut iter, "--auto-compact-threshold")?;
                    auto_compact_threshold = Some(raw.parse().map_err(|error| {
                        agent_eval::EvalError::Cli(format!(
                            "invalid --auto-compact-threshold `{raw}`: {error}"
                        ))
                    })?);
                }
                "--fake-emit-tool-call" => fake_emit_tool_call = true,
                "--fake-tool-id" => {
                    fake_tool_id = Some(next_value(&mut iter, "--fake-tool-id")?);
                }
                "--fake-tool-arguments" => {
                    let raw = next_value(&mut iter, "--fake-tool-arguments")?;
                    fake_tool_arguments = Some(serde_json::from_str(&raw).map_err(|error| {
                        agent_eval::EvalError::Cli(format!(
                            "invalid --fake-tool-arguments JSON `{raw}`: {error}"
                        ))
                    })?);
                }
                "--wait-timeout-ms" => {
                    let raw = next_value(&mut iter, "--wait-timeout-ms")?;
                    wait_timeout_ms = Some(raw.parse().map_err(|error| {
                        agent_eval::EvalError::Cli(format!(
                            "invalid --wait-timeout-ms `{raw}`: {error}"
                        ))
                    })?);
                }
                "--seed-synthetic-pairs" => {
                    let raw = next_value(&mut iter, "--seed-synthetic-pairs")?;
                    seed_synthetic_pairs = Some(raw.parse().map_err(|error| {
                        agent_eval::EvalError::Cli(format!(
                            "invalid --seed-synthetic-pairs `{raw}`: {error}"
                        ))
                    })?);
                }
                "--fail-fast" => fail_fast = true,
                "--tag" => include_tags.push(next_value(&mut iter, "--tag")?),
                "--exclude-tag" => exclude_tags.push(next_value(&mut iter, "--exclude-tag")?),
                "--help" | "-h" => return Err(agent_eval::EvalError::Cli(usage())),
                other => {
                    return Err(agent_eval::EvalError::Cli(format!(
                        "unknown argument: {other}\n{}",
                        usage()
                    )));
                }
            }
        }

        Ok(Self {
            scenarios: scenarios.ok_or_else(|| {
                agent_eval::EvalError::Cli(format!("missing --scenarios\n{}", usage()))
            })?,
            output: output.ok_or_else(|| {
                agent_eval::EvalError::Cli(format!("missing --output\n{}", usage()))
            })?,
            summary,
            report,
            workspace,
            profile,
            approval_policy,
            sandbox_policy,
            include_trace,
            enable_mcp,
            enable_hooks,
            auto_compact_threshold,
            fake_emit_tool_call,
            fake_tool_id,
            fake_tool_arguments,
            wait_timeout_ms,
            seed_synthetic_pairs,
            fail_fast,
            include_tags,
            exclude_tags,
        })
    }
}

struct ListArgs {
    scenarios: PathBuf,
    include_tags: Vec<String>,
    exclude_tags: Vec<String>,
    format: ListFormat,
}

impl ListArgs {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut scenarios = None;
        let mut include_tags = Vec::new();
        let mut exclude_tags = Vec::new();
        let mut format = ListFormat::Text;

        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--scenarios" => scenarios = Some(next_path(&mut iter, "--scenarios")?),
                "--tag" => include_tags.push(next_value(&mut iter, "--tag")?),
                "--exclude-tag" => exclude_tags.push(next_value(&mut iter, "--exclude-tag")?),
                "--format" => {
                    format = ListFormat::parse(&next_value(&mut iter, "--format")?)?;
                }
                "--help" | "-h" => return Err(agent_eval::EvalError::Cli(list_usage())),
                other => {
                    return Err(agent_eval::EvalError::Cli(format!(
                        "unknown argument: {other}\n{}",
                        list_usage()
                    )));
                }
            }
        }

        Ok(Self {
            scenarios: scenarios.ok_or_else(|| {
                agent_eval::EvalError::Cli(format!("missing --scenarios\n{}", list_usage()))
            })?,
            include_tags,
            exclude_tags,
            format,
        })
    }
}

#[derive(Clone, Copy)]
enum ListFormat {
    Text,
    Json,
}

impl ListFormat {
    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(agent_eval::EvalError::Cli(format!(
                "invalid --format `{raw}`; expected text or json"
            ))),
        }
    }
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &'static str) -> Result<String> {
    iter.next()
        .ok_or_else(|| agent_eval::EvalError::Cli(format!("missing value for {flag}")))
}

fn next_path(iter: &mut impl Iterator<Item = String>, flag: &'static str) -> Result<PathBuf> {
    Ok(PathBuf::from(next_value(iter, flag)?))
}

fn parse_sandbox_policy(raw: &str) -> Result<SandboxPolicy> {
    match raw.to_ascii_lowercase().as_str() {
        "read_only" | "readonly" | "read-only" => Ok(SandboxPolicy::ReadOnly),
        "workspace_write" | "workspacewrite" | "workspace-write" => {
            Ok(SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            })
        }
        "danger_full_access" | "dangerfullaccess" | "danger-full-access" => {
            Ok(SandboxPolicy::DangerFullAccess)
        }
        _ => serde_json::from_str(raw).map_err(|error| {
            agent_eval::EvalError::Policy(format!("invalid sandbox policy `{raw}`: {error}"))
        }),
    }
}

fn compare_command(args: CompareArgs) -> Result<()> {
    let baseline_content = std::fs::read_to_string(&args.baseline)?;
    let candidate_content = std::fs::read_to_string(&args.candidate)?;
    let baseline: EvalReport = serde_json::from_str(&baseline_content)?;
    let candidate: EvalReport = serde_json::from_str(&candidate_content)?;

    let comparison = compare_reports(&baseline, &candidate);

    if let Some(output) = args.output {
        write_comparison_json(output, &comparison)?;
    }

    println!("{}", serde_json::to_string_pretty(&comparison)?);
    if !comparison.regressions.is_empty() {
        std::process::exit(3);
    }
    Ok(())
}

struct CompareArgs {
    baseline: PathBuf,
    candidate: PathBuf,
    output: Option<PathBuf>,
}

impl CompareArgs {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut baseline = None;
        let mut candidate = None;
        let mut output = None;

        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--baseline" => baseline = Some(next_path(&mut iter, "--baseline")?),
                "--candidate" => candidate = Some(next_path(&mut iter, "--candidate")?),
                "--output" => output = Some(next_path(&mut iter, "--output")?),
                "--help" | "-h" => return Err(agent_eval::EvalError::Cli(compare_usage())),
                other => {
                    return Err(agent_eval::EvalError::Cli(format!(
                        "unknown argument: {other}\n{}",
                        compare_usage()
                    )));
                }
            }
        }

        Ok(Self {
            baseline: baseline.ok_or_else(|| {
                agent_eval::EvalError::Cli(format!("missing --baseline\n{}", compare_usage()))
            })?,
            candidate: candidate.ok_or_else(|| {
                agent_eval::EvalError::Cli(format!("missing --candidate\n{}", compare_usage()))
            })?,
            output,
        })
    }
}

fn compare_usage() -> String {
    "usage: kairox-eval compare --baseline <report.json> --candidate <report.json> [--output <comparison.json>]".into()
}

fn usage() -> String {
    "usage: kairox-eval run --scenarios <file.jsonl> --output <results.jsonl> [--summary <summary.json>] [--report <report.json>] [--workspace <path>] [--profile <alias>] [--approval-policy never|on_request|always] [--sandbox-policy read_only|workspace_write|danger_full_access|json] [--include-trace] [--enable-mcp] [--enable-hooks] [--auto-compact-threshold <f32>] [--fake-emit-tool-call] [--fake-tool-id <id>] [--fake-tool-arguments <json>] [--wait-timeout-ms <u64>] [--seed-synthetic-pairs <n>] [--fail-fast] [--tag <tag>] [--exclude-tag <tag>]".into()
}

fn list_usage() -> String {
    "usage: kairox-eval list --scenarios <file.jsonl> [--tag <tag>] [--exclude-tag <tag>] [--format text|json]".into()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
