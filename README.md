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

## Usage

```
// Not required. If set, contract json information can be exported
export CONTRACT_OUT=<CONTRACT_OUT> 
export RUST_LOG=info

cargo run --package fevm-test-vectors --bin fevm-test-vectors -- --geth-rpc-endpoint <GETH_RPC_ENDPOINT> --tx-hash <TX_HASH> --out <OUT>
```

## License

Dual-licensed under [MIT](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-MIT)

+ [Apache 2.0](https://github.com/froghub-io/fevm-test-vectors/blob/main/LICENSE-APACHE)
