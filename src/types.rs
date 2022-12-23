use std::collections::HashMap;

use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{strict_bytes, Cbor};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvmContractInput {
    pub states: HashMap<String, EvmContractState>,
    pub transactions: Vec<EvmContractTransaction>,
    pub context: EvmContractContext,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvmContractState {
    pub pre_balance: String,
    pub post_balance: String,
    pub pre_storage: HashMap<String, String>,
    pub post_storage: HashMap<String, String>,
    pub pre_code: Option<String>,
    pub post_code: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvmContractBalance {
    pub pre_balance: String,
    pub post_balance: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvmContractTransaction {
    pub block_number: u64,
    pub block_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvmContractContext {
    pub tx_hash: String,
    pub chain_id: u64,
    pub from: String,
    pub to: String,
    pub input: String,
    pub value: String,
    pub balance: EvmContractBalance,
    pub gas_limit: u64,
    pub gas_price: String,
    pub gas_fee_cap: String,
    pub gas_tip_cap: String,
    pub block_number: u64,
    pub timestamp: usize,
    pub nonce: u64,
    pub block_hash: String,
    pub block_difficulty: usize,
    pub status: usize,
    #[serde(alias = "return")]
    pub return_result: String,
}

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct CreateParams {
    #[serde(with = "strict_bytes")]
    pub initcode: Vec<u8>,
    pub nonce: u64,
}

impl Cbor for CreateParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractParams(#[serde(with = "strict_bytes")] pub Vec<u8>);

impl Cbor for ContractParams {}
