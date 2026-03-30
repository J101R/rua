mod action_builddir;
mod action_install;
mod action_search;
mod action_upgrade;
mod alpm_wrapper;
mod aur_rpc_utils;
mod cli_args;
mod git_utils;
mod pacman;
mod print_format;
mod print_package_info;
mod print_package_table;
mod reviewing;
mod rua_environment;
mod rua_paths;
mod srcinfo_to_pkgbuild;
mod tar_check;
mod terminal_util;
mod wrapped;
mod exit_codes;

use crate::print_package_info::info;
use crate::wrapped::shellcheck;
use anyhow::bail;
use anyhow::{Context, Result, Ok};
use clap::Parser;
use cli_args::Action;
use cli_args::CliArgs;
use nix::unistd::geteuid;
use std::collections::HashSet;
use std::process::ExitCode;

fn main() -> Result<ExitCode> {
	if geteuid().is_root() {
		bail!("RUA does not allow building as root.\nAlso, makepkg will not allow you building as root anyway.")
	}
 	let cli_args: CliArgs = CliArgs::parse();
	rua_environment::prepare_environment(&cli_args);
	match &cli_args.action {
		Action::Info { target } => {
			info(target, false)
				.context("Failed to find info")?
		}
		Action::Install {
			asdeps,
			offline,
			target,
		} => {
			let paths = rua_paths::RuaPaths::initialize_paths()?;
			action_install::install(target, &paths, *offline, *asdeps)?
		}
		Action::Builddir {
			offline,
			force,
			target,
		} => {
			let paths = rua_paths::RuaPaths::initialize_paths()?;
			action_builddir::action_builddir(target, &paths, *offline, *force)?
		}
		Action::Search { target } => {
			let code = action_search::action_search(target)?;
			return Ok(code);
		},
		Action::Shellcheck { target } => {
			shellcheck(target)?
		}
		Action::Tarcheck { target } => {
			tar_check::tar_check(
				target,
				target.to_str().context("target is not valid UTF-8")?,
			)?;
			eprintln!("Finished checking package: {:?}", target);
		}
		Action::Upgrade {
			devel,
			printonly,
			ignored,
			target,
		} => {
			let ignored_set = ignored
				.iter()
				.flat_map(|i| i.split(','))
				.collect::<HashSet<&str>>();
			if *printonly {
				action_upgrade::upgrade_printonly(*devel, &ignored_set, target);
			} else {
				let paths = rua_paths::RuaPaths::initialize_paths()?;
				let code = action_upgrade::upgrade_real(*devel, &paths, &ignored_set, target)?;
				return Ok(code);
			}
		}
	};
	Ok(ExitCode::SUCCESS)
}
