use std::path::Path;

use fevm_test_vectors::types::EvmContractInput;
use fevm_test_vectors::util::{compute_address_create, is_create_contract, string_to_eth_address};
use fevm_test_vectors::{export_test_vector_file, init_log, load_evm_contract_input};
use fil_actor_eam::EthAddress;
use fil_actor_evm::DelegateCallParams;
use fvm_ipld_encoding::{from_slice, strict_bytes, BytesDe, Cbor, RawBytes};
use serde::{Deserialize, Serialize};
use serde_tuple::*;

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

    let input3: Vec<u8> = Vec::from([
        130, 216, 42, 88, 39, 0, 1, 85, 160, 228, 2, 32, 205, 243, 166, 109, 52, 179, 184, 192,
        184, 86, 183, 65, 79, 105, 47, 141, 10, 53, 137, 80, 84, 200, 202, 2, 75, 139, 56, 29, 83,
        18, 103, 66, 89, 2, 100, 9, 125, 161, 248, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        94, 240, 208, 157, 30, 98, 4, 20, 27, 77, 55, 83, 8, 8, 237, 25, 246, 15, 186, 53, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        164, 84, 206, 137, 97, 146, 211, 215, 244, 205, 211, 79, 207, 68, 61, 204, 54, 147, 122,
        142, 235, 236, 158, 62, 246, 209, 29, 152, 233, 152, 25, 138, 221, 42, 60, 102, 214, 185,
        105, 31, 126, 55, 175, 177, 222, 87, 219, 2, 254, 113, 2, 14, 137, 127, 4, 129, 198, 233,
        165, 50, 67, 233, 251, 153, 184, 229, 22, 6, 142, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 182, 132, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 239, 185, 81, 34, 160,
        93, 178, 5, 196, 126, 169, 140, 250, 194, 12, 199, 35, 89, 221, 205, 164, 90, 135, 109,
        242, 15, 18, 202, 211, 242, 109, 103, 223, 236, 178, 222, 171, 25, 77, 177, 88, 73, 222,
        55, 51, 72, 241, 182, 117, 245, 207, 218, 105, 115, 0, 100, 91, 231, 109, 231, 172, 130,
        12, 167, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 182, 146, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 34, 166, 225, 224, 23, 140, 48, 51, 18, 121, 251, 77, 148,
        17, 247, 217, 111, 25, 126, 163, 88, 40, 222, 82, 120, 68, 22, 32, 236, 52, 39, 222, 111,
        218, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 182, 132, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ]);
    let delegate_call_params = from_slice::<DelegateCallParams>(&input3).unwrap();
    println!(
        "{:?}, {:?}",
        delegate_call_params.code, delegate_call_params.input
    );
}

#[async_std::test]
async fn exec_export() {
    init_log();
    let input: EvmContractInput =
        serde_json::from_str(include_str!("contracts/contract.json")).unwrap();
    export_test_vector_file(input, Path::new("test_vector.json").to_path_buf())
        .await
        .unwrap();
}
