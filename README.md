# FEVM Test Vectors

FVM 2.1 which introduces support for Ethereum smart contracts in the Filecoin network (FEVM) will be the biggest network
upgrade since mainnet launch. At the same time, it immensely widens the area of possible problems. That’s why it must be
tested thoroughly. One of possibilities to test FEVM is to run already existing and proven smart contracts from Ethereum
and compare the state it produces on Ethereum vs Filecoin.

For some reasons (signatures, contracts involving some long history, etc.), transactions on Ethereum cannot be fully
replayed on the testnet. So we verify FEVM in this way: before replaying a transaction, we will import the transaction
into the FEVM of the state (usually there are multiple contracts) involved in Ethereum, and then execute the
transaction, In the end we only compare the slots that were modified in the execution of this transaction, not all the
slots in the storage of all contracts. for comparison (this involves OpCode SLOAD/SSTORE).

Collect the Stack Memory Storage corresponding to the transaction from Ethereum, which contains the data read by SLOAD
and the data written by SSTORE, which is used to generate the Test Vector JSON file.

FEVM consumes Test Vector. It involves migrating SLOAD data to BlockStore and intercepting SSTORE data, by comparing it
with the expected SSTORE data in the Test Vector to get a test report.

## Release

``` bash
make build --release
```

## CLI: fevm-test-vectors

fevm-test-vectors extract extracts a test vector from a live network. It requires access to a GETH client that exposes
the standard GETH-RPC-ENDPOINT API endpoint. It has three subcommands.

- extract: Generate a fvm test vector by extracting it from a live chain.

``` bash
fevm-test-vectors extract --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out-dir <OUT_DIR>
```

- extract-evm: Generate a evm test vector by extracting it from a live chain.

``` bash
fevm-test-vectors extract-evm --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out-dir <OUT_DIR> 
```

- trans: Evm test vector to fvm test vector

``` bash
fevm-test-vectors trans --in-dir <IN_DIR> --out-dir <OUT_DIR>
```

## License

Dual-licensed under [MIT](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-MIT)

+ [Apache 2.0](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-APACHE)
