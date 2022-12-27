# FEVM Test Vectors

`fevm-test-vectors` generate test vector from geth rpc with debug namespace enabled.

## Build

``` bash
cargo build
```

## Command

**extract evm transaction**

Extract transaction details file through `evm tracing` (including contract slot changes, balance changes, and bytecodes, etc.).

``` bash
RUST_LOG=info fevm-test-vectors extract-evm --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out-dir <OUT_DIR> 
```

**generate test vector**

Generate test vector from evm transation file.

``` bash
RUST_LOG=info fevm-test-vectors trans --in-dir <IN_DIR> --out-dir <OUT_DIR>
```

Generate test vector from geth rpc directly.

``` bash
RUST_LOG=info fevm-test-vectors extract --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out-dir <OUT_DIR>
```

## License

Dual-licensed under [MIT](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-MIT)

+ [Apache 2.0](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-APACHE)
