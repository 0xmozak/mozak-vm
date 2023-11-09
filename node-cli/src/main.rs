use clap::{Parser, Subcommand};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// logger level
    #[arg(short, long)]
    loglevel: String,

    /// Run a mozak-node
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate default config for mozak-node
    NodeCFGGen {
        /// config file
        #[arg(short, long)]
        cfg: String,
    },
    /// Run a mozak-node
    RunNode {
        /// config file
        #[arg(short, long)]
        cfg: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::NodeCFGGen { cfg }) => {
            mozak_node::config::generate_default_and_save(cfg);
        }
        Some(Commands::RunNode { cfg }) => mozak_node::node::start(cfg),
        None => {}
    }
}
