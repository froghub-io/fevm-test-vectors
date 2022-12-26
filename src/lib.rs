use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Once};

use async_std::channel::bounded;
use async_std::sync::RwLock;
use bytes::Buf;
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fil_actor_eam::EthAddress;
use fil_actor_evm::interpreter::system::StateKamt;
use fil_actor_evm::interpreter::U256;
use fil_actors_runtime::runtime::builtins::Type;
use fil_actors_runtime::runtime::EMPTY_ARR_CID;
use fil_actors_runtime::{
    ActorError, AsActorError, BURNT_FUNDS_ACTOR_ID, EAM_ACTOR_ID, REWARD_ACTOR_ID,
};
use flate2::bufread::GzEncoder;
use flate2::Compression;
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_car::CarHeader;
use fvm_ipld_encoding::{BytesDe, BytesSer, Cbor, CborStore, RawBytes, DAG_CBOR};
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::{Address, Payload};
use fvm_shared::bigint::{BigInt, Integer};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::randomness::RANDOMNESS_LENGTH;
use fvm_shared::receipt::Receipt;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{MethodNum, HAMT_BIT_WIDTH, IDENTITY_HASH, METHOD_SEND};
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use util::get_code_cid_map;
use vector::{ApplyMessage, PreConditions, StateTreeVector, TestVector, Variant};

use crate::evm_state::State as EvmState;
use crate::mock_single_actors::{address_to_eth, Actor, Mock, KAMT_CONFIG};
use crate::tracing_blockstore::TracingBlockStore;
use crate::types::{
    ContractParams, CreateParams, EvmContractBalance, EvmContractContext, EvmContractInput,
};
use crate::util::{
    compute_address_create, is_create_contract, string_to_big_int, string_to_bytes,
    string_to_eth_address, string_to_u256,
};
use crate::vector::{RandomnessMatch, RandomnessRule, TipsetCid};

mod cidjson;
pub mod evm_state;
pub mod extractor;
pub mod mock_single_actors;
pub mod tracing_blockstore;
pub mod types;
pub mod util;
mod vector;

const LOG_INIT: Once = Once::new();

#[inline(always)]
pub fn init_log() {
    LOG_INIT.call_once(|| {
        fil_logger::init();
    });
}

pub async fn export_test_vector_file(input: EvmContractInput, path: PathBuf) -> anyhow::Result<()> {
    let actor_codes = get_code_cid_map()?;
    let store = TracingBlockStore::new(MemoryBlockstore::new());

    let (pre_state_root, post_state_root, contract_addrs) =
        load_evm_contract_input(&store, actor_codes, &input)?;
    let pre_state_root = store.put_cbor(&(5, pre_state_root, EMPTY_ARR_CID), Code::Blake2b256)?;
    let post_state_root = store.put_cbor(&(5, post_state_root, EMPTY_ARR_CID), Code::Blake2b256)?;

    //car_bytes
    let car_header = CarHeader::new(vec![pre_state_root, post_state_root], 1);
    let (tx, mut rx) = bounded(100);
    let buffer: Arc<RwLock<Vec<u8>>> = Default::default();
    let buffer_cloned = buffer.clone();
    let write_task = async_std::task::spawn(async move {
        car_header
            .write_stream_async(&mut *buffer_cloned.write().await, &mut rx)
            .await
            .unwrap()
    });
    for cid in (&store).traced.borrow().iter() {
        tx.send((cid.clone(), store.base.get(cid).unwrap().unwrap()))
            .await
            .unwrap();
    }
    drop(tx);
    write_task.await;
    let car_bytes = buffer.read().await.clone();

    //gzip car_bytes
    let mut gz_car_bytes: Vec<u8> = Default::default();
    let mut gz_encoder = GzEncoder::new(car_bytes.reader(), Compression::new(9));
    gz_encoder.read_to_end(&mut gz_car_bytes).unwrap();

    //message
    let message = to_message(&input.context);

    //receipt
    let receipt = Receipt {
        exit_code: ExitCode::OK,
        return_data: RawBytes::serialize(BytesDe(hex::decode(&input.context.return_result)?))?,
        gas_used: 0,
        events_root: None,
    };
    log::info!("receipt: {:?}", receipt);

    // tipset_cids
    let mut tipset_cids = Vec::new();
    for t in input.transactions {
        tipset_cids.push(TipsetCid {
            epoch: t.block_number as ChainEpoch,
            cid: Cid::new_v1(
                DAG_CBOR,
                multihash::Multihash::wrap(IDENTITY_HASH, &hex::decode(t.block_hash).unwrap())
                    .unwrap(),
            ),
        });
    }

    const ENTROPY: &[u8] = b"prevrandao";
    let block_difficulty_bytes = input.context.block_difficulty.to_be_bytes();
    let mut ret = vec![0u8; 32];
    ret[32 - block_difficulty_bytes.len()..32].copy_from_slice(&block_difficulty_bytes);
    let randomness = vec![RandomnessMatch {
        on: RandomnessRule {
            kind: vector::RandomnessKind::Beacon,
            dst: 10, //fil_actors_runtime::runtime::randomness::DomainSeparationTag::EvmPrevRandao as i64,
            epoch: input.context.block_number as ChainEpoch,
            entropy: Vec::from(ENTROPY),
        },
        ret,
    }];
    let variants = vec![Variant {
        id: String::from("test_evm"),
        epoch: input.context.block_number as ChainEpoch,
        timestamp: Some(1671507767),
        nv: NetworkVersion::V18 as u32,
    }];
    let test_vector = TestVector {
        class: String::from_str("message")?,
        chain_id: Some(1),
        selector: None,
        meta: None,
        car: gz_car_bytes,
        preconditions: PreConditions {
            state_tree: StateTreeVector {
                root_cid: pre_state_root,
            },
            basefee: None,
            circ_supply: None,
            variants,
        },
        apply_messages: vec![ApplyMessage {
            bytes: message.marshal_cbor()?,
            epoch_offset: None,
        }],
        postconditions: vector::PostConditions {
            state_tree: StateTreeVector {
                root_cid: post_state_root,
            },
            receipts: vec![receipt],
        },
        skip_compare_gas_used: true,
        skip_compare_addresses: Some(vec![message.from]),
        skip_compare_actor_ids: Some(vec![REWARD_ACTOR_ID, BURNT_FUNDS_ACTOR_ID]),
        additional_compare_addresses: Some(contract_addrs),
        tipset_cids: Some(tipset_cids),
        randomness,
    };

    let output = File::create(&path)?;
    serde_json::to_writer_pretty(output, &test_vector)?;
    Ok(())
}

pub fn get_eth_addr_balance(
    eth_addr: &String,
    balances: &HashMap<String, EvmContractBalance>,
    pre: bool,
) -> TokenAmount {
    match balances.get(eth_addr) {
        Some(v) => {
            if pre {
                TokenAmount::from_atto(string_to_big_int(&v.pre_balance))
            } else {
                TokenAmount::from_atto(string_to_big_int(&v.post_balance))
            }
        }
        None => TokenAmount::from_atto(0),
    }
}

pub fn load_evm_contract_input<BS>(
    store: &BS,
    actor_codes: BTreeMap<Type, Cid>,
    input: &EvmContractInput,
) -> anyhow::Result<(Cid, Cid, Vec<Address>)>
where
    BS: Blockstore,
{
    let mut contract_addrs = Vec::new();

    let mut mock = Mock::new(store, actor_codes);
    mock.mock_builtin_actor();

    let from = Address::new_delegated(EAM_ACTOR_ID, &string_to_eth_address(&input.context.from).0)
        .unwrap();
    mock.mock_embryo_address_actor(
        from,
        TokenAmount::from_atto(string_to_big_int(&input.context.balance.pre_balance)),
        input.context.nonce,
    );

    // preconditions
    let create_contract_eth_addr = if is_create_contract(&input.context.to) {
        Some(compute_address_create(
            &string_to_eth_address(&input.context.from),
            input.context.nonce,
        ))
    } else {
        None
    };
    for (eth_addr_str, state) in &input.states {
        let eth_addr = string_to_eth_address(&eth_addr_str);
        let to = Address::new_delegated(EAM_ACTOR_ID, &eth_addr.0).unwrap();
        let balance = TokenAmount::from_atto(string_to_big_int(&state.pre_balance));

        contract_addrs.push(to.clone());

        if let Some(create_contract_eth_addr) = create_contract_eth_addr {
            if eth_addr.eq(&create_contract_eth_addr) {
                continue;
            }
        }
        mock.mock_evm_actor(to, balance);
        let mut storage = HashMap::<U256, U256>::new();
        for (k, v) in &state.pre_storage {
            let key = string_to_u256(&k);
            let value = string_to_u256(&v);
            storage.insert(key, value);
        }
        let bytecode = match &state.pre_code {
            Some(bytecode) => Some(string_to_bytes(bytecode)),
            None => None,
        };
        mock.mock_evm_actor_state(&to, storage, bytecode)?;
    }
    let pre_state_root = mock.get_state_root();
    log::info!("pre_state_root: {:?}", pre_state_root);
    mock.print_actor_state(pre_state_root)?;

    // postconditions
    mock.mock_actor_balance(
        &from,
        TokenAmount::from_atto(string_to_big_int(&input.context.balance.post_balance)),
    )?;
    for (eth_addr, state) in &input.states {
        let eth_addr = string_to_eth_address(&eth_addr);
        let to = Address::new_delegated(EAM_ACTOR_ID, &eth_addr.0).unwrap();
        let balance = TokenAmount::from_atto(string_to_big_int(&state.post_balance));
        if let Some(create_contract_eth_addr) = create_contract_eth_addr {
            if eth_addr.eq(&create_contract_eth_addr) {
                mock.mock_evm_actor(to, balance.clone());
            }
        }
        let mut storage = HashMap::<U256, U256>::new();
        for (k, v) in &state.post_storage {
            let key = string_to_u256(&k);
            let value = string_to_u256(&v);
            storage.insert(key, value);
        }
        let bytecode = match &state.post_code {
            Some(bytecode) => Some(string_to_bytes(bytecode)),
            None => None,
        };
        mock.mock_evm_actor_state(&to, storage, bytecode)?;
        mock.mock_actor_balance(&to, balance)?;
    }
    let post_state_root = mock.get_state_root();
    log::info!("post_state_root: {:?}", post_state_root);
    mock.print_actor_state(post_state_root)?;

    return Ok((pre_state_root, post_state_root, contract_addrs));
}

pub fn to_message(context: &EvmContractContext) -> Message {
    let from =
        Address::new_delegated(EAM_ACTOR_ID, &string_to_eth_address(&context.from).0).unwrap();
    let to: Address;
    let method_num: MethodNum;
    let mut params = RawBytes::from(vec![0u8; 0]);
    if is_create_contract(&context.to) {
        to = Address::new_id(10);
        method_num = fil_actor_eam::Method::Create as u64;
        let params2 = CreateParams {
            initcode: string_to_bytes(&context.input),
            nonce: context.nonce,
        };
        params = RawBytes::serialize(params2).unwrap();
    } else {
        to = Address::new_delegated(EAM_ACTOR_ID, &string_to_eth_address(&context.to).0).unwrap();
        if context.input.len() > 0 {
            params = RawBytes::serialize(ContractParams(string_to_bytes(&context.input))).unwrap();
            method_num = fil_actor_evm::Method::InvokeContract as u64
        } else {
            method_num = METHOD_SEND;
        }
    }
    Message {
        version: 0,
        from,
        to,
        sequence: context.nonce,
        value: TokenAmount::from_atto(string_to_big_int(&context.value)),
        method_num,
        params,
        gas_limit: context.gas_limit as i64,
        gas_fee_cap: TokenAmount::from_atto(string_to_big_int(&context.gas_fee_cap)),
        gas_premium: TokenAmount::from_atto(string_to_big_int(&context.gas_tip_cap)),
    }
}

pub fn get_actor_state<BS: Blockstore>(
    state_root: Cid,
    store: &BS,
) -> anyhow::Result<HashMap<String, HashMap<U256, U256>>> {
    let mut states = HashMap::new();
    let actors = Hamt::<&BS, Actor>::load_with_bit_width(&state_root, store, HAMT_BIT_WIDTH)?;
    actors.for_each(|_, v| {
        let state_root = v.head;
        let store = store.clone();
        match store.get_cbor::<EvmState>(&state_root) {
            Ok(res) => match res {
                Some(state) => {
                    if v.predictable_address.is_some() {
                        let receiver_eth_addr = address_to_eth(&v.predictable_address.unwrap())?;
                        let mut storage = HashMap::new();
                        let slots = StateKamt::load_with_config(
                            &state.contract_state,
                            store,
                            KAMT_CONFIG.clone(),
                        )
                        .context_code(ExitCode::USR_ILLEGAL_STATE, "state not in blockstore")?;
                        slots.for_each(|k, v| {
                            storage.insert(k.clone(), v.clone());
                            Ok(())
                        })?;
                        states.insert(hex::encode(receiver_eth_addr.0), storage);
                    }
                }
                None => {}
            },
            Err(_) => {}
        }
        Ok(())
    })?;
    Ok(states)
}
