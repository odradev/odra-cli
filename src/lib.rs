#![feature(box_patterns, error_generic_member_access)]
use std::collections::BTreeSet;

pub use args::CommandArg;
use clap::{command, Arg, Command};
use cmd::{OdraCliCommand, OdraCommand};
use odra::{
    contract_def::HasIdent,
    host::{EntryPointsCallerProvider, HostEnv},
    schema::{casper_contract_schema::CustomType, SchemaCustomTypes, SchemaEntrypoints},
    OdraContract,
};

mod args;
mod cmd;
mod container;
mod entry_point;
mod types;

pub use cmd::{
    deploy::{DeployError, DeployScript},
    scenario::{Scenario, ScenarioArgs, ScenarioError, ScenarioMetadata},
};
pub use container::DeployedContractsContainer;

const CONTRACTS_SUBCOMMAND: &str = "contract";
const SCENARIOS_SUBCOMMAND: &str = "scenario";
const DEPLOY_SUBCOMMAND: &str = "deploy";

pub(crate) type CustomTypeSet = BTreeSet<CustomType>;

/// OdraCli is a struct that represents the Odra CLI.
///
/// The Odra CLI is a command line interface that allows users to interact with the blockchain.
pub struct OdraCli {
    main_cmd: Command,
    scenarios_cmd: Command,
    contracts_cmd: Command,
    commands: Vec<OdraCliCommand>,
    custom_types: CustomTypeSet,
    host_env: HostEnv,
}

impl OdraCli {
    pub fn new() -> Self {
        let contracts_cmd = Command::new(CONTRACTS_SUBCOMMAND)
            .about("Commands for interacting with contracts")
            .subcommand_required(true)
            .arg_required_else_help(true);
        let scenarios_cmd = Command::new(SCENARIOS_SUBCOMMAND)
            .about("Commands for running user-defined scenarios")
            .subcommand_required(true)
            .arg_required_else_help(true);
        let main_cmd = Command::new("Odra CLI")
            .subcommand_required(true)
            .arg_required_else_help(true);

        Self {
            main_cmd,
            commands: vec![],
            custom_types: CustomTypeSet::new(),
            host_env: odra_casper_livenet_env::env(),
            contracts_cmd,
            scenarios_cmd,
        }
    }

    /// Set the description of the CLI
    pub fn about(mut self, about: &str) -> Self {
        self.main_cmd = self.main_cmd.about(about.to_string());
        self
    }

    /// Add a contract to the CLI
    pub fn contract<T: SchemaEntrypoints + SchemaCustomTypes + OdraContract>(mut self) -> Self {
        let contract_name = T::HostRef::ident();
        if let Ok(container) = DeployedContractsContainer::load() {
            let caller = T::HostRef::entry_points_caller(&self.host_env);
            let address = container
                .address(&contract_name)
                .expect("Contract not found");
            self.host_env
                .register_contract(address, contract_name.clone(), caller);
        }
        self.custom_types
            .extend(T::schema_types().into_iter().filter_map(|ty| ty));

        // build entry points commands
        let mut contract_cmd = Command::new(&contract_name)
            .about(format!(
                "Commands for interacting with the {} contract",
                &contract_name
            ))
            .subcommand_required(true)
            .arg_required_else_help(true);
        for entry_point in T::schema_entrypoints() {
            if entry_point.name == "init" {
                continue;
            }
            let mut ep_cmd = Command::new(&entry_point.name)
                .about(&entry_point.description.clone().unwrap_or_default());
            for arg in args::entry_point_args(&entry_point, &self.custom_types) {
                ep_cmd = ep_cmd.arg(arg);
            }
            ep_cmd = ep_cmd.arg(args::attached_value_arg());
            contract_cmd = contract_cmd.subcommand(ep_cmd);
        }
        self.contracts_cmd = self.contracts_cmd.subcommand(contract_cmd);

        // store a command
        self.commands
            .push(OdraCliCommand::new_contract::<T>(contract_name));
        self
    }

    /// Add a deploy script to the CLI
    pub fn deploy(mut self, script: impl DeployScript + 'static) -> Self {
        // register a subcommand for the deploy script
        self.main_cmd = self
            .main_cmd
            .subcommand(command!(DEPLOY_SUBCOMMAND).about("Runs the deploy script"));
        // store a command
        self.commands.push(OdraCliCommand::new_deploy(script));
        self
    }

    /// Add a scenario to the CLI
    pub fn scenario<S: ScenarioMetadata + Scenario>(mut self, scenario: S) -> Self {
        // register a subcommand for the scenario
        let mut scenario_cmd = Command::new(S::NAME).about(S::DESCRIPTION);
        let args = scenario
            .args()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<Arg>>();
        for arg in args {
            scenario_cmd = scenario_cmd.arg(arg);
        }

        self.scenarios_cmd = self.scenarios_cmd.subcommand(scenario_cmd);

        // store a command
        self.commands.push(OdraCliCommand::new_scenario(scenario));
        self
    }

    /// Build the CLI
    pub fn build(mut self) -> Self {
        self.main_cmd = self.main_cmd.subcommand(self.contracts_cmd.clone());
        self.main_cmd = self.main_cmd.subcommand(self.scenarios_cmd.clone());
        self
    }

    /// Run the CLI and parses the input
    pub fn run(self) {
        let matches = self.main_cmd.get_matches();
        let (cmd, args) = matches
            .subcommand()
            .map(|(subcommand, sub_matches)| match subcommand {
                DEPLOY_SUBCOMMAND => {
                    find_deploy(&self.commands).map(|deploy| (deploy, sub_matches))
                }
                CONTRACTS_SUBCOMMAND => {
                    sub_matches
                        .subcommand()
                        .map(|(contract_name, entrypoint_matches)| {
                            (
                                find_contract(&self.commands, contract_name),
                                entrypoint_matches,
                            )
                        })
                }
                SCENARIOS_SUBCOMMAND => {
                    sub_matches.subcommand().map(|(subcommand, sub_matches)| {
                        (find_scenario(&self.commands, subcommand), sub_matches)
                    })
                }
                _ => unreachable!(),
            })
            .flatten()
            .expect("Subcommand not found");

        match cmd.run(args, &self.host_env, &self.custom_types) {
            Ok(_) => prettycli::info("Command executed successfully"),
            Err(err) => prettycli::error(&format!("{:?}", err)),
        }
    }
}

fn find_scenario<'a>(commands: &'a [OdraCliCommand], name: &str) -> &'a OdraCliCommand {
    commands
        .iter()
        .find(|cmd| match cmd {
            OdraCliCommand::Scenario(scenario) => scenario.name() == name,
            _ => false,
        })
        .unwrap()
}

fn find_deploy<'a>(commands: &'a [OdraCliCommand]) -> Option<&'a OdraCliCommand> {
    commands
        .iter()
        .find(|cmd| matches!(cmd, OdraCliCommand::Deploy(_)))
}

fn find_contract<'a>(commands: &'a [OdraCliCommand], contract_name: &str) -> &'a OdraCliCommand {
    commands
        .iter()
        .find(|cmd| match cmd {
            OdraCliCommand::Contract(contract) => contract.name() == contract_name,
            _ => false,
        })
        .unwrap()
}
