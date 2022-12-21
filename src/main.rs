use std::path::Path;
use clap::Parser;
use fevm_test_vectors::export_test_vector_file;
use fevm_test_vectors::extract_evm::run_extract;

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
    let cli = Cli::parse();
    let input = run_extract(cli.geth_rpc_endpoint, cli.tx_hash).await?;
    export_test_vector_file(
        input,
        Path::new(&cli.out).to_path_buf(),
    ).await?;
    Ok(())
}
