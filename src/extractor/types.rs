use std::collections::BTreeMap;

use ethers::types::{Bytes, H160, H256, U256, U64};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthAccountState {
    pub nonce: u64,
    pub balance: U256,
    pub code: Bytes,
    pub storage: BTreeMap<H256, H256>,
}

pub type EthState = BTreeMap<H160, EthAccountState>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthTransactionTestVector {
    // transaction fields
    pub hash: H256,
    pub nonce: u64,
    pub from: H160,
    pub to: H160,
    pub value: U256,
    pub input: Bytes,
    pub gas: U256,                              // transaction gas limit,
    pub gas_price: U256, // for type 2 transaction, it's the effective gas price
    pub max_priority_fee_per_gas: Option<U256>, // type 2 transaction field
    pub max_fee_per_gas: Option<U256>, // type 2 transaction field
    // transaction receipt fields
    pub status: u64, // Status: either 1 (success) or 0 (failure).
    pub gas_used: U256,
    pub return_value: Bytes,
    // call context
    pub coinbase: H160,
    // pub gas_limit: u64, // block gas limit
    pub base_fee_per_gas: Option<U256>,
    pub difficultly: U256,
    pub chain_id: U256,
    pub block_number: u64,
    pub block_hashes: BTreeMap<u64, H256>,
    pub timestamp: U256,
    // pre-state and post-state
    pub prestate: EthState,
    pub poststate: EthState,
}
