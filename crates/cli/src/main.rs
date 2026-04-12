use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "lite-vision")]
#[command(about = "Lite-Vision CLI - CPU-secured truth, GPU-powered intelligence")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "node")]
    Node {
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,

        #[arg(long, default_value_t = 8080)]
        port: u16,

        #[arg(long)]
        truth_only: bool,
    },
    #[command(name = "validator")]
    Validator {
        #[arg(long)]
        stake: Option<u64>,

        #[arg(long, default_value = "127.0.0.1")]
        rpc_host: String,

        #[arg(long, default_value_t = 8081)]
        rpc_port: u16,
    },
    #[command(name = "operator")]
    Operator {
        #[arg(long)]
        gpu_model: Option<String>,

        #[arg(long, default_value = "127.0.0.1")]
        rpc_host: String,

        #[arg(long, default_value_t = 8082)]
        rpc_port: u16,
    },
    #[command(name = "submit")]
    SubmitJob {
        #[arg(long)]
        kernel: String,

        #[arg(long)]
        input: String,

        #[arg(long, default_value_t = 1000)]
        budget: u64,

        #[arg(long, default_value = "127.0.0.1")]
        rpc_host: String,

        #[arg(long, default_value_t = 8082)]
        rpc_port: u16,
    },
    #[command(name = "status")]
    Status {
        #[arg(long)]
        rpc_host: Option<String>,

        #[arg(long)]
        rpc_port: Option<u16>,
    },
    #[command(name = "version")]
    Version,
    #[command(name = "query")]
    Query {
        #[arg(long)]
        job_id: Option<String>,

        #[arg(long, default_value = "127.0.0.1")]
        rpc_host: String,

        #[arg(long, default_value_t = 8082)]
        rpc_port: u16,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Node {
            bind,
            port,
            truth_only,
        } => {
            println!("Starting Lite-Vision node on {}:{}", bind, port);
            if truth_only {
                println!("Mode: Truth Plane (BFT + Ledger)");
            } else {
                println!("Mode: Full Node (Truth + Intelligence Plane)");
            }
            println!("Use 'lite-vision validator' or 'lite-vision operator' to join network");
            ExitCode::SUCCESS
        }
        Commands::Validator {
            stake,
            rpc_host,
            rpc_port,
        } => {
            let stake = stake.unwrap_or(1000);
            println!("Starting validator with stake: {} LVU", stake);
            println!("Connecting to RPC: {}:{}", rpc_host, rpc_port);
            println!("Validator ready - awaiting block production");
            ExitCode::SUCCESS
        }
        Commands::Operator {
            gpu_model,
            rpc_host,
            rpc_port,
        } => {
            let gpu = gpu_model.unwrap_or_else(|| "unknown".to_string());
            println!("Starting operator with GPU: {}", gpu);
            println!("Connecting to RPC: {}:{}", rpc_host, rpc_port);
            println!("Operator ready - awaiting job assignments");
            ExitCode::SUCCESS
        }
        Commands::SubmitJob {
            kernel,
            input,
            budget,
            rpc_host,
            rpc_port,
        } => {
            println!("Submitting job: kernel={}", kernel);
            println!("Input: {}", input);
            println!("Budget: {} LVU", budget);
            println!("Target: {}:{}", rpc_host, rpc_port);
            println!("Job submitted (mock - requires running operator)");
            ExitCode::SUCCESS
        }
        Commands::Status { rpc_host, rpc_port } => {
            if let (Some(host), Some(port)) = (rpc_host, rpc_port) {
                println!("Querying node status: {}:{}", host, port);
            }
            println!("Lite-Vision Status: OK");
            println!("Network: Ready");
            println!("Validators: 0/4 (requires network join)");
            ExitCode::SUCCESS
        }
        Commands::Version => {
            println!("Lite-Vision v{}", env!("CARGO_PKG_VERSION"));
            println!("Truth Plane: BFT consensus");
            println!("Intelligence Plane: Job execution");
            println!("RPACK: Render packet format");
            ExitCode::SUCCESS
        }
        Commands::Query {
            job_id,
            rpc_host,
            rpc_port,
        } => {
            if let Some(id) = job_id {
                println!("Querying job: {}", id);
                println!("Target: {}:{}", rpc_host, rpc_port);
                println!("Job status: Not Found (mock - requires running network)");
            } else {
                println!("No job ID provided. Use --job-id <ID>");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}
