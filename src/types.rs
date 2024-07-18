#![allow(dead_code, unused_variables)]
use std::{collections::HashMap, fmt::Debug, str::FromStr};

use clap::ArgMatches;
use odra::{
    casper_types::{
        bytesrepr::ToBytes, CLTyped, RuntimeArgs, URef, U128, U256, U512,
    },
    schema::casper_contract_schema::{Argument, Entrypoint, NamedCLType},
    Address,
};

use crate::Parser;

pub fn build_args(
    entry_point: &Entrypoint,
    args: &ArgMatches,
    parsers: &HashMap<String, Box<dyn Parser>>,
) -> RuntimeArgs {
    let mut runtime_args = RuntimeArgs::new();

    entry_point
        .arguments
        .iter()
        .enumerate()
        .for_each(|(idx, arg)| {
            let input = args.get_one::<String>(&arg.name).unwrap();
            match &arg.ty.0 {
                NamedCLType::Bool => {
                    insert_arg::<bool>(&mut runtime_args, arg, input);
                }
                NamedCLType::I32 => {
                    insert_arg::<i32>(&mut runtime_args, arg, input);
                }
                NamedCLType::I64 => {
                    insert_arg::<i64>(&mut runtime_args, arg, input);
                }
                NamedCLType::U8 => {
                    insert_arg::<u8>(&mut runtime_args, arg, input);
                }
                NamedCLType::U32 => {
                    insert_arg::<u32>(&mut runtime_args, arg, input);
                }
                NamedCLType::U64 => {
                    insert_arg::<u64>(&mut runtime_args, arg, input);
                }
                NamedCLType::U128 => {
                    insert_arg::<U128>(&mut runtime_args, arg, input);
                }
                NamedCLType::U256 => {
                    insert_arg::<U256>(&mut runtime_args, arg, input);
                }
                NamedCLType::U512 => {
                    insert_arg::<U512>(&mut runtime_args, arg, input);
                }
                NamedCLType::String => {
                    insert_arg::<String>(&mut runtime_args, arg, input);
                }
                NamedCLType::Key => {
                    insert_arg::<Address>(&mut runtime_args, arg, input);
                }
                NamedCLType::URef => {
                    let value: URef = URef::from_formatted_str(input).unwrap();
                    runtime_args.insert(arg.name.clone(), value).unwrap();
                }
                // NamedCLType::Option(ty) => {
                //     insert_arg::<String>(&mut runtime_args, arg, input);
                // }
                NamedCLType::List(ty) => {
                    let a = input.split(",");
                    insert_arg::<String>(&mut runtime_args, arg, input);
                }
                // NamedCLType::ByteArray(_) => {
                //     insert_arg::<String>(&mut runtime_args, arg, input);
                // }
                // NamedCLType::Result { ok, err } => todo!(),
                // NamedCLType::Map { key, value } => todo!(),
                // NamedCLType::Tuple1(_) => todo!(),
                // NamedCLType::Tuple2(_) => todo!(),
                // NamedCLType::Tuple3(_) => todo!(),
                NamedCLType::Custom(s) => {
                    let parser = parsers.get(s).unwrap();
                    let value = parser.parse(input);
                    runtime_args.insert_cl_value(arg.name.clone(), value);
                }
                _ => {
                    panic!("Type not supported {:?}", &arg.ty.0);
                }
            }
        });
    runtime_args
}

fn parse_value<T: FromStr>(value: &str) -> T
where
    <T as FromStr>::Err: Debug,
{
    <T as FromStr>::from_str(value).unwrap()
}

fn insert_arg<T: FromStr + CLTyped + ToBytes>(
    args: &mut RuntimeArgs,
    argument: &Argument,
    value: &str,
) where
    <T as FromStr>::Err: Debug,
{
    let value = parse_value::<T>(value);
    args.insert(argument.name.clone(), value).unwrap();
}
