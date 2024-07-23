#![allow(dead_code, unused_variables)]
use odra::{
    casper_types::{RuntimeArgs, Timestamp},
    host::HostEnv,
    schema::casper_contract_schema::{Entrypoint, NamedCLType},
    CallDef,
};
use odra_casper_rpc_client::casper_client::CasperClient;
use tokio::runtime::Runtime;

use crate::{args, CustomTypeSet, DeployedContractsContainer};

pub const DEFAULT_GAS: u64 = 15_000_000_000;
pub const ONE_CSPR: u64 = 1_000_000_000;

pub fn call(
    env: &HostEnv,
    container: &DeployedContractsContainer,
    entry_point: &Entrypoint,
    runtime_args: RuntimeArgs,
    contract_name: &str,
    types: &CustomTypeSet,
) {
    let contract_address = container
        .address(contract_name)
        .expect("Contract not found");
    let method = &entry_point.name;
    let ty = &entry_point.return_ty;

    // TODO: can't register a contract in the env
    let call_def = CallDef::new(method, true, runtime_args);
    // let call_def = CallDef::new(name, is_mut, runtime_args);

    let mut client = CasperClient::default();
    if entry_point.is_mutable {
        client.set_gas(DEFAULT_GAS);
    } else {
        client.set_gas(ONE_CSPR);
    }
    let rt = Runtime::new().unwrap();

    let timestamp = Timestamp::now();
    let result = if &ty.0 == &NamedCLType::Unit {
        rt.block_on(client.deploy_entrypoint_call(contract_address, call_def, timestamp))
    } else {
        rt.block_on(client.deploy_entrypoint_call_with_proxy(contract_address, call_def, timestamp))
    };

    let result = result.map(|bytes| args::decode(bytes.inner_bytes(), ty, types).0);
    match result {
        Ok(value) => {
            prettycli::info("Result");
            prettycli::info(&value);
        }
        Err(e) => prettycli::error(&format!("Error: {:?}", e)),
    }
}
