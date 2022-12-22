use fevm_test_vectors::mock_single_actors::{
    print_actor_state, to_message, ContractParams, CreateParams,
};
use fevm_test_vectors::util::{compute_address_create, is_create_contract, string_to_eth_address};
use fevm_test_vectors::{export_test_vector_file, load_evm_contract_input, EvmContractInput};
use fil_actor_eam::EthAddress;
use fvm_ipld_encoding::{from_slice, strict_bytes, BytesDe, Cbor, RawBytes};
use serde::{Deserialize, Serialize};
use serde_tuple::*;
use std::path::Path;

#[test]
fn evm_create_test() {
    let from = string_to_eth_address("0x443c0c6F6Cb301B49eE5E9Be07B867378e73Fb54");
    let expected = string_to_eth_address("0xcc3d7ca4a302d196e70760e772ee26d38bd09dca");
    let result = compute_address_create(&EthAddress(from.0), 1);
    assert_eq!(result.0[..], expected.0[..]);
}

#[test]
fn from_slice_test() {
    let input: Vec<u8> = Vec::from([
        88, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 10,
    ]);
    let bytebuf = from_slice::<BytesDe>(&input);
    println!("{:?}", bytebuf.unwrap().into_vec());

    let input2: Vec<u8> = Vec::from([
        88, 36, 96, 87, 54, 29, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 45,
    ]);
    let bytebuf2 = from_slice::<BytesDe>(&input2);
    println!("{:?}", bytebuf2.unwrap().into_vec());
    // println!("{:?}", hex::encode(bytebuf2.unwrap().into_vec()));
}

#[async_std::test]
async fn exec_export() {
    let input: EvmContractInput =
        serde_json::from_str(include_str!("contracts/contract3.json")).unwrap();
    export_test_vector_file(
        input,
        Path::new("/Users/zhenghe/Downloads/constract3_test_vector.json").to_path_buf(),
    )
    .await
    .unwrap();
}
