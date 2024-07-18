use std::collections::HashMap;

use clap::{command, Arg, ArgAction, ArgMatches, Command};
use odra::{
    casper_types::CLValue,
    contract_def::HasIdent,
    host::HostEnv,
    schema::{
        casper_contract_schema::{Entrypoint, NamedCLType}, SchemaCustomTypes, SchemaEntrypoints
    },
};

mod container;
mod entry_point;
mod types;

pub use container::DeployedContractsContainer;

const CONTRACTS_SUBCOMMAND: &str = "contract";
const SCENARIOS_SUBCOMMAND: &str = "scenario";
const DEPLOY_SUBCOMMAND: &str = "deploy";

pub trait OdraCommand {
    fn register(&self, cmd: Command) -> Command;
    fn name(&self) -> &str;
    fn run(&self, args: &ArgMatches, parsers: &HashMap<String, Box<dyn Parser>>);
}

pub enum OdraCliCommand {
    Deploy(DeployCmd),
    Scenario(ScenarioCmd),
    Contract(ContractCmd),
}

impl OdraCommand for OdraCliCommand {
    fn register(&self, cmd: Command) -> Command {
        match self {
            OdraCliCommand::Deploy(deploy) => deploy.register(cmd),
            OdraCliCommand::Scenario(scenario) => scenario.register(cmd),
            OdraCliCommand::Contract(contract) => contract.register(cmd),
        }
    }

    fn name(&self) -> &str {
        match self {
            OdraCliCommand::Deploy(deploy) => deploy.name(),
            OdraCliCommand::Scenario(scenario) => scenario.name(),
            OdraCliCommand::Contract(contract) => contract.name(),
        }
    }

    fn run(&self, args: &ArgMatches, parsers: &HashMap<String, Box<dyn Parser>>) {
        match self {
            OdraCliCommand::Deploy(deploy) => deploy.run(args, parsers),
            OdraCliCommand::Scenario(scenario) => scenario.run(args, parsers),
            OdraCliCommand::Contract(contract) => contract.run(args, parsers),
        }
    }
}

pub trait DeployScript {
    fn deploy(&self, container: &mut DeployedContractsContainer, env: &HostEnv);
}

pub struct DeployCmd {
    script: Box<dyn DeployScript>,
}

impl OdraCommand for DeployCmd {
    fn register(&self, cmd: Command) -> Command {
        cmd.subcommand(command!(DEPLOY_SUBCOMMAND).about("Runs the deploy script"))
    }

    fn name(&self) -> &str {
        DEPLOY_SUBCOMMAND
    }

    fn run(&self, _args: &ArgMatches, _parsers: &HashMap<String, Box<dyn Parser>>) {
        let mut container = DeployedContractsContainer::new();
        let env = odra_casper_livenet_env::env();
        self.script.deploy(&mut container, &env)
    }
}

pub struct CommandArg {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
}

impl CommandArg {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            required: true,
            description: None,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }
}

impl From<CommandArg> for Arg {
    fn from(arg: CommandArg) -> Self {
        Arg::new(&arg.name)
            .long(&arg.name)
            .action(ArgAction::Set)
            .value_name("VALUE")
            .required(arg.required)
            .help(arg.description.unwrap_or_default())
    }
}

pub trait Scenario {
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

pub struct ScenarioCmd {
    name: String,
    description: String,
    scenario: Box<dyn Scenario>,
}

impl OdraCommand for ScenarioCmd {
    fn register(&self, cmd: Command) -> Command {
        let mut scenario_cmd = Command::new(&self.name).about(&self.description);
        let args = self
            .scenario
            .args()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<Arg>>();
        for arg in args {
            scenario_cmd = scenario_cmd.arg(arg);
        }

        cmd.subcommand(scenario_cmd)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, args: &ArgMatches, _parsers: &HashMap<String, Box<dyn Parser>>) {
        let container = DeployedContractsContainer::load().expect("No deployed contracts found");
        let env = odra_casper_livenet_env::env();

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
                    args.get_one::<String>(arg.name.as_str())
                        .expect("Missing argument")
                        .to_string(),
                ))
            })
            .collect();

        self.scenario.run(container, &env, args);
    }
}

pub struct ContractCmd {
    name: String,
    commands: Vec<Box<dyn OdraCommand>>,
}

impl OdraCommand for ContractCmd {
    fn register(&self, cmd: Command) -> Command {
        let mut contract_cmd = Command::new(&self.name)
            .subcommand_required(true)
            .arg_required_else_help(true);
        for cmd in &self.commands {
            contract_cmd = cmd.register(contract_cmd);
        }
        cmd.subcommand(contract_cmd)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, args: &ArgMatches, parsers: &HashMap<String, Box<dyn Parser>>) {
        args.subcommand().map(|(entrypoint_name, entrypoint_args)| {
            let entry_point = self
                .commands
                .iter()
                .find(|cmd| cmd.name() == entrypoint_name)
                .expect("Contract action not found");
            entry_point.run(entrypoint_args, parsers);
        });
    }
}

struct CallCmd {
    contract_name: String,
    entry_point: Entrypoint,
}

impl OdraCommand for CallCmd {
    fn register(&self, cmd: Command) -> Command {
        let mut ep_cmd = Command::new(&self.entry_point.name)
            .about(&self.entry_point.description.clone().unwrap_or_default());
        for arg in &self.entry_point.arguments {
            let arg = Arg::new(&arg.name)
                .long(&arg.name)
                .action(ArgAction::Set)
                .value_name("VALUE")
                .required(!arg.optional)
                .help(arg.description.clone().unwrap_or_default());
            ep_cmd = ep_cmd.arg(arg);
        }
        cmd.subcommand(ep_cmd)
    }

    fn name(&self) -> &str {
        &self.entry_point.name
    }

    fn run(&self, args: &ArgMatches, parsers: &HashMap<String, Box<dyn Parser>>) {
        let env = odra_casper_livenet_env::env();
        let container = DeployedContractsContainer::load().expect("No deployed contracts found");
        let runtime_args = types::build_args(&self.entry_point, args, parsers);
        let entry_point = &self.entry_point;
        let contract_name = &self.contract_name;
        entry_point::call(
            &env,
            &container,
            entry_point,
            runtime_args,
            contract_name,
            parsers,
        );
    }
}

pub struct OdraCli {
    command: Command,
    commands: Vec<OdraCliCommand>,
    parsers: HashMap<String, Box<dyn Parser>>,
}

pub trait Parser {
    fn parse(&self, input: &str) -> CLValue;
}

impl OdraCli {
    pub fn new() -> Self {
        Self {
            command: Command::new("Odra CLI")
                .subcommand_required(true)
                .arg_required_else_help(true),
            commands: vec![],
            parsers: HashMap::new(),
        }
    }

    /// Set the description of the CLI
    pub fn about(mut self, about: &str) -> Self {
        self.command = self.command.about(about.to_string());
        self
    }

    /// Add a contract to the CLI
    pub fn contract<T: SchemaEntrypoints + SchemaCustomTypes + HasIdent>(mut self) -> Self {
        let commands = T::schema_entrypoints()
            .into_iter()
            .map(|entry_point| {
                Box::new(CallCmd {
                    contract_name: T::ident(),
                    entry_point,
                }) as Box<dyn OdraCommand>
            })
            .collect::<Vec<_>>();
        self.commands.push(OdraCliCommand::Contract(ContractCmd {
            name: T::ident(),
            commands,
        }));
        self
    }

    /// Add a deploy script to the CLI
    pub fn deploy(mut self, script: impl DeployScript + 'static) -> Self {
        self.commands.push(OdraCliCommand::Deploy(DeployCmd {
            script: Box::new(script),
        }));
        self
    }

    /// Add a scenario to the CLI
    pub fn scenario(
        mut self,
        scenario: impl Scenario + 'static,
        name: &str,
        description: &str,
    ) -> Self {
        self.commands.push(OdraCliCommand::Scenario(ScenarioCmd {
            name: name.to_string(),
            description: description.to_string(),
            scenario: Box::new(scenario),
        }));
        self
    }

    pub fn parser(mut self, cl_type: NamedCLType, parser: impl Parser + 'static) -> Self {
        if let NamedCLType::Custom(name) = cl_type {
            self.parsers.insert(name, Box::new(parser));
        };
        self
    }

    /// Build the CLI
    pub fn build(mut self) -> Self {
        let mut contracts_command = Command::new(CONTRACTS_SUBCOMMAND)
            .about("Commands for interacting with contracts")
            .subcommand_required(true)
            .arg_required_else_help(true);

        let mut scenarios_command = Command::new(SCENARIOS_SUBCOMMAND)
            .about("Commands for running user-defined scenarios")
            .subcommand_required(true)
            .arg_required_else_help(true);

        for cmd in &self.commands {
            match cmd {
                OdraCliCommand::Deploy(cmd) => {
                    self.command = cmd.register(self.command);
                }
                OdraCliCommand::Scenario(cmd) => {
                    scenarios_command = cmd.register(scenarios_command);
                }
                OdraCliCommand::Contract(cmd) => {
                    contracts_command = cmd.register(contracts_command)
                }
            }
        }
        self.command = self.command.subcommand(contracts_command);
        self.command = self.command.subcommand(scenarios_command);

        self
    }

    /// Run the CLI and parses the input
    pub fn run(self) {
        let matches = self.command.get_matches();

        matches.subcommand().map(|(subcommand, sub_matches)| {
            match subcommand {
                DEPLOY_SUBCOMMAND => {
                    find_deploy(&self.commands)
                        .expect("Deploy command not found")
                        .run(sub_matches, &self.parsers);
                }
                CONTRACTS_SUBCOMMAND => {
                    sub_matches
                        .subcommand()
                        .map(|(contract_name, entrypoint_matches)| {
                            find_contract(&self.commands, contract_name)
                                .expect("Contract not found")
                                .run(entrypoint_matches, &self.parsers);
                        });
                }
                SCENARIOS_SUBCOMMAND => {
                    sub_matches.subcommand().map(|(subcommand, sub_matches)| {
                        find_scenario(&self.commands, subcommand)
                            .expect("Scenario not found")
                            .run(sub_matches, &self.parsers);
                    });
                }
                _ => {
                    panic!("Unknown subcommand");
                }
            }
        });
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
