use std::collections::BTreeMap;
use std::str::FromStr;

use ethers::prelude::*;
use ethers::providers::{Middleware, Provider};
use ethers::utils;
use ethers::utils::get_contract_address;

use super::opcodes::*;
use crate::extractor::types::{EthState, EthTransactionTestVector};

pub async fn extract_eth_transaction_test_vector<P: JsonRpcClient>(
    provider: &Provider<P>,
    tx_hash: H256,
) -> anyhow::Result<EthTransactionTestVector> {
    let transaction = provider.get_transaction(tx_hash).await?.unwrap();

    let block = provider
        .get_block_with_txs(transaction.block_hash.unwrap())
        .await?
        .unwrap();

    let mut block_hashes = BTreeMap::new();
    block_hashes.insert(block.number.unwrap().as_u64(), block.hash.unwrap());

    let tx_from = transaction.from;
    let tx_contract_address = transaction
        .to
        .unwrap_or_else(|| get_contract_address(tx_from, transaction.nonce));

    let prestate_tracing_options: GethDebugTracingOptions = GethDebugTracingOptions {
        tracer: Some("prestateTracer".to_owned()),
        ..Default::default()
    };
    let prestate: EthState = provider
        .request(
            "debug_traceTransaction",
            [
                utils::serialize(&tx_hash),
                utils::serialize(&prestate_tracing_options),
            ],
        )
        .await?;

    let mut poststate = prestate.clone();

    let trace_options: GethDebugTracingOptions = GethDebugTracingOptions {
        disable_storage: Some(true),
        enable_memory: Some(false),
        disable_stack: Some(false),
        enable_return_data: Some(true),
        ..Default::default()
    };
    let transaction_trace = provider
        .debug_trace_transaction(tx_hash, trace_options)
        .await?;

    // calculate gas fee(including leftover gas)
    let gas_price = transaction.gas_price.unwrap();
    let gas_fee = gas_price * transaction.gas;
    let account_state = poststate.get_mut(&tx_from).unwrap();
    account_state.balance -= gas_fee;

    let mut execution_contexts = vec![tx_contract_address];
    let mut snapshots = vec![poststate.clone()];

    if transaction.to.is_none() {
        // FIXME the contract may have self-destructed
        let code = provider.get_code(tx_contract_address, None).await?;
        let eth_account_state = poststate.get_mut(&tx_contract_address).unwrap();
        eth_account_state.code = code;
    }

    // transaction value transfer
    if !transaction.value.is_zero() {
        let account_state = poststate.get_mut(&tx_from).unwrap();
        account_state.balance -= transaction.value;
        let account_state = poststate.get_mut(&tx_from).unwrap();
        account_state.balance += transaction.value
    }

    let mut depth = 1u64;
    let mut i = 0;
    while i < transaction_trace.struct_logs.len() {
        let log = &transaction_trace.struct_logs[i];

        if depth > log.depth {
            depth = log.depth;
            execution_contexts.truncate(depth.try_into().unwrap());
            snapshots.truncate(depth.try_into().unwrap());
        }

        match log.op.as_str() {
            OP_SLOAD => {}
            OP_SSTORE => {
                let stack = log.stack.as_ref().unwrap();

                let key = U256_to_H256(stack[stack.len() - 1]);
                let val = U256_to_H256(stack[stack.len() - 2]);

                let account_state = poststate
                    .get_mut(execution_contexts.last().unwrap())
                    .unwrap();
                account_state.storage.insert(key, val);
            }
            OP_CALL => {
                snapshots.push(poststate.clone());

                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 2]);

                let value = stack[stack.len() - 3];

                if !value.is_zero() {
                    let caller = execution_contexts.last().unwrap();

                    let caller_account_state = poststate.get_mut(caller).unwrap();

                    // the call will fail silently without error in trace logs and there's no "revert".
                    if depth <= 1024 && caller_account_state.balance >= value {
                        caller_account_state.balance -= value;

                        let callee_account_state = poststate.get_mut(&address).unwrap();
                        callee_account_state.balance += value;
                    }
                }

                execution_contexts.push(address);

                depth += 1;
            }
            OP_STATICCALL => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 2]);

                execution_contexts.push(address);
                snapshots.push(poststate.clone());

                depth += 1;
            }
            OP_DELEGATECALL => {
                execution_contexts.push(*execution_contexts.last().unwrap());
                snapshots.push(poststate.clone());

                depth += 1;
            }
            OP_CALLCODE => {
                execution_contexts.push(*execution_contexts.last().unwrap());
                snapshots.push(poststate.clone());

                depth += 1;
            }
            OP_CREATE => {
                snapshots.push(poststate.clone());

                let stack = log.stack.as_ref().unwrap();

                let value = stack[stack.len() - 1];

                let mut address = H160::zero();
                for log in &transaction_trace.struct_logs[i + 1..] {
                    if log.depth == depth {
                        let stack = log.stack.as_ref().unwrap();
                        address = decode_address(stack[stack.len() - 1]);
                        break;
                    }
                }

                let caller = execution_contexts.last().unwrap();

                // the call will fail silently without error in trace logs and there's no "revert".
                if depth <= 1024 && poststate.get(caller).unwrap().balance >= value {
                    if !value.is_zero() {
                        poststate.get_mut(caller).unwrap().balance -= value;

                        poststate.get_mut(&address).unwrap().balance += value;
                    }

                    poststate.get_mut(caller).unwrap().nonce += 1;

                    // FIXME
                    let code = provider.get_code(address, None).await?;
                    poststate.get_mut(&address).unwrap().code = code;
                }

                execution_contexts.push(address);

                depth += 1;
            }
            OP_CREATE2 => {
                snapshots.push(poststate.clone());

                let stack = log.stack.as_ref().unwrap();

                let value = stack[stack.len() - 1];

                let mut address = H160::zero();
                for log in &transaction_trace.struct_logs[i + 1..] {
                    if log.depth == depth {
                        let stack = log.stack.as_ref().unwrap();
                        address = decode_address(stack[stack.len() - 1]);
                        break;
                    }
                }

                let caller = execution_contexts.last().unwrap();

                // the call will fail silently without error in trace logs and there's no "revert".
                if depth <= 1024 && poststate.get(caller).unwrap().balance >= value {
                    if !value.is_zero() {
                        poststate.get_mut(caller).unwrap().balance -= value;

                        poststate.get_mut(&address).unwrap().balance += value;
                    }

                    poststate.get_mut(caller).unwrap().nonce += 1;

                    // FIXME
                    let code = provider.get_code(address, None).await?;
                    poststate.get_mut(&address).unwrap().code = code;
                }

                execution_contexts.push(address);

                depth += 1;
            }
            OP_SELFDESTRUCT => {
                let stack = log.stack.as_ref().unwrap();
                let beneficiary = decode_address(stack[stack.len() - 1]);

                let caller = execution_contexts.last().unwrap();

                let caller_balance = poststate.get_mut(caller).unwrap().balance;
                if caller_balance != 0.into() {
                    poststate.get_mut(&beneficiary).unwrap().balance += caller_balance;
                }

                // consider delete the account?
                let caller_account_state = poststate.get_mut(caller).unwrap();
                caller_account_state.balance = 0.into();
                caller_account_state.nonce = 0;
                caller_account_state.code = Bytes::default();
            }
            OP_BALANCE => {}
            OP_SELFBALANCE => {}
            OP_EXTCODESIZE => {}
            OP_EXTCODECOPY => {}
            OP_EXTCODEHASH => {}
            OP_BLOCKHASH => {
                let stack = log.stack.as_ref().unwrap();

                let stack_after = transaction_trace.struct_logs[i + 1].clone().stack.unwrap();

                let num = stack[stack.len() - 1].as_u64();
                let hash = stack_after[stack_after.len() - 1];
                let mut bytes = [0; 32];
                hash.to_big_endian(&mut bytes);
                block_hashes.insert(num, bytes.into());
            }
            OP_REVERT => {
                poststate = snapshots.pop().unwrap();
            }
            OP_INVALID => {
                poststate = snapshots.pop().unwrap();
            }
            _ => (),
        }

        if log.error.is_some() {
            poststate = snapshots.pop().unwrap();
        }
        i += 1;
    }

    // refund unused gas
    // TODO  some opcodes(e.g. SSTORE) have additional gas refund.
    let leftover_gas = transaction.gas - transaction_trace.gas;
    poststate.get_mut(&tx_from).unwrap().balance += leftover_gas * gas_price;

    let eth_transaction_test_vector = EthTransactionTestVector {
        hash: transaction.hash,
        nonce: transaction.nonce.as_u64(),
        from: transaction.from,
        to: transaction.to.unwrap_or_else(|| H160::zero()),
        value: transaction.value,
        input: transaction.input,
        gas: transaction.gas,
        gas_price: transaction.gas_price.unwrap(),
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
        max_fee_per_gas: transaction.max_fee_per_gas,
        status: if transaction_trace.failed { 0 } else { 1 },
        gas_used: transaction_trace.gas.into(),
        return_value: transaction_trace.return_value,
        coinbase: block.author.unwrap(),
        base_fee_per_gas: block.base_fee_per_gas,
        difficultly: block.difficulty,
        chain_id: transaction.chain_id.unwrap(),
        block_number: block.number.unwrap().as_u64(),
        block_hashes,
        block_mix_hash: block.mix_hash,
        timestamp: block.timestamp,
        prestate,
        poststate,
    };
    Ok(eth_transaction_test_vector)
}

fn decode_address(raw_address: U256) -> H160 {
    let mut bytes = [0; 32];
    raw_address.to_big_endian(&mut bytes);
    H160::from_slice(&bytes[12..])
}

fn U256_to_H256(val: U256) -> H256 {
    let mut bytes = [0; 32];
    val.to_big_endian(&mut bytes);
    H256::from_slice(&bytes)
}

#[tokio::test]
async fn test_extract() {
    let tx_hash =
        H256::from_str("0xa1ab514169a899fcea29144a8eb6a4613b46f2108e15b0a5d43afd44e0baa839")
            .unwrap();
    let provider = Provider::<Http>::try_from("http://localhost:8546")
        .expect("could not instantiate HTTP Provider");

    let r = extract_eth_transaction_test_vector(&provider, tx_hash)
        .await
        .unwrap();
    dbg!(&r.prestate, &r.poststate);
}
