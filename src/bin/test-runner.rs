#![feature(rustc_private)]

use std::{
    env, fs,
    io::{self, BufRead, Error, ErrorKind},
    process::{self, exit},
};

use clap::Parser;
use ferrous_owl::{LspClient, TestCase, TestResult, cleanup_workspace, run_test, setup_workspace};
use rayon::prelude::*;

/// Test runner for ferrous-owl LSP decoration tests.
#[derive(Parser, Debug)]
#[command(name = "test-runner")]
#[command(about = "Runs LSP decoration tests against ferrous-owl")]
struct Args {
    /// Run a single test case (JSON string)
    #[arg(long)]
    single: Option<String>,
}

fn read_input(single: Option<String>) -> io::Result<String> {
    single.map_or_else(
        || {
            io::stdin()
                .lock()
                .lines()
                .collect::<io::Result<Vec<_>>>()
                .map(|lines| lines.join("\n"))
        },
        |json| Ok(format!("[{json}]")),
    )
}

fn parse_tests(input: &str) -> Result<Vec<TestCase>, String> {
    serde_json::from_str(input).map_err(|e| format!("Failed to parse test cases: {e}"))
}

fn create_workspace(test_name: &str, index: usize) -> io::Result<String> {
    let unique_id = process::id();
    let base_dir = env::temp_dir().join("owl-tests");
    fs::create_dir_all(&base_dir)?;

    let workspace_name = format!("{test_name}_{unique_id}_{index}");
    setup_workspace(&base_dir.to_string_lossy(), &workspace_name)
}

fn execute_test(owl_binary: &str, test: &TestCase, index: usize) -> TestResult {
    let workspace_dir = match create_workspace(&test.name, index) {
        Ok(dir) => dir,
        Err(e) => {
            return TestResult {
                name: test.name.clone(),
                passed: false,
                message: format!("Failed to create workspace: {e}"),
            };
        }
    };

    let result = (|| -> io::Result<TestResult> {
        let mut client = LspClient::start(owl_binary, &[])?;
        let workspace_uri = format!("file://{workspace_dir}");
        client.initialize(&workspace_uri)?;

        let result = run_test(&mut client, test, &workspace_dir).unwrap_or_else(|e| TestResult {
            name: test.name.clone(),
            passed: false,
            message: format!("Error: {e}"),
        });

        let _ = client.shutdown();
        Ok(result)
    })();

    let _ = cleanup_workspace(&workspace_dir);

    result.unwrap_or_else(|e| TestResult {
        name: test.name.clone(),
        passed: false,
        message: format!("LSP client error: {e}"),
    })
}

fn print_summary(results: &[TestResult]) {
    let (passed, failed): (Vec<_>, Vec<_>) = results.iter().partition(|r| r.passed);

    for result in &failed {
        eprintln!("✗ {}", result.name);
        eprintln!("  {}", result.message);
    }

    eprintln!("\n{} passed, {} failed", passed.len(), failed.len());
}

fn main() -> io::Result<()> {
    env_logger::init();

    let args = Args::parse();
    let input = read_input(args.single)?;

    let tests = parse_tests(&input).unwrap_or_else(|e| {
        eprintln!("{e}");
        exit(1);
    });

    if tests.is_empty() {
        eprintln!("No test cases provided");
        exit(1);
    }

    let owl_binary = find_owl_binary()?;

    let results: Vec<_> = tests
        .par_iter()
        .enumerate()
        .map(|(index, test)| execute_test(&owl_binary, test, index))
        .collect();

    print_summary(&results);

    let has_failures = results.iter().any(|r| !r.passed);
    if has_failures {
        exit(1);
    }

    Ok(())
}

fn find_owl_binary() -> io::Result<String> {
    let owl_path = env::current_exe()?
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "Cannot determine executable directory"))?
        .join("ferrous-owl");

    if owl_path.exists() {
        Ok(owl_path.to_string_lossy().to_string())
    } else {
        Err(Error::new(
            ErrorKind::NotFound,
            format!("ferrous-owl binary not found at {}", owl_path.display()),
        ))
    }
}
