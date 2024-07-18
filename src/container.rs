use std::{fs::File, io::Write, path::PathBuf, str::FromStr};

use chrono::{DateTime, SecondsFormat, Utc};
use odra::{
    contract_def::HasIdent,
    host::{EntryPointsCallerProvider, HostEnv, HostRef, HostRefLoader},
    Address,
};
use serde_derive::{Deserialize, Serialize};

const DEPLOYED_CONTRACTS_FILE: &str = "resources/deployed_contracts.toml";

/// This struct represents a contract in the `deployed_contracts.toml` file.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DeployedContractsContainer {
    time: String,
    contracts: Vec<Contract>,
}

impl DeployedContractsContainer {
    /// Create new instance.
    pub(crate) fn new() -> Self {
        Self::handle_previous_version();
        let now: DateTime<Utc> = Utc::now();
        Self {
            time: now.to_rfc3339_opts(SecondsFormat::Secs, true),
            contracts: Vec::new(),
        }
    }

    /// Add contract to the list.
    pub fn add_contract<T: HostRef + HasIdent>(&mut self, contract: &T) {
        self.contracts.push(Contract {
            name: T::ident(),
            package_hash: contract.address().to_string(),
        });
        self.update();
    }


    pub fn get_ref<T: EntryPointsCallerProvider + HostRef + HasIdent + 'static>(&self, env: &HostEnv) -> Option<T> {
        self.contracts
            .iter()
            .find(|c| c.name == T::ident())
            .map(|c| {
                let address = Address::from_str(&c.package_hash).unwrap();
                dbg!("registering contract");
                T::load(env, address)
            })
    }

    /// Return contract address.
    pub fn address(&self, name: &str) -> Option<Address> {
        self.contracts
            .iter()
            .find(|c| c.name == name)
            .map(|c| Address::from_str(&c.package_hash).unwrap())
    }

    /// Return creation time.
    pub(crate) fn time(&self) -> &str {
        &self.time
    }

    /// Update the file.
    pub(crate) fn update(&self) {
        let path = Self::file_path();
        self.save_at(&path);
    }

    /// Save the file at the given path.
    pub(crate) fn save_at(&self, file_path: &PathBuf) {
        let content = toml::to_string_pretty(&self).unwrap();
        let mut file = File::create(file_path).unwrap();

        file.write_all(content.as_bytes()).unwrap();
    }

    /// Load from the file.
    pub(crate) fn load() -> Option<Self> {
        let path = Self::file_path();
        std::fs::read_to_string(path)
            .ok()
            .map(|s| toml::from_str(&s).unwrap())
    }

    /// Backup previous version of the file.
    pub(crate) fn handle_previous_version() {
        if let Some(deployed_contracts) = Self::load() {
            // Build new file name.
            let date = deployed_contracts.time();
            let mut path = project_root::get_project_root().unwrap();
            path.push(format!("{}.{}", DEPLOYED_CONTRACTS_FILE, date));

            // Store previous version under new file name.
            deployed_contracts.save_at(&path);

            // Remove old file.
            std::fs::remove_file(path).unwrap();
        }
    }

    fn file_path() -> PathBuf {
        let mut path = project_root::get_project_root().unwrap();
        path.push(DEPLOYED_CONTRACTS_FILE);

        path
    }
}

/// This struct represents a contract in the `deployed_contracts.toml` file.
#[derive(Deserialize, Serialize, Debug, Clone)]
struct Contract {
    pub name: String,
    pub package_hash: String,
}
