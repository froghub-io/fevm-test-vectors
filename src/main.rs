use std::fmt::format;
use std::fs::{File, FileType};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use fevm_test_vectors::extractor::extract_transaction;
use fevm_test_vectors::types::EvmContractInput;
use fevm_test_vectors::{export_test_vector_file, init_log};
use walkdir::{DirEntry, WalkDir};

#[derive(Parser, Debug)]
pub struct Cli {
    #[clap(subcommand)]
    cmd: SubCommand,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    Extract(Extract),
    ExtractEvm(ExtractEvm),
    Trans(Trans),
}

#[derive(Debug, Parser)]
#[clap(about = "Generate test vector from geth rpc directly.", long_about = None)]
pub struct Extract {
    #[clap(short, long)]
    geth_rpc_endpoint: String,

    /// eth transaction hash
    #[clap(short, long)]
    tx_hash: String,

    /// test vector output dir path
    #[clap(short, long)]
    out_dir: String,
}

#[derive(Debug, Parser)]
#[clap(about = "Extract transaction details file through `evm tracing`.", long_about = None)]
pub struct ExtractEvm {
    #[clap(short, long)]
    geth_rpc_endpoint: String,

    /// eth transaction hash
    #[clap(short, long)]
    tx_hash: String,

    /// test vector output dir path
    #[clap(short, long)]
    out_dir: String,
}

#[derive(Debug, Parser)]
#[clap(about = "Generate test vector from evm transation file.", long_about = None)]
pub struct Trans {
    /// evm test vector input dir path
    #[clap(short, long)]
    in_dir: String,

    /// fvm test vector output dir path
    #[clap(short, long)]
    out_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_log();
    let cli = Cli::parse();
    match cli.cmd {
        SubCommand::Extract(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let evm_input = extract_transaction(&config.tx_hash, &config.geth_rpc_endpoint).await?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            export_test_vector_file(evm_input, path).await?;
        }
        SubCommand::ExtractEvm(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let evm_input = extract_transaction(&config.tx_hash, &config.geth_rpc_endpoint).await?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            let output = File::create(&path)?;
            serde_json::to_writer_pretty(output, &evm_input)?;
        }
        SubCommand::Trans(config) => {
            let in_dir = Path::new(&config.in_dir);
            assert!(in_dir.is_dir(), "in_dir must directory");
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let files: Vec<PathBuf> = WalkDir::new(in_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_runnable)
                .map(|e| e.path().to_path_buf())
                .collect();

            for p in files {
                let file_name = p.file_name().unwrap().to_str().unwrap();
                let file = File::open(p.clone())?;
                let reader = BufReader::new(file);
                let evm_input: EvmContractInput = serde_json::from_reader(reader)
                    .expect(&*format!("Serialization failed: {:?}", p));
                let path = out_dir.join(file_name);
                export_test_vector_file(evm_input, path).await?;
            }
        }
    }
    Ok(())
}

pub fn is_runnable(entry: &DirEntry) -> bool {
    let file_name = match entry.path().to_str() {
        Some(file) => file,
        None => return false,
    };

    file_name.ends_with(".json")
}
