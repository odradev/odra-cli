use crate::{
    container::ContractError, CustomTypeSet, DeployedContractsContainer, DEPLOY_SUBCOMMAND,
};
use anyhow::Result;
use clap::ArgMatches;
use odra::{host::HostEnv, OdraError};
use thiserror::Error;

use super::OdraCommand;

/// DeployCmd is a struct that represents the deploy command in the Odra CLI.
///
/// The deploy command runs the [DeployScript].
pub(crate) struct DeployCmd {
    pub script: Box<dyn DeployScript>,
}

impl OdraCommand for DeployCmd {
    fn name(&self) -> &str {
        DEPLOY_SUBCOMMAND
    }

    fn run(&self, _args: &ArgMatches, env: &HostEnv, _types: &CustomTypeSet) -> Result<()> {
        let mut container = DeployedContractsContainer::new()?;
        self.script.deploy(&mut container, &env)?;
        Ok(())
    }
}

/// DeployScript is a trait that represents a deploy script.
///
/// In a deploy script, you can define the contracts that you want to deploy to the blockchain.
pub trait DeployScript {
    fn deploy(
        &self,
        container: &mut DeployedContractsContainer,
        env: &HostEnv,
    ) -> core::result::Result<(), DeployError>;
}

#[derive(Debug, Error)]
pub enum DeployError {
    #[error("Deploy error: {message}")]
    OdraError { message: String },
    #[error("Contract read error: {0}")]
    ContractReadError(#[from] ContractError),
}

impl From<OdraError> for DeployError {
    fn from(err: OdraError) -> Self {
        DeployError::OdraError {
            message: format!("{:?}", err),
        }
    }
}
