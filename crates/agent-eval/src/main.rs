use agent_eval::{
    load_scenarios, write_results_jsonl, write_summary_json, EvalHarness, EvalRunOptions,
    EvalSummary, Result,
};
use agent_tools::{parse_legacy_mode, ApprovalPolicy, SandboxPolicy};
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
                "--permission-mode" => {
                    let raw = next_value(&mut iter, "--permission-mode")?;
                    let (parsed_approval, parsed_sandbox) = parse_legacy_mode(&raw)
                        .ok_or_else(|| agent_eval::EvalError::PermissionMode(raw.clone()))?;
                    approval_policy = parsed_approval;
                    sandbox_policy = parsed_sandbox;
                }
                "--include-trace" => include_trace = true,
                "--enable-mcp" => enable_mcp = true,
                "--enable-hooks" => enable_hooks = true,
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

fn usage() -> String {
    "usage: kairox-eval run --scenarios <file.jsonl> --output <results.jsonl> [--summary <summary.json>] [--workspace <path>] [--profile <alias>] [--permission-mode read_only|suggest|agent|autonomous|interactive] [--include-trace] [--enable-mcp] [--enable-hooks]".into()
}
