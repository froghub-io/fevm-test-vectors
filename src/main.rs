use std::env;
use std::fs::File;
use std::path::Path;

use clap::Parser;
use fevm_test_vectors::extract_evm::run_extract;
use fevm_test_vectors::{export_test_vector_file, init_log};

#[derive(Parser, Debug)]
#[clap(name = env!("CARGO_PKG_NAME"))]
#[clap(version = env!("CARGO_PKG_VERSION"))]
#[clap(about = "Generate a test vector by extracting it from a live chain.", long_about = None)]
struct Cli {
    #[clap(default_value = "http://localhost:8545", short, long)]
    geth_rpc_endpoint: String,

    /// eth transaction hash
    #[clap(short, long)]
    tx_hash: String,

    /// test-vector file output path ( such as: /a/b/xx/xxx.json )
    #[clap(short, long)]
    out: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_log();
    let cli = Cli::parse();
    let input = run_extract(cli.geth_rpc_endpoint, cli.tx_hash).await?;
    let contract_out = env::var("CONTRACT_OUT");
    match contract_out {
        Ok(contract_out) => {
            let path = Path::new(&contract_out).to_path_buf();
            log::info!("contract_path: {:?}", path);
            let output = File::create(&path)?;
            serde_json::to_writer_pretty(output, &input)?;
        }
        Err(_) => {}
    }
    let path = Path::new(&cli.out).to_path_buf();
    log::info!("test_vector_path: {:?}", path);
    export_test_vector_file(input, path).await?;
    Ok(())
}
