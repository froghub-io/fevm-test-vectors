use fil_actor_eam::EthAddress;
use fil_actors_runtime::test_utils::ACTOR_CODES;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::{strict_bytes, BytesDe, Cbor, RawBytes};
use serde::{Deserialize, Serialize};
use serde_tuple::*;
use std::path::Path;
use fevm_test_vectors::mock_single_actors::{ContractParams, CreateParams, print_actor_state, to_message};
use fevm_test_vectors::{compute_address_create, is_create_contract, string_to_eth_address, EvmContractInput};
use fevm_test_vectors::{export_test_vector_file, load_evm_contract_input};

#[test]
fn evm_create_test() {
    let from = string_to_eth_address("0x443c0c6F6Cb301B49eE5E9Be07B867378e73Fb54");
    let expected = string_to_eth_address("0xcc3d7ca4a302d196e70760e772ee26d38bd09dca");
    let result = compute_address_create(&EthAddress(from.0), 1);
    assert_eq!(result.0[..], expected.0[..]);
}

#[async_std::test]
async fn exec_export() {
    let input: EvmContractInput =
        serde_json::from_str(include_str!("contracts/contract2.json")).unwrap();
    export_test_vector_file(
        input,
        Path::new("/Users/grw/Desktop/constract2_test_vector.json").to_path_buf(),
    )
    .await
    .unwrap();
}