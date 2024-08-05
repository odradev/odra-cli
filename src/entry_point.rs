use anyhow::Result;
use clap::ArgMatches;
use odra::{
    casper_types::U512,
    host::HostEnv,
    schema::casper_contract_schema::{Entrypoint, NamedCLType},
    CallDef,
};

use crate::{args, CustomTypeSet, DeployedContractsContainer};

pub const DEFAULT_GAS: u64 = 20_000_000_000;

pub fn call(
    env: &HostEnv,
    contract_name: &str,
    entry_point: &Entrypoint,
    args: &ArgMatches,
    types: &CustomTypeSet,
) -> Result<String> {
    let container = DeployedContractsContainer::load().expect("No deployed contracts found");
    let amount = args
        .try_get_one::<String>("__attached_value")
        .ok()
        .flatten()
        .map(|s| U512::from_dec_str(s).unwrap())
        .unwrap_or(U512::zero());

    let runtime_args = args::compose(&entry_point, args, types);
    let contract_address = container
        .address(contract_name)
        .expect("Contract not found");

    let method = &entry_point.name;
    let is_mut = entry_point.is_mutable;
    let ty = &entry_point.return_ty;
    let call_def = CallDef::new(method, is_mut, runtime_args).with_amount(amount);
    let use_proxy = ty.0 != NamedCLType::Unit || !call_def.amount().is_zero();

    if is_mut {
        env.set_gas(DEFAULT_GAS);
    }
    env.raw_call_contract(contract_address, call_def, use_proxy)
        .map(|bytes| args::decode(bytes.inner_bytes(), ty, types).0)
        .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    // match result {
    //     Ok(value) => {
    //         prettycli::info("Result");
    //         prettycli::info(&value);
    //     }
    //     Err(e) => prettycli::error(&format!("Error: {:?}", e)),
    // }
}
