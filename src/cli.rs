use clap::Command;

pub fn cli() -> Command {
    Command::new("zgit")
        .about("An alternate to Git")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("init").about(
                "zgit-init - Create an empty Zgit repository or reinitialize an existing one",
            ),
        )
        .subcommand(Command::new("commit"))
}
