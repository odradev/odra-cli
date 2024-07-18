#![allow(dead_code, unused_variables)]
use std::{collections::HashMap, fmt::Debug};

use odra::{
    casper_types::{
        bytesrepr::FromBytes, CLTyped, Key, PublicKey, RuntimeArgs, URef, U128, U256, U512,
    }, host::HostEnv, schema::casper_contract_schema::{Entrypoint, NamedCLType}, Address, CallDef
};

use crate::{DeployedContractsContainer, Parser};

pub const DEFAULT_GAS: u64 = 10_000_000_000;
pub const ONE_CSPR: u64 = 1_000_000_000;

pub fn call(
    env: &HostEnv,
    container: &DeployedContractsContainer,
    entry_point: &Entrypoint,
    runtime_args: RuntimeArgs,
    contract_name: &str,
    _parsers: &HashMap<String, Box<dyn Parser>>,
) {
    let is_mut = entry_point.is_mutable;
    let name = &entry_point.name;

    let contract_address = container
        .address(contract_name)
        .expect("Contract not found");
    // TODO: can't register a contract in the env
    let call_def = CallDef::new(name, true, runtime_args);
    // let call_def = CallDef::new(name, is_mut, runtime_args);

    if is_mut {
        env.set_gas(DEFAULT_GAS);
    } else {
        env.set_gas(ONE_CSPR);
    }

    match &entry_point.return_ty.0 {
        NamedCLType::Bool => {
            call_and_print::<bool>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::I32 => {
            call_and_print::<i32>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::I64 => {
            call_and_print::<i64>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::U8 => {
            call_and_print::<u8>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::U32 => {
            call_and_print::<u32>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::U64 => {
            call_and_print::<u64>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::U128 => {
            call_and_print::<U128>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::U256 => {
            call_and_print::<U256>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::U512 => {
            call_and_print::<U512>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::Unit => {
            call_and_print::<()>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::String => {
            call_and_print::<String>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::Key => {
            call_and_print::<Key>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::URef => {
            call_and_print::<URef>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::PublicKey => {
            call_and_print::<PublicKey>(&env, contract_address, call_def, contract_name, name);
        }
        NamedCLType::Option(ty) => {
            todo!();
            // self.call_contract::<Option<()>>(&env, contract_address, call_def);
        }
        NamedCLType::List(_) => todo!(),
        NamedCLType::ByteArray(_) => todo!(),
        NamedCLType::Result { ok, err } => todo!(),
        NamedCLType::Map { key, value } => todo!(),
        NamedCLType::Tuple1(_) => todo!(),
        NamedCLType::Tuple2(_) => todo!(),
        NamedCLType::Tuple3(_) => todo!(),
        NamedCLType::Custom(ty) => panic!("Custom type not supported: {}", ty),
    };
}

fn call_and_print<T: CLTyped + FromBytes + Debug>(
    env: &HostEnv,
    contract_address: Address,
    call_def: CallDef,
    contract_name: &str,
    entry_point: &str,
) {
    let result = env.call_contract::<T>(contract_address, call_def);
    let msg = format!(
        "{}.{} = {:?}",
        contract_name, entry_point, result
    );
    if result.is_err() {
        prettycli::error(&msg);
    } else {
        prettycli::info(&msg);
    }
}
