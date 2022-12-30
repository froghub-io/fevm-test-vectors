use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use async_std::task::block_on;
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
    Generate(Generate),
    ExtractTransaction(ExtractTransaction),
    GenerateFromFile(GenerateFromFile),
}

#[derive(Debug, Parser)]
#[clap(about = "Generate test vector from geth rpc directly.", long_about = None)]
pub struct Generate {
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
#[clap(about = "Extract transaction detail file through evm tracing.", long_about = None)]
pub struct ExtractTransaction {
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
#[clap(about = "Generate test vector from transation detail file.", long_about = None)]
pub struct GenerateFromFile {
    /// evm test vector input file/dir path
    #[clap(short, long)]
    input: String,

    /// fvm test vector output dir path
    #[clap(short, long)]
    out_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_log();
    let cli = Cli::parse();
    match cli.cmd {
        SubCommand::Generate(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let evm_input = block_on(extract_transaction(
                &config.tx_hash,
                &config.geth_rpc_endpoint,
            ))?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            block_on(export_test_vector_file(evm_input, path))?;
        }
        SubCommand::ExtractTransaction(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let evm_input = block_on(extract_transaction(
                &config.tx_hash,
                &config.geth_rpc_endpoint,
            ))?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            let output = File::create(&path)?;
            serde_json::to_writer_pretty(output, &evm_input)?;
        }
        SubCommand::GenerateFromFile(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let input = Path::new(&config.input);
            if input.is_dir() {
                let files: Vec<PathBuf> = WalkDir::new(input)
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
                    block_on(export_test_vector_file(evm_input, path))?;
                }
            } else {
                let file_name = input.file_name().unwrap().to_str().unwrap();
                let file = File::open(input.clone())?;
                let reader = BufReader::new(file);
                let evm_input: EvmContractInput = serde_json::from_reader(reader)
                    .expect(&*format!("Serialization failed: {:?}", input));
                let path = out_dir.join(file_name);
                block_on(export_test_vector_file(evm_input, path))?;
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
