#![feature(box_patterns)]
use std::{
    any::Any,
    collections::{BTreeSet, HashMap},
};

pub use args::CommandArg;
use clap::{command, Arg, ArgMatches, Command};
use odra::{
    contract_def::HasIdent,
    host::{EntryPointsCallerProvider, HostEnv},
    schema::{
        casper_contract_schema::{CustomType, Entrypoint},
        SchemaCustomTypes, SchemaEntrypoints,
    },
    Contract,
};

mod args;
mod container;
mod entry_point;
mod tmp;
mod types;

pub use container::DeployedContractsContainer;

const CONTRACTS_SUBCOMMAND: &str = "contract";
const SCENARIOS_SUBCOMMAND: &str = "scenario";
const DEPLOY_SUBCOMMAND: &str = "deploy";

pub(crate) type CustomTypeSet = BTreeSet<CustomType>;

/// DeployScript is a trait that represents a deploy script.
///
/// In a deploy script, you can define the contracts that you want to deploy to the blockchain.
pub trait DeployScript {
    fn deploy(&self, container: &mut DeployedContractsContainer, env: &HostEnv);
}

/// Scenario is a trait that represents a custom scenario.
///
/// A scenario is a user-defined set of actions that can be run in the Odra CLI.
/// If you want to run a custom scenario that calls multiple entry points,
/// you need to implement this trait.
pub trait Scenario: Any {
    fn args(&self) -> Vec<CommandArg> {
        vec![]
    }
    fn run(
        &self,
        container: DeployedContractsContainer,
        env: &HostEnv,
        args: HashMap<String, String>,
    );
}

pub trait ScenarioMetadata {
    const NAME: &'static str;
    const DESCRIPTION: &'static str;
}

/// OdraCommand is a trait that represents a command that can be run in the Odra CLI.
trait OdraCommand {
    fn name(&self) -> &str;
    fn run(&self, args: &ArgMatches, env: &HostEnv, types: &CustomTypeSet);
}

/// OdraCliCommand is an enum that represents the different commands that can be run in the Odra CLI.
enum OdraCliCommand {
    Deploy(DeployCmd),
    Scenario(ScenarioCmd),
    Contract(ContractCmd),
}

impl OdraCommand for OdraCliCommand {
    fn name(&self) -> &str {
        match self {
            OdraCliCommand::Deploy(deploy) => deploy.name(),
            OdraCliCommand::Scenario(scenario) => scenario.name(),
            OdraCliCommand::Contract(contract) => contract.name(),
        }
    }

    fn run(&self, args: &ArgMatches, env: &HostEnv, types: &CustomTypeSet) {
        match self {
            OdraCliCommand::Deploy(deploy) => deploy.run(args, env, types),
            OdraCliCommand::Scenario(scenario) => scenario.run(args, env, types),
            OdraCliCommand::Contract(contract) => contract.run(args, env, types),
        }
    }
}

/// DeployCmd is a struct that represents the deploy command in the Odra CLI.
///
/// The deploy command runs the [DeployScript].
struct DeployCmd {
    script: Box<dyn DeployScript>,
}

impl OdraCommand for DeployCmd {
    fn name(&self) -> &str {
        DEPLOY_SUBCOMMAND
    }

    fn run(&self, _args: &ArgMatches, env: &HostEnv, _types: &CustomTypeSet) {
        let mut container = DeployedContractsContainer::new();
        self.script.deploy(&mut container, &env)
    }
}

/// ScenarioCmd is a struct that represents a scenario command in the Odra CLI.
///
/// The scenario command runs a [Scenario]. A scenario is a user-defined set of actions that can be run in the Odra CLI.
struct ScenarioCmd {
    name: String,
    scenario: Box<dyn Scenario>,
}

impl OdraCommand for ScenarioCmd {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, args: &ArgMatches, env: &HostEnv, _types: &CustomTypeSet) {
        let container = DeployedContractsContainer::load().expect("No deployed contracts found");

        let args = self
            .scenario
            .args()
            .into_iter()
            .filter_map(|arg| {
                let value = args.get_one::<String>(arg.name.as_str());
                if arg.required && value.is_none() {
                    panic!("Missing argument: {}", arg.name);
                }
                if value.is_none() {
                    return None;
                }
                Some((
                    arg.name.clone(),
                    // TODO: handle get_many
                    args.get_one::<String>(arg.name.as_str())
                        .expect("Missing argument")
                        .to_string(),
                ))
            })
            .collect();

        self.scenario.run(container, env, args);
    }
}

/// ContractCmd is a struct that represents a contract command in the Odra CLI.
///
/// The contract command runs a contract with a given entry point.
struct ContractCmd {
    name: String,
    commands: Vec<Box<dyn OdraCommand>>,
}

impl OdraCommand for ContractCmd {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, args: &ArgMatches, env: &HostEnv, types: &CustomTypeSet) {
        args.subcommand().map(|(entrypoint_name, entrypoint_args)| {
            let entry_point = self
                .commands
                .iter()
                .find(|cmd| cmd.name() == entrypoint_name)
                .expect("Contract action not found");
            entry_point.run(entrypoint_args, env, types);
        });
    }
}

/// CallCmd is a struct that represents a call command in the Odra CLI.
///
/// The call command runs a contract with a given entry point.
struct CallCmd {
    contract_name: String,
    entry_point: Entrypoint,
}

impl OdraCommand for CallCmd {
    fn name(&self) -> &str {
        &self.entry_point.name
    }

    fn run(&self, args: &ArgMatches, env: &HostEnv, types: &CustomTypeSet) {
        let entry_point = &self.entry_point;
        let contract_name = &self.contract_name;

        entry_point::call(env, contract_name, entry_point, args, types);
    }
}

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
    pub fn contract<T: SchemaEntrypoints + SchemaCustomTypes + Contract>(mut self) -> Self {
        let contract_name = T::HostRef::ident();
        if let Some(container) = DeployedContractsContainer::load() {
            let caller = T::HostRef::entry_points_caller(&self.host_env);
            let address = container
                .address(&contract_name)
                .expect("Contract not found");
            self.host_env
                .register_contract(address, contract_name.clone(), caller);
        }
        self.custom_types
            .extend(T::schema_types().into_iter().filter_map(|ty| ty));
        tmp::register_missing_types(&mut self.custom_types);

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
        let commands = T::schema_entrypoints()
            .into_iter()
            .map(|entry_point| {
                Box::new(CallCmd {
                    contract_name: contract_name.clone(),
                    entry_point,
                }) as Box<dyn OdraCommand>
            })
            .collect::<Vec<_>>();
        self.commands.push(OdraCliCommand::Contract(ContractCmd {
            name: contract_name,
            commands,
        }));
        self
    }

    /// Add a deploy script to the CLI
    pub fn deploy(mut self, script: impl DeployScript + 'static) -> Self {
        // register a subcommand for the deploy script
        self.main_cmd = self
            .main_cmd
            .subcommand(command!(DEPLOY_SUBCOMMAND).about("Runs the deploy script"));
        // store a command
        self.commands.push(OdraCliCommand::Deploy(DeployCmd {
            script: Box::new(script),
        }));
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
        self.commands.push(OdraCliCommand::Scenario(ScenarioCmd {
            name: S::NAME.to_string(),
            scenario: Box::new(scenario),
        }));
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
        self.main_cmd.get_matches().subcommand().map(
            |(subcommand, sub_matches)| match subcommand {
                DEPLOY_SUBCOMMAND => {
                    find_deploy(&self.commands)
                        .expect("Deploy command not found")
                        .run(sub_matches, &self.host_env, &self.custom_types);
                }
                CONTRACTS_SUBCOMMAND => {
                    sub_matches
                        .subcommand()
                        .map(|(contract_name, entrypoint_matches)| {
                            find_contract(&self.commands, contract_name)
                                .expect("Contract not found")
                                .run(entrypoint_matches, &self.host_env, &self.custom_types);
                        });
                }
                SCENARIOS_SUBCOMMAND => {
                    sub_matches.subcommand().map(|(subcommand, sub_matches)| {
                        find_scenario(&self.commands, subcommand)
                            .expect("Scenario not found")
                            .run(sub_matches, &self.host_env, &self.custom_types);
                    });
                }
                _ => {
                    panic!("Unknown subcommand");
                }
            },
        );
    }
}

fn find_scenario<'a>(commands: &'a [OdraCliCommand], name: &str) -> Option<&'a OdraCliCommand> {
    commands.iter().find(|cmd| match cmd {
        OdraCliCommand::Scenario(scenario) => scenario.name == name,
        _ => false,
    })
}

fn find_deploy<'a>(commands: &'a [OdraCliCommand]) -> Option<&'a OdraCliCommand> {
    commands
        .iter()
        .find(|cmd| matches!(cmd, OdraCliCommand::Deploy(_)))
}

fn find_contract<'a>(
    commands: &'a [OdraCliCommand],
    contract_name: &str,
) -> Option<&'a OdraCliCommand> {
    commands.iter().find(|cmd| match cmd {
        OdraCliCommand::Contract(contract) => contract.name == contract_name,
        _ => false,
    })
}
