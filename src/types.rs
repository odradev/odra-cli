#![allow(dead_code, unused_variables)]
use std::{fmt::Debug, str::FromStr};

use odra::{
    casper_types::{
        bytesrepr::{FromBytes, ToBytes, OPTION_NONE_TAG, RESULT_ERR_TAG, RESULT_OK_TAG},
        AsymmetricType, CLType, Key, PublicKey, URef, U128, U256, U512,
    },
    schema::casper_contract_schema::NamedCLType,
    Address,
};

macro_rules! call_from_bytes {
    ($ty:ty, $value:ident) => {
        <$ty as FromBytes>::from_bytes($value)
            .map(|(v, rem)| (v.to_string(), rem))
            .unwrap()
    };
}

macro_rules! call_to_bytes {
    ($ty:ty, $value:ident) => {
        parse_value::<$ty>($value).to_bytes().unwrap()
    };
}

macro_rules! big_int_to_bytes {
    ($ty:ident, $value:ident) => {
        $ty::from_dec_str($value).unwrap().to_bytes().unwrap()
    };
}

fn parse_value<T: FromStr>(value: &str) -> T
where
    <T as FromStr>::Err: Debug,
{
    <T as FromStr>::from_str(value).unwrap()
}

pub(crate) fn named_cl_type_to_cl_type(ty: &NamedCLType) -> CLType {
    match ty {
        NamedCLType::Bool => CLType::Bool,
        NamedCLType::I32 => CLType::I32,
        NamedCLType::I64 => CLType::I64,
        NamedCLType::U8 => CLType::U8,
        NamedCLType::U32 => CLType::U32,
        NamedCLType::U64 => CLType::U64,
        NamedCLType::U128 => CLType::U128,
        NamedCLType::U256 => CLType::U256,
        NamedCLType::U512 => CLType::U512,
        NamedCLType::String => CLType::String,
        NamedCLType::Key => CLType::Key,
        NamedCLType::URef => CLType::URef,
        NamedCLType::PublicKey => CLType::PublicKey,
        NamedCLType::Option(ty) => CLType::Option(Box::new(named_cl_type_to_cl_type(ty))),
        NamedCLType::List(ty) => CLType::List(Box::new(named_cl_type_to_cl_type(ty))),
        NamedCLType::ByteArray(n) => CLType::ByteArray(*n),
        NamedCLType::Result { ok, err } => CLType::Result {
            ok: Box::new(named_cl_type_to_cl_type(ok)),
            err: Box::new(named_cl_type_to_cl_type(err)),
        },
        NamedCLType::Map { key, value } => CLType::Map {
            key: Box::new(named_cl_type_to_cl_type(key)),
            value: Box::new(named_cl_type_to_cl_type(value)),
        },
        NamedCLType::Tuple1(ty) => CLType::Tuple1([Box::new(named_cl_type_to_cl_type(&ty[0]))]),
        NamedCLType::Tuple2(ty) => CLType::Tuple2([
            Box::new(named_cl_type_to_cl_type(&ty[0])),
            Box::new(named_cl_type_to_cl_type(&ty[1])),
        ]),
        NamedCLType::Tuple3(ty) => CLType::Tuple3([
            Box::new(named_cl_type_to_cl_type(&ty[0])),
            Box::new(named_cl_type_to_cl_type(&ty[1])),
            Box::new(named_cl_type_to_cl_type(&ty[2])),
        ]),
        NamedCLType::Custom(_) => CLType::Any,
        NamedCLType::Unit => CLType::Unit,
    }
}

pub(crate) fn vec_into_bytes(ty: &NamedCLType, input: Vec<&str>) -> Vec<u8> {
    let mut result = vec![];

    result.append(&mut (input.len() as u32).to_bytes().unwrap());

    for value in input {
        result.extend(into_bytes(ty, &value));
    }
    result
}

pub(crate) fn into_bytes(ty: &NamedCLType, input: &str) -> Vec<u8> {
    match ty {
        NamedCLType::Bool => call_to_bytes!(bool, input),
        NamedCLType::I32 => call_to_bytes!(i32, input),
        NamedCLType::I64 => call_to_bytes!(i64, input),
        NamedCLType::U8 => call_to_bytes!(u8, input),
        NamedCLType::U32 => call_to_bytes!(u32, input),
        NamedCLType::U64 => call_to_bytes!(u64, input),
        NamedCLType::U128 => big_int_to_bytes!(U128, input),
        NamedCLType::U256 => big_int_to_bytes!(U256, input),
        NamedCLType::U512 => big_int_to_bytes!(U512, input),
        NamedCLType::String => call_to_bytes!(String, input),
        NamedCLType::Key => call_to_bytes!(Address, input),
        NamedCLType::URef => URef::from_formatted_str(input).unwrap().to_bytes().unwrap(),
        NamedCLType::PublicKey => PublicKey::from_hex(input).unwrap().to_bytes().unwrap(),
        NamedCLType::Option(ty) => {
            if input.is_empty() {
                vec![OPTION_NONE_TAG]
            } else {
                let mut result = vec![OPTION_NONE_TAG];
                result.extend(into_bytes(ty, input));
                result
            }
        }
        NamedCLType::Result { ok, err } => {
            let mut result = vec![];
            if input.starts_with("err:") {
                let value = input.strip_prefix("err:").unwrap();
                result.push(RESULT_ERR_TAG);
                result.extend(into_bytes(err, &value));
            } else if input.starts_with("ok:") {
                let value = input.strip_prefix("ok:").unwrap();
                result.push(RESULT_OK_TAG);
                result.extend(into_bytes(ok, &value));
            } else {
                panic!("Invalid variant");
            }
            result
        }
        NamedCLType::Tuple1(ty) => into_bytes(&ty[0], input),
        NamedCLType::Tuple2(ty) => {
            let parts = input.split(',').collect::<Vec<_>>();
            let mut result = vec![];
            result.extend(into_bytes(&ty[0], parts[0]));
            result.extend(into_bytes(&ty[1], parts[1]));
            result
        }
        NamedCLType::Tuple3(ty) => {
            let parts = input.split(',').collect::<Vec<_>>();
            let mut result = vec![];
            result.extend(into_bytes(&ty[0], parts[0]));
            result.extend(into_bytes(&ty[1], parts[1]));
            result.extend(into_bytes(&ty[2], parts[2]));
            result
        }
        NamedCLType::Unit => vec![],
        NamedCLType::Map { key, value } => {
            todo!();
        }
        NamedCLType::List(ty) => {
            todo!();
        }
        NamedCLType::ByteArray(_) => {
            todo!();
        }
        NamedCLType::Custom(_) => unreachable!("should not be here"),
    }
}

pub(crate) fn from_bytes<'a>(ty: &NamedCLType, input: &'a [u8]) -> (String, &'a [u8]) {
    match ty {
        NamedCLType::Bool => call_from_bytes!(bool, input),
        NamedCLType::I32 => call_from_bytes!(i32, input),
        NamedCLType::I64 => call_from_bytes!(i64, input),
        NamedCLType::U8 => call_from_bytes!(u8, input),
        NamedCLType::U32 => call_from_bytes!(u32, input),
        NamedCLType::U64 => call_from_bytes!(u64, input),
        NamedCLType::U128 => call_from_bytes!(U128, input),
        NamedCLType::U256 => call_from_bytes!(U256, input),
        NamedCLType::U512 => call_from_bytes!(U512, input),
        NamedCLType::String => call_from_bytes!(String, input),
        NamedCLType::Key => call_from_bytes!(Key, input),
        NamedCLType::URef => call_from_bytes!(URef, input),
        NamedCLType::PublicKey => call_from_bytes!(PublicKey, input),
        NamedCLType::Option(ty) => {
            if input.get(0) == Some(&OPTION_NONE_TAG) {
                return ("null".to_string(), input);
            } else {
                from_bytes(&*ty, &input[1..])
            }
        }
        NamedCLType::Result { ok, err } => {
            let (variant, rem) = u8::from_bytes(input).unwrap();
            match variant {
                RESULT_ERR_TAG => {
                    let (value, rem) = from_bytes(err, rem);
                    (format!("Err({})", value), rem)
                }
                RESULT_OK_TAG => {
                    let (value, rem) = from_bytes(ok, rem);
                    (format!("Ok({})", value), rem)
                }
                _ => panic!("Invalid variant"),
            }
        }
        NamedCLType::Tuple1(ty) => {
            let v = from_bytes(&ty[0], input);
            (format!("({},)", v.0), v.1)
        }
        NamedCLType::Tuple2(ty) => {
            let (v1, rem) = from_bytes(&ty[0], input);
            let (v2, rem) = from_bytes(&ty[1], rem);
            (format!("({}, {})", v1, v2), rem)
        }
        NamedCLType::Tuple3(ty) => {
            let (v1, rem) = from_bytes(&ty[0], input);
            let (v2, rem) = from_bytes(&ty[1], rem);
            let (v3, rem) = from_bytes(&ty[2], rem);
            (format!("({}, {}, {})", v1, v2, v3), rem)
        }
        NamedCLType::Unit => <() as FromBytes>::from_bytes(input)
            .map(|(v, rem)| ("".to_string(), rem))
            .unwrap(),
        NamedCLType::Custom(_) => unreachable!("should not be here"),
        NamedCLType::List(ty) => {
            todo!();
        }
        NamedCLType::ByteArray(_) => {
            todo!();
        }
        NamedCLType::Map { key, value } => {
            todo!();
        }
    }
}
