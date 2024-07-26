use odra::schema::casper_contract_schema::{CustomType, NamedCLType, StructMember, Type, TypeName};

use crate::CustomTypeSet;

pub fn register_missing_types(custom_types: &mut CustomTypeSet) {
    custom_types.insert(CustomType::Struct {
        name: TypeName::new("NameMintInfo"),
        description: None,
        members: vec![
            StructMember {
                name: "label".to_string(),
                description: None,
                ty: Type(NamedCLType::String),
            },
            StructMember {
                name: "owner".to_string(),
                description: None,
                ty: Type(NamedCLType::Key),
            },
            StructMember {
                name: "token_expiration".to_string(),
                description: None,
                ty: Type(NamedCLType::U64),
            },
        ],
    });
    custom_types.insert(CustomType::Struct {
        name: TypeName::new("TokenRenewalInfo"),
        description: None,
        members: vec![
            StructMember {
                name: "token_id".to_string(),
                description: None,
                ty: Type(NamedCLType::String),
            },
            StructMember {
                name: "token_expiration".to_string(),
                description: None,
                ty: Type(NamedCLType::U64),
            },
        ],
    });
    custom_types.insert(CustomType::Struct {
        name: TypeName::new("NameTokenMetadata"),
        description: None,
        members: vec![
            StructMember {
                name: "token_hash".to_string(),
                description: None,
                ty: Type(NamedCLType::String),
            },
            StructMember {
                name: "expiration".to_string(),
                description: None,
                ty: Type(NamedCLType::U64),
            },
            StructMember {
                name: "resolver".to_string(),
                description: None,
                ty: Type(NamedCLType::Option(Box::new(NamedCLType::Key))),
            },
        ],
    });
}
