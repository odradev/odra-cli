use std::str::FromStr;

use clap::{Arg, ArgAction, ArgMatches};
use odra::{
    casper_types::{
        bytesrepr::{FromBytes, ToBytes},
        CLType, CLValue, RuntimeArgs,
    },
    schema::casper_contract_schema::{Argument, CustomType, Entrypoint, NamedCLType, Type},
};
use serde_json::Value;

use crate::{types, CustomTypeSet};

/// A typed command argument.
#[derive(Debug, PartialEq)]
pub struct CommandArg {
    pub name: String,
    pub required: bool,
    pub description: String,
    pub ty: NamedCLType,
    pub is_list_element: bool,
}

impl CommandArg {
    pub fn new(
        name: &str,
        description: &str,
        ty: NamedCLType,
        required: bool,
        is_list_element: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            required,
            description: description.to_string(),
            ty,
            is_list_element,
        }
    }
}

pub fn entry_point_args(entry_point: &Entrypoint, types: &CustomTypeSet) -> Vec<Arg> {
    entry_point
        .arguments
        .iter()
        .flat_map(|arg| flat_arg(arg, types, false))
        .map(Into::into)
        .collect()
}

impl From<CommandArg> for Arg {
    fn from(arg: CommandArg) -> Self {
        let result = Arg::new(&arg.name)
            .long(arg.name)
            .value_name(format!("{:?}", arg.ty))
            .required(arg.required)
            .help(arg.description);

        match arg.is_list_element {
            true => result.action(ArgAction::Append),
            false => result.action(ArgAction::Set),
        }
    }
}

fn flat_arg(arg: &Argument, types: &CustomTypeSet, is_list_element: bool) -> Vec<CommandArg> {
    match &arg.ty.0 {
        NamedCLType::Custom(name) => {
            let matching_type = types
                .iter()
                .find(|ty| {
                    let type_name = match ty {
                        CustomType::Struct { name, .. } => &name.0,
                        CustomType::Enum { name, .. } => &name.0,
                    };
                    name == type_name
                })
                .expect("Type not found");

            match matching_type {
                CustomType::Struct { members, .. } => members
                    .iter()
                    .flat_map(|field| {
                        let field_arg = Argument {
                            name: format!("{}.{}", arg.name, field.name),
                            ty: field.ty.clone(),
                            optional: arg.optional,
                            description: field.description.clone(),
                        };
                        flat_arg(&field_arg, types, is_list_element)
                    })
                    .collect(),
                CustomType::Enum { variants, .. } => variants
                    .iter()
                    .flat_map(|variant| {
                        let variant_arg = Argument {
                            name: format!("{}.{}", arg.name, variant.name.to_lowercase()),
                            ty: variant.ty.clone(),
                            optional: arg.optional,
                            description: variant.description.clone(),
                        };
                        flat_arg(&variant_arg, types, is_list_element)
                    })
                    .collect(),
            }
        }
        NamedCLType::List(inner) => {
            let arg = Argument {
                ty: Type(*inner.clone()),
                ..arg.clone()
            };
            flat_arg(&arg, types, true)
        }
        _ => {
            vec![CommandArg::new(
                &arg.name,
                &arg.description.clone().unwrap_or_default(),
                arg.ty.0.clone(),
                !arg.optional,
                is_list_element,
            )]
        }
    }
}

pub fn compose(entry_point: &Entrypoint, args: &ArgMatches, types: &CustomTypeSet) -> RuntimeArgs {
    let mut runtime_args = RuntimeArgs::new();
    entry_point
        .arguments
        .iter()
        .enumerate()
        .for_each(|(_idx, arg)| {
            let parts: Vec<CommandArg> = flat_arg(arg, types, false);

            let cl_value = if parts.len() == 1 {
                let input = args
                    .get_many::<String>(&arg.name)
                    .unwrap_or_default()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>();
                let ty = &arg.ty.0;
                if input.is_empty() {
                    return;
                }
                match ty {
                    NamedCLType::List(inner) => {
                        let input = input
                            .iter()
                            .map(|v| v.split(',').collect::<Vec<_>>())
                            .flatten()
                            .collect();
                        let bytes = types::vec_into_bytes(inner, input);
                        let cl_type =
                            CLType::List(Box::new(types::named_cl_type_to_cl_type(inner)));
                        CLValue::from_components(cl_type, bytes)
                    }
                    _ => {
                        let bytes = types::into_bytes(ty, input[0]);
                        let cl_type = types::named_cl_type_to_cl_type(ty);
                        CLValue::from_components(cl_type, bytes)
                    }
                }
            } else {
                build_complex_arg(parts, args)
            };
            runtime_args.insert_cl_value(arg.name.clone(), cl_value);
        });
    runtime_args
}

#[derive(Debug, PartialEq)]
struct ComposedArg<'a> {
    name: String,
    values: Vec<Values<'a>>,
}
type Values<'a> = (NamedCLType, Vec<&'a str>);

impl<'a> ComposedArg<'a> {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            values: vec![],
        }
    }

    fn add(&mut self, value: Values<'a>) {
        self.values.push(value);
    }

    fn flush(&mut self, buffer: &mut Vec<u8>) {
        if self.values.is_empty() {
            return;
        }
        let size = self.values[0].1.len();
        buffer.extend((size as u32).to_bytes().unwrap());

        (0..size).for_each(|i| {
            for (ty, values) in &self.values {
                let bytes = types::into_bytes(ty, values[i]);
                buffer.extend_from_slice(&bytes);
            }
        });
        self.values.clear();
    }
}

fn build_complex_arg(args: Vec<CommandArg>, matches: &ArgMatches) -> CLValue {
    let mut current_group = ComposedArg::new("");
    let mut buffer: Vec<u8> = vec![];
    for arg in args {
        let args = matches
            .get_many::<String>(&arg.name)
            .expect("Arg not found")
            .map(|v| v.as_str())
            .collect::<Vec<_>>();
        let ty = arg.ty;
        let is_list_element = arg.is_list_element;

        let parts = arg
            .name
            .split(".")
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let parent = parts[parts.len() - 2].clone();

        if &current_group.name != &parent && is_list_element {
            current_group.flush(&mut buffer);
            current_group = ComposedArg::new(&parent);
            current_group.add((ty, args));
        } else if &current_group.name == &parent && is_list_element {
            current_group.add((ty, args));
        } else {
            current_group.flush(&mut buffer);
            let bytes = types::into_bytes(&ty, args[0]);
            buffer.extend_from_slice(&bytes);
        }
    }
    current_group.flush(&mut buffer);
    CLValue::from_components(CLType::Any, buffer)
}

pub fn decode<'a>(bytes: &'a [u8], ty: &Type, types: &'a CustomTypeSet) -> (String, &'a [u8]) {
    match &ty.0 {
        NamedCLType::Custom(name) => {
            let matching_type = types
                .iter()
                .find(|ty| {
                    let type_name = match ty {
                        CustomType::Struct { name, .. } => &name.0,
                        CustomType::Enum { name, .. } => &name.0,
                    };
                    name == type_name
                })
                .expect("Type not found");
            let mut bytes = bytes;

            match matching_type {
                CustomType::Struct { members, .. } => {
                    let mut decoded = "{ ".to_string();
                    for field in members {
                        let (value, rem) = decode(bytes, &field.ty, types);
                        decoded.push_str(format!(" \"{}\": \"{}\",", field.name, value).as_str());
                        bytes = rem;
                    }
                    decoded.pop();
                    decoded.push_str(" }");
                    let json = Value::from_str(&decoded).unwrap();
                    (serde_json::to_string_pretty(&json).unwrap(), bytes)
                }
                CustomType::Enum { variants, .. } => {
                    let ty = Type(NamedCLType::U8);
                    let (value, rem) = decode(bytes, &ty, types);

                    let variant = variants
                        .iter()
                        .find(|v| v.discriminant == value.parse::<u16>().unwrap())
                        .expect("Variant not found");
                    bytes = rem;
                    (variant.name.clone(), bytes)
                }
            }
        }
        NamedCLType::List(inner) => {
            let ty = Type(*inner.clone());
            let mut bytes = bytes;
            let mut decoded = "[".to_string();
            let (len, rem) = u32::from_bytes(bytes).unwrap();
            bytes = rem;
            for _ in 0..len {
                let (value, rem) = decode(bytes, &ty, types);
                bytes = rem;
                decoded.push_str(format!("{},", value).as_str());
            }
            decoded.pop();
            decoded.push_str("]");
            let json = Value::from_str(&decoded).unwrap();
            (serde_json::to_string_pretty(&json).unwrap(), bytes)
        }
        _ => types::from_bytes(&ty.0, bytes),
    }
}

pub fn attached_value_arg() -> Arg {
    Arg::new("__attached_value")
        .help("The amount of CSPR attached to the call")
        .long("__attached_value")
        .required(false)
        .value_name("VALUE")
        .action(ArgAction::Set)
}

#[cfg(test)]
mod t {
    use clap::{Arg, Command};
    use odra::{
        casper_types::{bytesrepr::Bytes, runtime_args, RuntimeArgs, U512},
        schema::{
            casper_contract_schema::{Access, Argument, Entrypoint, NamedCLType, Type},
            SchemaCustomTypes,
        },
        Address,
    };

    use crate::{args::CommandArg, CustomTypeSet};

    #[odra::odra_type]
    pub struct NameTokenMetadata {
        pub token_hash: String,
        pub expiration: u64,
        pub resolver: Option<Address>,
    }

    #[odra::odra_type]
    pub struct PaymentVoucher {
        payment: PaymentInfo,
        names: Vec<NameMintInfo>,
        voucher_expiration: u64,
    }

    #[odra::odra_type]
    pub struct PaymentInfo {
        pub buyer: Address,
        pub payment_id: String,
        pub amount: U512,
    }

    #[odra::odra_type]
    pub struct NameMintInfo {
        pub label: String,
        pub owner: Address,
        pub token_expiration: u64,
    }

    const NAMED_TOKEN_METADATA_BYTES: [u8; 50] = [
        4, 0, 0, 0, 107, 112, 111, 98, 0, 32, 74, 169, 209, 1, 0, 0, 1, 1, 226, 74, 54, 110, 186,
        196, 135, 233, 243, 218, 49, 175, 91, 142, 42, 103, 172, 205, 97, 76, 95, 247, 61, 188, 60,
        100, 10, 52, 124, 59, 94, 73,
    ];

    const NAMED_TOKEN_METADATA_JSON: &str = r#"{
  "token_hash": "kpob",
  "expiration": "2000000000000",
  "resolver": "Key::Hash(e24a366ebac487e9f3da31af5b8e2a67accd614c5ff73dbc3c640a347c3b5e49)"
}"#;

    #[test]
    fn test_decode() {
        let custom_types = custom_types();

        let ty = Type(NamedCLType::Custom("NameTokenMetadata".to_string()));
        let (result, _bytes) = super::decode(&NAMED_TOKEN_METADATA_BYTES, &ty, &custom_types);
        pretty_assertions::assert_eq!(result, NAMED_TOKEN_METADATA_JSON);
    }

    #[test]
    fn test_command_args() {
        let entry_point = entry_point();
        let custom_types = custom_types();

        let args = entry_point
            .arguments
            .iter()
            .flat_map(|arg| super::flat_arg(arg, &custom_types, false))
            .collect::<Vec<_>>();

        let expected = command_args();
        pretty_assertions::assert_eq!(args, expected);
    }

    #[test]
    fn test_compose() {
        let entry_point = entry_point();
        let args = command_args()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<Arg>>();
        let mut cmd = Command::new("myprog");
        for a in args {
            cmd = cmd.arg(a);
        }
        let args = cmd.get_matches_from(vec![
            "myprog",
            "--voucher.payment.buyer",
            "hash-56fef1f62d86ab68655c2a5d1c8b9ed8e60d5f7e59736e9d4c215a40b10f4a22",
            "--voucher.payment.payment_id",
            "id_001",
            "--voucher.payment.amount",
            "666",
            "--voucher.names.label",
            "kpob",
            "--voucher.names.owner",
            "hash-f01cec215ddfd4c4a19d58f9c917023391a1da871e047dc47a83ae55f6cfc20a",
            "--voucher.names.token_expiration",
            "1000000",
            "--voucher.names.label",
            "qwerty",
            "--voucher.names.owner",
            "hash-f01cec215ddfd4c4a19d58f9c917023391a1da871e047dc47a83ae55f6cfc20a",
            "--voucher.names.token_expiration",
            "1000000",
            "--voucher.voucher_expiration",
            "2000000",
            "--signature",
            "1,148,81,107,136,16,186,87,48,202,151",
        ]);
        let types = custom_types();
        let args = super::compose(&entry_point, &args, &types);
        let expected = runtime_args! {
            "voucher" => PaymentVoucher {
                payment: PaymentInfo {
                    buyer: "hash-56fef1f62d86ab68655c2a5d1c8b9ed8e60d5f7e59736e9d4c215a40b10f4a22".parse().unwrap(),
                    payment_id: "id_001".parse().unwrap(),
                    amount: U512::from_dec_str("666").unwrap()
                 },
                names: vec![
                    NameMintInfo {
                        label: "kpob".to_string(),
                        owner: "hash-f01cec215ddfd4c4a19d58f9c917023391a1da871e047dc47a83ae55f6cfc20a".parse().unwrap(),
                        token_expiration: 1000000
                    },
                    NameMintInfo {
                        label: "qwerty".to_string(),
                        owner: "hash-f01cec215ddfd4c4a19d58f9c917023391a1da871e047dc47a83ae55f6cfc20a".parse().unwrap(),
                        token_expiration: 1000000
                    }
                ],
                voucher_expiration: 2000000
            },
            "signature" => Bytes::from(vec![1u8, 148u8, 81u8, 107u8, 136u8, 16u8, 186u8, 87u8, 48u8, 202u8, 151u8]),
        };
        pretty_assertions::assert_eq!(args, expected);
    }

    fn entry_point() -> Entrypoint {
        Entrypoint {
            name: "test".to_string(),
            description: None,
            is_mutable: false,
            arguments: vec![
                Argument::new(
                    "voucher",
                    "",
                    NamedCLType::Custom("PaymentVoucher".to_string()),
                ),
                Argument::new(
                    "signature",
                    "",
                    NamedCLType::List(Box::new(NamedCLType::U8)),
                ),
            ],
            return_ty: Type(NamedCLType::Bool),
            is_contract_context: true,
            access: Access::Public,
        }
    }

    fn command_args() -> Vec<CommandArg> {
        vec![
            CommandArg::new("voucher.payment.buyer", "", NamedCLType::Key, true, false),
            CommandArg::new(
                "voucher.payment.payment_id",
                "",
                NamedCLType::String,
                true,
                false,
            ),
            CommandArg::new("voucher.payment.amount", "", NamedCLType::U512, true, false),
            CommandArg::new("voucher.names.label", "", NamedCLType::String, true, true),
            CommandArg::new("voucher.names.owner", "", NamedCLType::Key, true, true),
            CommandArg::new(
                "voucher.names.token_expiration",
                "",
                NamedCLType::U64,
                true,
                true,
            ),
            CommandArg::new(
                "voucher.voucher_expiration",
                "",
                NamedCLType::U64,
                true,
                false,
            ),
            CommandArg::new("signature", "", NamedCLType::U8, true, true),
        ]
    }

    fn custom_types() -> CustomTypeSet {
        CustomTypeSet::from_iter(PaymentVoucher::schema_types().into_iter().filter_map(|t| t))
    }
}
