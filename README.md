# FEVM Test Vectors

`fevm-test-vectors` generate [test vector](https://github.com/filecoin-project/test-vectors) from geth rpc with debug namespace enabled.

## Build

``` bash
cargo build --release
```

## Command

**extract ethereum transaction**

Extract transaction detail file through `evm tracing` (including contract slot changes, balance changes, and bytecodes, etc.).

``` bash
RUST_LOG=info fevm-test-vectors extract-transaction --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out-dir <OUT_DIR> 
```

**generate test vector**

Generate test vector from transation detail file.

``` bash
RUST_LOG=info fevm-test-vectors generate-from-file --input <IN_FILE|IN_DIR> --out-dir <OUT_DIR>
```

Generate test vector from geth rpc directly.

``` bash
RUST_LOG=info fevm-test-vectors generate --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out-dir <OUT_DIR>
```

## Consume Test Vector

Use [froghub-io/ref-fvm](https://github.com/froghub-io/ref-fvm/tree/testing-evm-vectors) to consume test vectors.

``` bash
RUST_LOG=info CARGO_PROFILE_BENCH_DEBUG=true VECTOR=fevm-test-vectors/corpus/test_vectors/xxx.json cargo bench --bench  bench_conformance -- --nocapture
```

## License

Dual-licensed under [MIT](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-MIT)

+ [Apache 2.0](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-APACHE)
