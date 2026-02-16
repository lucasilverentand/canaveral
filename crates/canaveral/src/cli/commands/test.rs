//! Test command - Run tests for a project

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use console::style;

use canaveral_frameworks::{
    context::{TestContext, TestReporter},
    testing::{ReportGenerator, TestRunner, TestRunnerConfig},
    TestReport, TestStatus,
};

use crate::cli::{Cli, OutputFormat};

/// Run tests for a project
#[derive(Debug, Args)]
pub struct TestCommand {
    /// Path to the project (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Target platform (optional, some tests are platform-agnostic)
    #[arg(short, long)]
    pub platform: Option<PlatformArg>,

    /// Framework to use (auto-detected if not specified)
    #[arg(short, long)]
    pub framework: Option<String>,

    /// Test filter/pattern
    #[arg(long)]
    pub filter: Option<String>,

    /// Collect code coverage
    #[arg(long)]
    pub coverage: bool,

    /// Output format for test results
    #[arg(long, default_value = "pretty")]
    pub reporter: ReporterArg,

    /// Output file for test results (e.g., junit.xml)
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Fail fast - stop on first failure
    #[arg(long)]
    pub fail_fast: bool,

    /// Retry failed tests
    #[arg(long, default_value = "0")]
    pub retry: usize,

    /// Test timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,

    /// Number of parallel test jobs
    #[arg(short, long)]
    pub jobs: Option<usize>,

    /// Perform a dry run (validate but don't run tests)
    #[arg(long)]
    pub dry_run: bool,

    /// Use smart test selection (only run tests covering changed code)
    #[arg(long)]
    pub smart: bool,

    /// Only test affected packages (requires monorepo workspace)
    #[arg(long)]
    pub affected: bool,

    /// Base ref for affected/smart detection (default: main)
    #[arg(long, default_value = "main")]
    pub base: String,
}

/// Platform argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PlatformArg {
    Ios,
    Android,
    MacOs,
    Windows,
    Linux,
    Web,
}

impl From<PlatformArg> for canaveral_frameworks::traits::Platform {
    fn from(p: PlatformArg) -> Self {
        match p {
            PlatformArg::Ios => Self::Ios,
            PlatformArg::Android => Self::Android,
            PlatformArg::MacOs => Self::MacOs,
            PlatformArg::Windows => Self::Windows,
            PlatformArg::Linux => Self::Linux,
            PlatformArg::Web => Self::Web,
        }
    }
}

/// Reporter output format
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ReporterArg {
    /// Human-readable output
    #[default]
    Pretty,
    /// JSON output
    Json,
    /// JUnit XML output
    Junit,
    /// GitHub Actions annotations
    GithubActions,
}

impl From<ReporterArg> for TestReporter {
    fn from(r: ReporterArg) -> Self {
        match r {
            ReporterArg::Pretty => Self::Pretty,
            ReporterArg::Json => Self::Json,
            ReporterArg::Junit => Self::Junit,
            ReporterArg::GithubActions => Self::GithubActions,
        }
    }
}

impl TestCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        // Resolve path
        let path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            std::env::current_dir()?.join(&self.path)
        };

        if !path.exists() {
            anyhow::bail!("Path not found: {}", path.display());
        }

        // Build test context
        let mut ctx = TestContext::new(&path);

        if let Some(platform) = self.platform {
            ctx = ctx.with_platform(platform.into());
        }

        if let Some(ref filter) = self.filter {
            ctx = ctx.with_filter(filter);
        }

        ctx = ctx.with_coverage(self.coverage);

        if let Some(timeout) = self.timeout {
            ctx = ctx.with_timeout(timeout);
        }

        ctx = ctx.with_reporter(self.reporter.into());
        ctx.dry_run = self.dry_run;

        if let Some(jobs) = self.jobs {
            ctx.jobs = Some(jobs);
        }

        // Build runner config
        let config = TestRunnerConfig::new()
            .with_fail_fast(self.fail_fast)
            .with_retry(self.retry)
            .with_verbose(cli.verbose);

        let config = if let Some(ref framework) = self.framework {
            config.with_adapter(framework)
        } else {
            config
        };

        // Show header
        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Running tests...").bold());
            println!("  Path: {}", style(path.display()).cyan());
            if let Some(ref filter) = self.filter {
                println!("  Filter: {}", style(filter).dim());
            }
            if self.coverage {
                println!("  Coverage: {}", style("enabled").green());
            }
            if self.dry_run {
                println!("  {}", style("(DRY RUN)").yellow().bold());
            }
            println!();
        }

        // Run tests
        let runner = TestRunner::with_config(config);
        let report = runner.run(&path, &ctx).await?;

        // Output results
        self.output_results(&report, cli)?;

        // Write to file if requested
        if let Some(ref output_path) = self.output {
            let reporter: TestReporter = self.reporter.into();
            ReportGenerator::write_to_file(&report, reporter, output_path)?;

            if !cli.quiet && cli.format == OutputFormat::Text {
                println!(
                    "{} Report written to {}",
                    style("✓").green(),
                    style(output_path.display()).cyan()
                );
            }
        }

        // Exit with error if tests failed
        if !report.success() {
            anyhow::bail!(
                "Tests failed: {} passed, {} failed, {} skipped",
                report.passed,
                report.failed,
                report.skipped
            );
        }

        Ok(())
    }

    fn output_results(&self, report: &TestReport, cli: &Cli) -> anyhow::Result<()> {
        // JSON output mode
        if cli.format == OutputFormat::Json {
            let json = ReportGenerator::generate_json(report);
            println!("{}", json);
            return Ok(());
        }

        // GitHub Actions mode
        if matches!(self.reporter, ReporterArg::GithubActions) {
            let ga_output = ReportGenerator::generate_github_actions(report);
            print!("{}", ga_output);
            return Ok(());
        }

        // JUnit mode
        if matches!(self.reporter, ReporterArg::Junit) {
            let junit = ReportGenerator::generate_junit(report);
            println!("{}", junit);
            return Ok(());
        }

        // Pretty mode (default)
        if !cli.quiet {
            self.print_pretty(report);
        }

        Ok(())
    }

    fn print_pretty(&self, report: &TestReport) {
        println!();
        println!("{}", style("═".repeat(70)).dim());
        println!("  {}", style("TEST RESULTS").bold());
        println!("{}", style("═".repeat(70)).dim());
        println!();

        for suite in &report.suites {
            let suite_status = if suite.tests.iter().all(|t| t.status == TestStatus::Passed) {
                style("✓").green()
            } else if suite.tests.iter().any(|t| t.status == TestStatus::Failed) {
                style("✗").red()
            } else {
                style("○").yellow()
            };

            println!(
                "  {} {} ({} tests, {}ms)",
                suite_status,
                style(&suite.name).bold(),
                suite.tests.len(),
                suite.duration_ms
            );

            for test in &suite.tests {
                let (icon, name_style) = match test.status {
                    TestStatus::Passed => (style("✓").green(), style(&test.name).dim()),
                    TestStatus::Failed => (style("✗").red(), style(&test.name).red()),
                    TestStatus::Skipped => (style("○").yellow(), style(&test.name).yellow()),
                };

                println!("      {} {} ({}ms)", icon, name_style, test.duration_ms);

                if let Some(ref error) = test.error {
                    for line in error.lines().take(5) {
                        println!("          {}", style(line).red().dim());
                    }
                    if error.lines().count() > 5 {
                        println!("          {}", style("...").dim());
                    }
                }
            }

            println!();
        }

        println!("{}", style("═".repeat(70)).dim());
        println!(
            "  {} {} passed, {} {} failed, {} {} skipped ({}ms)",
            style(report.passed).green().bold(),
            style("passed").dim(),
            style(report.failed).red().bold(),
            style("failed").dim(),
            style(report.skipped).yellow().bold(),
            style("skipped").dim(),
            report.duration_ms
        );

        if let Some(ref coverage) = report.coverage {
            println!();
            println!(
                "  Coverage: {:.1}% lines",
                style(format!("{:.1}", coverage.line_coverage * 100.0)).cyan()
            );
            if let Some(branch) = coverage.branch_coverage {
                println!(
                    "            {:.1}% branches",
                    style(format!("{:.1}", branch * 100.0)).cyan()
                );
            }
        }

        println!("{}", style("═".repeat(70)).dim());

        if report.success() {
            println!();
            println!("  {} {}", style("✓").green().bold(), style("All tests passed!").green());
        } else {
            println!();
            println!("  {} {}", style("✗").red().bold(), style("Some tests failed.").red());
        }

        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_conversion() {
        let ios: canaveral_frameworks::traits::Platform = PlatformArg::Ios.into();
        assert!(matches!(ios, canaveral_frameworks::traits::Platform::Ios));
    }

    #[test]
    fn test_reporter_conversion() {
        let pretty: TestReporter = ReporterArg::Pretty.into();
        assert!(matches!(pretty, TestReporter::Pretty));
    }
}
