pub mod opcodes;

use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::str::FromStr;

use ethers::prelude::*;
use ethers::providers::{Http, Middleware, Provider};
use ethers::utils::get_contract_address;

use self::opcodes::*;
use crate::types::{
    EvmContractBalance, EvmContractContext, EvmContractInput, EvmContractState,
    EvmContractTransaction,
};

pub async fn extract_transaction(
    hash: &str,
    geth_rpc_endpoint: &str,
) -> anyhow::Result<EvmContractInput> {
    let tx_hash = H256::from_str(hash)?;

    let provider =
        Provider::<Http>::try_from(geth_rpc_endpoint).expect("could not instantiate HTTP Provider");

    let transaction = provider.get_transaction(tx_hash).await?.unwrap();

    let block = provider
        .get_block_with_txs(transaction.block_hash.unwrap())
        .await?
        .unwrap();

    let tx_from = transaction.from;
    let tx_contract_address = transaction
        .to
        .unwrap_or_else(|| get_contract_address(tx_from, transaction.nonce));

    let mut pre_storages = BTreeMap::new();
    let mut post_storages = BTreeMap::new();

    let mut pre_balances = BTreeMap::new();
    let mut post_balances = BTreeMap::new();
    let mut post_balances_negative = BTreeMap::new();

    let mut pre_codes = BTreeMap::new();
    let mut post_codes = BTreeMap::new();

    let mut number_to_hash = BTreeMap::new();
    number_to_hash.insert(block.number.unwrap().as_u64(), block.hash.unwrap());

    // trace current transaction
    let trace_options: GethDebugTracingOptions = GethDebugTracingOptions {
        disable_storage: Some(true),
        enable_memory: Some(false),
        disable_stack: Some(false),
        ..Default::default()
    };
    let transaction_trace = provider
        .debug_trace_transaction(tx_hash, trace_options)
        .await?;

    // calculate gas fee
    let gas_used: U256 = transaction_trace.gas.into();
    let gas_price = transaction.gas_price.unwrap();
    let gas_fee = gas_used * gas_price;
    pre_balances.insert(tx_from, U256::zero());
    post_balances_negative.insert(tx_from, gas_fee);

    let mut execution_context = vec![tx_contract_address];
    let mut snapshots = vec![(
        post_storages.clone(),
        post_codes.clone(),
        post_balances.clone(),
        post_balances_negative.clone(),
    )];

    // TODO some contracts may have "selfdestructed"
    let code = provider.get_code(tx_contract_address, None).await?;
    if transaction.to.is_some() {
        pre_codes.insert(tx_contract_address, code.clone());
    }
    post_codes.insert(tx_contract_address, code);

    // transaction value transfer
    post_balances.insert(tx_contract_address, transaction.value);
    post_balances_negative.insert(tx_from, transaction.value + gas_fee);

    let mut depth = 1u64;
    let mut i = 0;
    while i < transaction_trace.struct_logs.len() {
        let log = &transaction_trace.struct_logs[i];

        if depth > log.depth {
            depth = log.depth;
            execution_context.truncate(depth.try_into().unwrap());
            snapshots.truncate(depth.try_into().unwrap());
        }

        match log.op.as_str() {
            OP_SLOAD => {
                let stack = log.stack.as_ref().unwrap();

                let key = stack[stack.len() - 1];

                let next_log = &transaction_trace.struct_logs[i + 1];
                let next_log_stack = next_log.stack.as_ref().unwrap();
                let val = next_log_stack[next_log_stack.len() - 1];

                // insert if not exist
                pre_storages
                    .entry(*execution_context.last().unwrap())
                    .or_insert(BTreeMap::new())
                    .entry(key)
                    .or_insert(val);
            }
            OP_SSTORE => {
                let stack = log.stack.as_ref().unwrap();

                let key = stack[stack.len() - 1];
                let val = stack[stack.len() - 2];

                post_storages
                    .entry(*execution_context.last().unwrap())
                    .or_insert(BTreeMap::new())
                    .insert(key, val);
            }
            OP_CALL => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 2]);

                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }

                let value = stack[stack.len() - 3];
                let next_log = &transaction_trace.struct_logs[i + 1];
                // In some cases, e.g. insufficient balance for transfer, the call will fail without error
                // or "revert" opcode in trace logs.
                let failed = next_log.depth == log.depth;
                if !value.is_zero() && !failed {
                    let caller = *execution_context.last().unwrap();

                    pre_balances.insert(address, U256::zero());
                    pre_balances.insert(caller, U256::zero());

                    post_balances.insert(address, value);
                    post_balances_negative.insert(caller, value);
                }

                execution_context.push(address);
                snapshots.push((
                    post_storages.clone(),
                    post_codes.clone(),
                    post_balances.clone(),
                    post_balances_negative.clone(),
                ));

                depth += 1;
            }
            OP_STATICCALL => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 2]);

                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }

                execution_context.push(address);
                snapshots.push((
                    post_storages.clone(),
                    post_codes.clone(),
                    post_balances.clone(),
                    post_balances_negative.clone(),
                ));

                depth += 1;
            }
            OP_DELEGATECALL => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 2]);

                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }

                execution_context.push(*execution_context.last().unwrap());
                snapshots.push((
                    post_storages.clone(),
                    post_codes.clone(),
                    post_balances.clone(),
                    post_balances_negative.clone(),
                ));

                depth += 1;
            }
            OP_CALLCODE => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 2]);

                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }

                execution_context.push(*execution_context.last().unwrap());
                snapshots.push((
                    post_storages.clone(),
                    post_codes.clone(),
                    post_balances.clone(),
                    post_balances_negative.clone(),
                ));

                depth += 1;
            }
            OP_CREATE => {
                let stack = log.stack.as_ref().unwrap();

                let caller = *execution_context.last().unwrap();

                let mut address = H160::zero();
                for log in &transaction_trace.struct_logs[i + 1..] {
                    if log.depth == depth {
                        let stack = log.stack.as_ref().unwrap();
                        address = decode_address(stack[stack.len() - 1]);
                        break;
                    }
                }

                let value = stack[stack.len() - 1];
                let next_log = &transaction_trace.struct_logs[i + 1];
                // In some cases, e.g. insufficient balance for transfer, the call will fail without error.
                let failed = next_log.depth == log.depth;
                if !value.is_zero() && !failed {
                    pre_balances.insert(address, U256::zero());
                    pre_balances.insert(caller, U256::zero());

                    post_balances.insert(address, value);
                    post_balances_negative.insert(caller, value);
                }

                let code = provider.get_code(address, None).await?;
                post_codes.insert(address, code);

                execution_context.push(address);
                snapshots.push((
                    post_storages.clone(),
                    post_codes.clone(),
                    post_balances.clone(),
                    post_balances_negative.clone(),
                ));

                depth += 1;
            }
            OP_CREATE2 => {
                let stack = log.stack.as_ref().unwrap();

                let caller = *execution_context.last().unwrap();

                let mut address = H160::zero();
                for log in &transaction_trace.struct_logs[i + 1..] {
                    if log.depth == depth {
                        let stack = log.stack.as_ref().unwrap();
                        address = decode_address(stack[stack.len() - 1]);
                        break;
                    }
                }

                let value = stack[stack.len() - 1];
                let next_log = &transaction_trace.struct_logs[i + 1];
                // In some cases, e.g. insufficient balance for transfer, the call will fail without error.
                let failed = next_log.depth == log.depth;
                if !value.is_zero() && !failed {
                    pre_balances.insert(address, U256::zero());
                    pre_balances.insert(caller, U256::zero());

                    post_balances.insert(address, value);
                    post_balances_negative.insert(caller, value);
                }

                let code = provider.get_code(address, None).await?;
                post_codes.insert(address, code);

                execution_context.push(address);
                snapshots.push((
                    post_storages.clone(),
                    post_codes.clone(),
                    post_balances.clone(),
                    post_balances_negative.clone(),
                ));

                depth += 1;
            }
            OP_SELFDESTRUCT => {
                // TODO
            }
            OP_BALANCE => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 1]);
                pre_balances.entry(address).or_insert(U256::zero());
            }
            OP_SELFBALANCE => {
                let address = *execution_context.last().unwrap();
                pre_balances.entry(address).or_insert(U256::zero());
            }
            OP_EXTCODESIZE => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 1]);
                // there's a possibility that the address didn't have code yet at this time,
                // but it may do in the future, and vice versa.
                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }
            }
            OP_EXTCODECOPY => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 1]);

                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }
            }
            OP_EXTCODEHASH => {
                let stack = log.stack.as_ref().unwrap();

                let address = decode_address(stack[stack.len() - 1]);

                if pre_codes.get(&address).is_none() {
                    let code = provider.get_code(address, None).await?;
                    pre_codes.insert(address, code.clone());
                }
            }
            OP_BLOCKHASH => {
                let stack = log.stack.as_ref().unwrap();

                let stack_after = transaction_trace.struct_logs[i + 1].clone().stack.unwrap();

                let num = stack[stack.len() - 1].as_u64();
                let hash = stack_after[stack_after.len() - 1];
                let mut bytes = [0; 32];
                hash.to_big_endian(&mut bytes);
                number_to_hash.insert(num, bytes.into());
            }
            OP_REVERT => {
                (
                    post_storages,
                    post_codes,
                    post_balances,
                    post_balances_negative,
                ) = snapshots.pop().unwrap();
            }
            OP_INVALID => {
                (
                    post_storages,
                    post_codes,
                    post_balances,
                    post_balances_negative,
                ) = snapshots.pop().unwrap();
            }
            _ => (),
        }

        if log.error.is_some() {
            (
                post_storages,
                post_codes,
                post_balances,
                post_balances_negative,
            ) = snapshots.pop().unwrap();
        }
        i += 1;
    }

    // populate_balance_at_block_number_and_index(
    //     &mut pre_balances,
    //     transaction.block_number.unwrap(),
    //     transaction.transaction_index.unwrap(),
    //     geth_rpc_endpoint,
    // )
    // .await?;

    // apply initial states to post-transaction states
    for (address, initial_balance) in pre_balances.iter() {
        if let Some(balance) = post_balances.get_mut(address) {
            *balance += *initial_balance;
        } else {
            post_balances.insert(*address, *initial_balance);
        }
    }
    // for (address, negative_value) in post_balances_negative {
    //     let balance = post_balances.get_mut(&address).unwrap();
    //     *balance -= negative_value;
    // }
    for (address, pre_storage) in pre_storages.iter() {
        if let Some(post_storage) = post_storages.get_mut(address) {
            for (key, val) in pre_storage {
                post_storage.entry(*key).or_insert(*val);
            }
        } else {
            post_storages.insert(*address, pre_storage.clone());
        }
    }
    for (address, code) in pre_codes.iter() {
        post_codes.insert(*address, code.clone());
    }

    // generate intermediate format that will then used to generate test vector
    let status = if transaction_trace.failed { 0 } else { 1 };
    let return_result = hex::encode(transaction_trace.return_value.to_vec());

    let input = eth_tx_to_input(
        tx_hash,
        transaction,
        block,
        pre_storages,
        post_storages,
        pre_balances,
        post_balances,
        pre_codes,
        post_codes,
        number_to_hash,
        status,
        return_result,
    );
    Ok(input)
}

fn decode_address(raw_address: U256) -> H160 {
    let mut bytes = [0; 32];
    raw_address.to_big_endian(&mut bytes);
    H160::from_slice(&bytes[12..])
}

fn push_adds<T>(adds: &mut Vec<Address>, bmap: &BTreeMap<Address, T>) {
    bmap.keys().for_each(|addr| {
        if !adds.contains(addr) {
            adds.push(addr.clone());
        }
    });
}

fn get_storage(
    addr: &Address,
    storages: &BTreeMap<Address, BTreeMap<U256, U256>>,
) -> HashMap<String, String> {
    let bmap = storages.get(&addr);
    let mut vmap = HashMap::new();
    if let Some(bmap) = bmap {
        for (k, v) in bmap {
            vmap.insert(u256_to_str(k), u256_to_str(v));
        }
    }
    vmap
}

fn get_balance(addr: &Address, balances: &BTreeMap<Address, U256>) -> String {
    match balances.get(&addr) {
        Some(v) => u256_to_str(v),
        None => String::from("00"),
    }
}

fn get_code(addr: &Address, codes: &BTreeMap<Address, Bytes>) -> Option<String> {
    match codes.get(&addr) {
        Some(v) => Some(hex::encode(v.to_vec())),
        None => None,
    }
}

fn get_eth_addr(addr: Option<Address>) -> String {
    match addr {
        Some(addr) => hex::encode(addr.0),
        None => String::from("0x00"),
    }
}

fn u256_to_str(v: &U256) -> String {
    let mut value = [0u8; 32];
    v.to_big_endian(&mut value);
    hex::encode(value)
}

fn h256_to_str(v: &H256) -> String {
    hex::encode(v.0)
}

fn eth_tx_to_input(
    tx_hash: H256,
    transaction: Transaction,
    block: Block<Transaction>,
    pre_storages: BTreeMap<Address, BTreeMap<U256, U256>>,
    post_storages: BTreeMap<Address, BTreeMap<U256, U256>>,
    pre_balances: BTreeMap<Address, U256>,
    post_balances: BTreeMap<Address, U256>,
    pre_codes: BTreeMap<Address, Bytes>,
    post_codes: BTreeMap<Address, Bytes>,
    number_to_hash: BTreeMap<u64, H256>,
    status: usize,
    return_result: String,
) -> EvmContractInput {
    let mut adds: Vec<Address> = Vec::new();
    push_adds(&mut adds, &pre_storages);
    push_adds(&mut adds, &post_storages);
    push_adds(&mut adds, &pre_balances);
    push_adds(&mut adds, &post_balances);
    push_adds(&mut adds, &pre_codes);
    push_adds(&mut adds, &post_codes);

    let mut balance = EvmContractBalance {
        pre_balance: String::from("00"),
        post_balance: String::from("00"),
    };
    let mut states = HashMap::<String, EvmContractState>::new();
    for addr in adds {
        let eth_addr = get_eth_addr(Some(addr));
        let pre_storage = get_storage(&addr, &pre_storages);
        let post_storage = get_storage(&addr, &post_storages);
        let pre_balance = get_balance(&addr, &pre_balances);
        let post_balance = get_balance(&addr, &post_balances);
        let pre_code = get_code(&addr, &pre_codes);
        let post_code = get_code(&addr, &post_codes);
        if addr.eq(&transaction.from) {
            balance = EvmContractBalance {
                pre_balance,
                post_balance,
            };
            continue;
        }
        states.insert(
            eth_addr.clone(),
            EvmContractState {
                pre_balance,
                post_balance,
                pre_storage,
                post_storage,
                pre_code,
                post_code,
            },
        );
    }

    let mut transactions: Vec<EvmContractTransaction> = Vec::new();
    for (block_num, block_hash) in number_to_hash {
        transactions.push(EvmContractTransaction {
            block_number: block_num,
            block_hash: h256_to_str(&block_hash),
        });
    }

    let context: EvmContractContext = EvmContractContext {
        tx_hash: String::from("0x") + &*h256_to_str(&tx_hash),
        chain_id: match transaction.chain_id {
            Some(v) => v.as_u64(),
            None => 0,
        },
        from: get_eth_addr(Some(transaction.from)),
        to: get_eth_addr(transaction.to),
        input: hex::encode(transaction.input.to_vec()),
        value: u256_to_str(&transaction.value),
        balance,
        gas_price: match transaction.gas_price {
            Some(v) => u256_to_str(&v),
            None => String::from("00"),
        },
        gas_limit: transaction.gas.as_u64(),
        gas_fee_cap: match transaction.max_fee_per_gas {
            Some(v) => u256_to_str(&v),
            None => String::from("00"),
        },
        gas_tip_cap: match transaction.max_priority_fee_per_gas {
            Some(v) => u256_to_str(&v),
            None => String::from("00"),
        },
        block_number: match transaction.block_number {
            Some(v) => v.as_u64(),
            None => 0,
        },
        timestamp: block.timestamp.as_usize(),
        nonce: transaction.nonce.as_u64(),
        block_hash: match transaction.block_hash {
            Some(v) => h256_to_str(&v),
            None => String::from("00"),
        },
        block_difficulty: block.difficulty.as_usize(),
        block_mix_hash: match block.mix_hash {
            Some(v) => h256_to_str(&v),
            None => String::from("00"),
        },
        status,
        return_result,
    };

    EvmContractInput {
        states,
        transactions,
        context,
    }
}

/// Populate accurate balance at specific transaction(**before** it being executed)
/// through Geth Debug PRC. Standard Ethereum JSON RPC only allow us get balance at "block".
// TODO trace whole block at a time to reduce RPC calls.
async fn populate_balance_at_block_number_and_index(
    address_to_balance: &mut BTreeMap<H160, U256>,
    block_number: U64,
    transaction_index: U64,
    geth_rpc_endpoint: &str,
) -> anyhow::Result<()> {
    let provider =
        Provider::<Http>::try_from(geth_rpc_endpoint).expect("could not instantiate HTTP Provider");

    let block = provider.get_block_with_txs(block_number).await?.unwrap();

    let prev_block_number = block_number - 1;
    for (address, value) in address_to_balance.iter_mut() {
        let balance = provider
            .get_balance(*address, Some(prev_block_number.into()))
            .await
            .unwrap();
        *value = balance;
    }

    for preceding_tx in &block.transactions {
        if preceding_tx.transaction_index.unwrap() == transaction_index {
            break;
        }

        let trace_options = GethDebugTracingOptions {
            disable_storage: Some(true),
            enable_memory: Some(false),
            disable_stack: Some(false),
            ..GethDebugTracingOptions::default()
        };
        let preceding_tx_trace = provider
            .debug_trace_transaction(preceding_tx.hash, trace_options)
            .await
            .unwrap();

        // gas fee
        if let Some(balance) = address_to_balance.get_mut(&preceding_tx.from) {
            // calculate gas fee
            let gas_used: U256 = preceding_tx_trace.gas.into();
            let gas_price = preceding_tx.gas_price.unwrap();
            let gas_fee = gas_used * gas_price;

            *balance -= gas_fee;
        }

        if preceding_tx_trace.failed {
            continue;
        }

        let tx_from = preceding_tx.from;
        let tx_to = match preceding_tx.to {
            Some(to) => to,
            None => get_contract_address(tx_from, preceding_tx.nonce),
        };

        let mut execution_context = vec![tx_to];
        let mut pre_balance_snapshots = vec![address_to_balance.clone()];

        if !preceding_tx.value.is_zero() {
            if let Some(v) = address_to_balance.get_mut(&tx_from) {
                *v -= preceding_tx.value;
            }

            if let Some(v) = address_to_balance.get_mut(&tx_to) {
                *v += preceding_tx.value;
            }
        }

        let mut depth = 1u64;
        let mut i = 0;
        while i < preceding_tx_trace.struct_logs.len() {
            let log = &preceding_tx_trace.struct_logs[i];

            if depth > log.depth {
                depth = log.depth;
                execution_context.truncate(depth.try_into().unwrap());
            }

            match log.op.as_str() {
                OP_CALL => {
                    let stack = log.stack.as_ref().unwrap();

                    let address = decode_address(stack[stack.len() - 2]);
                    let caller = execution_context.last().unwrap();

                    let value = stack[stack.len() - 3];
                    let next_log = preceding_tx_trace.struct_logs[i + 1].clone();
                    let failed = next_log.depth == log.depth;
                    if !value.is_zero() && !failed {
                        if let Some(balance) = address_to_balance.get_mut(caller) {
                            *balance -= value;
                        }
                        if let Some(balance) = address_to_balance.get_mut(&address) {
                            *balance += value;
                        }
                    }

                    execution_context.push(address);
                    pre_balance_snapshots.push(address_to_balance.clone());

                    depth += 1;
                }
                OP_STATICCALL => {
                    let stack = log.stack.as_ref().unwrap();

                    let address = decode_address(stack[stack.len() - 2]);

                    execution_context.push(address);
                    pre_balance_snapshots.push(address_to_balance.clone());

                    depth += 1;
                }
                OP_DELEGATECALL => {
                    execution_context.push(*execution_context.last().unwrap());
                    pre_balance_snapshots.push(address_to_balance.clone());

                    depth += 1;
                }
                OP_CALLCODE => {
                    execution_context.push(*execution_context.last().unwrap());
                    pre_balance_snapshots.push(address_to_balance.clone());

                    depth += 1;
                }
                OP_CREATE => {
                    let stack = log.stack.as_ref().unwrap();

                    let caller = *execution_context.last().unwrap();

                    let mut address = H160::zero();
                    for log in &preceding_tx_trace.struct_logs[i + 1..] {
                        if log.depth == depth {
                            let stack = log.stack.clone().unwrap();
                            address = decode_address(stack[stack.len() - 1]);
                            break;
                        }
                    }

                    let value = stack[stack.len() - 1];
                    let next_log = preceding_tx_trace.struct_logs[i + 1].clone();
                    let failed = next_log.depth == log.depth;
                    if !value.is_zero() && !failed {
                        if let Some(balance) = address_to_balance.get_mut(&caller) {
                            *balance -= value;
                        }
                        if let Some(balance) = address_to_balance.get_mut(&address) {
                            *balance += value;
                        }
                    }

                    execution_context.push(address);
                    pre_balance_snapshots.push(address_to_balance.clone());

                    depth += 1;
                }
                OP_CREATE2 => {
                    let stack = log.stack.as_ref().unwrap();

                    let caller = *execution_context.last().unwrap();

                    let mut address = H160::zero();
                    for log in &preceding_tx_trace.struct_logs[i + 1..] {
                        if log.depth == depth {
                            let stack = log.stack.as_ref().unwrap();
                            address = decode_address(stack[stack.len() - 1]);
                            break;
                        }
                    }

                    let value = stack[stack.len() - 1];
                    let next_log = preceding_tx_trace.struct_logs[i + 1].clone();
                    let failed = next_log.depth == log.depth;
                    if !value.is_zero() && !failed {
                        if let Some(balance) = address_to_balance.get_mut(&caller) {
                            *balance -= value;
                        }
                        if let Some(balance) = address_to_balance.get_mut(&address) {
                            *balance += value;
                        }
                    }

                    execution_context.push(address);
                    pre_balance_snapshots.push(address_to_balance.clone());

                    depth += 1;
                }
                OP_REVERT => {
                    *address_to_balance = pre_balance_snapshots.pop().unwrap();
                }
                OP_INVALID => {
                    *address_to_balance = pre_balance_snapshots.pop().unwrap();
                }
                _ => (),
            }

            if log.error.is_some() {
                *address_to_balance = pre_balance_snapshots.pop().unwrap();
            }

            i += 1;
        }
    }
    Ok(())
}
