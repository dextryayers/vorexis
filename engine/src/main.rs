mod core;
mod job;
mod modules;
mod output;
mod wordlists;

use crate::job::JobSpec;
use clap::Parser;
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "aipentest-engine", about = "High-performance pentest scanning engine written in Rust", version)]
struct Cli {
    /// Path to a JSON job file. Reads from stdin if omitted.
    #[arg(short, long)]
    job: Option<String>,

    /// One-shot: scan a target with the given modules (comma separated).
    #[arg(short, long)]
    target: Option<String>,

    /// Modules to run (comma separated), only used with --target.
    #[arg(short, long, default_value = "port,directory,http,tls,fingerprint,tech")]
    modules: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let job: JobSpec = if let Some(target) = cli.target {
        JobSpec {
            target,
            modules: cli
                .modules
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            concurrency: 50,
            timeout: 8,
            options: Default::default(),
            wordlists: Default::default(),
        }
    } else {
        let input = match &cli.job {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("failed to read job file: {e}");
                    std::process::exit(1);
                }
            },
            None => {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf).unwrap_or_default();
                buf
            }
        };
        match serde_json::from_str(&input) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("invalid job JSON: {e}");
                eprintln!("expected: {{\"target\": \"example.com\", \"modules\": [\"port\", \"directory\"], \"concurrency\": 50, \"timeout\": 8, \"options\": {{}}}}");
                std::process::exit(1);
            }
        }
    };

    let (tx, mut rx) = mpsc::channel::<output::OutputEvent>(1024);

    let runner = tokio::spawn(async move {
        modules::run(job, tx).await;
    });

    // Drain the channel and print NDJSON lines.
    while let Some(event) = rx.recv().await {
        print!("{}", output::to_json_line(&event));
        use std::io::Write;
        std::io::stdout().flush().ok();
    }
    let _ = runner.await;
}
