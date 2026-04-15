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
        #[arg(long, default_value = "0.0.0.0")]
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
    Submit {
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
    #[command(name = "query")]
    Query {
        #[arg(long)]
        job_id: Option<String>,

        #[arg(long, default_value = "127.0.0.1")]
        rpc_host: String,

        #[arg(long, default_value_t = 8082)]
        rpc_port: u16,
    },
    #[command(name = "version")]
    Version,
    #[command(name = "info")]
    Info,
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
                println!("Mode: Full Node (Truth + Intelligence)");
            }

            println!("Use 'lite-vision validator' to join consensus");
            ExitCode::SUCCESS
        }

        Commands::Validator {
            stake,
            rpc_host,
            rpc_port,
        } => {
            let stake = stake.unwrap_or(1000);
            println!("Starting validator with {} LVU stake", stake);
            println!("Connecting to RPC: {}:{}", rpc_host, rpc_port);
            ExitCode::SUCCESS
        }

        Commands::Operator {
            gpu_model,
            rpc_host,
            rpc_port,
        } => {
            let gpu = gpu_model.unwrap_or_else(|| "auto-detect".to_string());
            println!("Starting operator with GPU: {}", gpu);
            println!("Connecting to RPC: {}:{}", rpc_host, rpc_port);
            ExitCode::SUCCESS
        }

        Commands::Submit {
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
            ExitCode::SUCCESS
        }

        Commands::Status { rpc_host, rpc_port } => {
            if let (Some(host), Some(port)) = (rpc_host, rpc_port) {
                println!("Querying: {}:{}", host, port);
            }
            println!("Network Status: Ready (mock)");
            println!("Truth Plane: Active");
            println!("Intelligence Plane: Active");
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
                println!("Job status: Ready");
            } else {
                println!("Error: --job-id required");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }

        Commands::Version => {
            println!("Lite-Vision v{}", env!("CARGO_PKG_VERSION"));
            println!("Truth Plane: BFT consensus");
            println!("Intelligence Plane: Job execution");
            println!("RPACK: Render packet format");
            ExitCode::SUCCESS
        }

        Commands::Info => {
            println!("Lite-Vision Implementation");
            println!("");
            println!("Truth Plane (0100-series):");
            println!("  - BFT Consensus (0101)");
            println!("  - Validator Set (0102)");
            println!("  - State Machine (0103)");
            println!("  - RPC Server (0100)");
            println!("  - State Sync");
            println!("  - Pruning/Archive (0106)");
            println!("");
            println!("Intelligence Plane (0200-series):");
            println!("  - Job Model (0203)");
            println!("  - Routing (0204)");
            println!("  - Receipts (0205)");
            println!("  - Verification (0206)");
            println!("  - Disputes (0207)");
            println!("");
            println!("RPACK (0300-series):");
            println!("  - Container (0301)");
            println!("  - Scene IR (0302)");
            println!("  - Assets (0303)");
            println!("  - Deltas (0304)");
            println!("");
            println!("Storage (0400-series):");
            println!("  - Memory Model (0400)");
            println!("  - CRDTs (0401)");
            println!("  - Partitions (0402)");
            println!("  - Artifacts (0404)");
            println!("");
            println!("Network (0500-series):");
            println!("  - P2P (0500)");
            println!("  - Observability (0502)");
            ExitCode::SUCCESS
        }
    }
}
