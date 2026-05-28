use agent_eval::{
    load_scenarios, write_results_jsonl, write_summary_json, EvalHarness, EvalRunOptions,
    EvalSummary, Result,
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
    let args = CliArgs::parse(std::env::args().skip(1))?;
    let scenarios = load_scenarios(&args.scenarios)?;
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
    let results = harness.run_scenarios(&scenarios).await;
    let summary = EvalSummary::from_results(&results);

    write_results_jsonl(&args.output, &results)?;
    if let Some(summary_path) = args.summary {
        write_summary_json(summary_path, &summary)?;
    }

    println!("{}", serde_json::to_string_pretty(&summary)?);
    if summary.failed > 0 {
        std::process::exit(2);
    }
    Ok(())
}

struct CliArgs {
    scenarios: PathBuf,
    output: PathBuf,
    summary: Option<PathBuf>,
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
}

impl CliArgs {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut scenarios = None;
        let mut output = None;
        let mut summary = None;
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

        let mut iter = args.into_iter().peekable();
        if iter.peek().is_some_and(|arg| arg == "run") {
            iter.next();
        }

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--scenarios" => scenarios = Some(next_path(&mut iter, "--scenarios")?),
                "--output" => output = Some(next_path(&mut iter, "--output")?),
                "--summary" => summary = Some(next_path(&mut iter, "--summary")?),
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
        })
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

fn usage() -> String {
    "usage: kairox-eval run --scenarios <file.jsonl> --output <results.jsonl> [--summary <summary.json>] [--workspace <path>] [--profile <alias>] [--approval-policy never|on_request|always] [--sandbox-policy read_only|workspace_write|danger_full_access|json] [--include-trace] [--enable-mcp] [--enable-hooks] [--auto-compact-threshold <f32>] [--fake-emit-tool-call] [--fake-tool-id <id>] [--fake-tool-arguments <json>] [--wait-timeout-ms <u64>] [--seed-synthetic-pairs <n>]".into()
}
