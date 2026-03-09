use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lite-vision")]
#[command(about = "Lite-Vision CLI - CPU-secured truth, GPU-powered intelligence")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Node {
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,

        #[arg(long, default_value_t = 8080)]
        port: u16,
    },
    Validator {
        #[arg(long)]
        stake: Option<u64>,
    },
    Operator {
        #[arg(long)]
        gpu_model: Option<String>,
    },
    Job {
        #[arg(long)]
        kernel: String,

        #[arg(long)]
        input: String,

        #[arg(long, default_value_t = 1000)]
        budget: u64,
    },
    Status,
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Node { bind, port } => {
            println!("Starting Lite-Vision node on {}:{}", bind, port);
        }
        Commands::Validator { stake } => {
            println!("Starting validator with stake: {:?}", stake);
        }
        Commands::Operator { gpu_model } => {
            println!("Starting operator with GPU: {:?}", gpu_model);
        }
        Commands::Job {
            kernel,
            input,
            budget,
        } => {
            println!(
                "Submitting job: kernel={}, input={}, budget={}",
                kernel, input, budget
            );
        }
        Commands::Status => {
            println!("Lite-Vision Status: OK");
        }
        Commands::Version => {
            println!("Lite-Vision v{}", env!("CARGO_PKG_VERSION"));
        }
    }
}
