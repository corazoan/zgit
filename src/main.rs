use std::process;

use zgit::init_zgit_repo;

pub mod cli;

fn main() {
    let matches = cli::cli().get_matches();
    match matches.subcommand() {
        Some(("init", _)) => {
            if let Err(err) = init_zgit_repo() {
                println!("\x1b[31mError during initilializing git repository: {err} \x1b[00m")
            };
        }

        _ => {
            println!("\x1b[31mReceive unexpected arguments: \x1b[00m");
            process::exit(1)
        }
    }
}
