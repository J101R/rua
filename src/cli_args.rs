use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum, ValueHint};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum CLIColorType {
	#[clap(name = "auto")]
	#[default]
	Auto,
	#[clap(name = "never")]
	Never,
	#[clap(name = "always")]
	Always
}

#[derive(Parser, Debug)]
#[clap(
	rename_all = "kebab-case",
	after_help = "ENVIRONMENT:\n    RUA_SUDO_COMMAND: Sets the alternative command for sudo, such as gosu, doas, runas, suex etc."
)]
#[command(author, version, about)]
pub struct CliArgs {
	#[clap(
		long,
		default_value = "auto",
		help = "Set colors. Respects NO_COLOR environment and CLICOLOR specification"
	)]
	pub color: CLIColorType,
	#[command(subcommand)]
	pub action: Action,
}

#[derive(Subcommand, Clone, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Action {
	#[clap(
		about = "Build package in specified directory, in jail",
	)]
	Builddir {
		#[clap(
			short,
			long,
			help = "Forbid internet access while building packages.
Sources are downloaded using .SRCINFO only"
		)]
		offline: bool,
		#[clap(
			short,
			long,
			help = "Use --force option with makepkg, see makepkg(8)"
		)]
		force: bool,
		#[clap(
			help = "Target directory. Defaults to current directory '.' if not specified.",
			value_hint = ValueHint::DirPath
		)]
		target: Option<PathBuf>,
	},
	#[clap(about = "Show package information")]
	Info {
		#[clap(
			help = "Target to show for",
			value_hint = ValueHint::CommandWithArguments,
			required = true
		)]
		target: Vec<String>,
	},
	#[clap(about = "Download a package by name and build it in jail")]
	Install {
		#[clap(
			long,
			help = "Install package as dependency"
		)]
		asdeps: bool,
		#[clap(
			short,
			long,
			help = "Forbid internet access while building packages.
Sources are downloaded using .SRCINFO only"
		)]
		offline: bool,
		#[clap(
			help = "Target package",
			value_hint = ValueHint::CommandWithArguments,
			required = true
		)]
		target: Vec<String>,
	},
	#[clap(
		about = "Search for packages by name or description. If multiple keywords are used, all of them must match."
	)]
	Search {
		#[clap(
			help = "Target to search for",
			value_hint = ValueHint::CommandWithArguments,
			required = true
		)]
		target: Vec<String>,
	},
	#[clap(
		about = "Run shellcheck on a PKGBUILD, taking care of PKGBUILD-specific variables"
	)]
	Shellcheck {
		#[clap(
			help = "PKGBUILD or directory to check. Defaults to /dev/stdin if not specified. Appends ./PKGBUILD for directories",
			value_hint = ValueHint::AnyPath
		)]
		target: Option<PathBuf>,
	},
	#[clap(
		about = "Check *.pkg.tar or *.pkg.tar.xz  or *.pkg.tar.gz or *.pkg.tar.zst archive"
	)]
	Tarcheck {
		#[clap(help = "Archive to check", value_hint = ValueHint::FilePath, required = true)]
		target: PathBuf,
	},
	#[clap(
		about = "Upgrade AUR packages. To ignore packages, add them to IgnorePkg in /etc/pacman.conf"
	)]
	Upgrade {
		#[clap(
			long,
			short,
			help = "Also rebuild development packages.
Supports: git, hg, bzr, svn, cvs, darcs. Currently by suffix only."
		)]
		devel: bool,
		#[clap(
			long,
			help = "Print the list of outdated packages to stdout, delimited by newline. Don't upgrade anything, don't ask questions (for use in scripts). Exits with code 7 if no upgrades are available."
		)]
		printonly: bool,
		#[clap(
			long,
			value_hint = ValueHint::Other,
			help = "Don't upgrade the specified package(s). Accepts multiple arguments separated by `,`."
		)]
		ignored: Option<String>,
		#[clap(
			help = "upgrade the specified package(s). Accepts multiple names separated by a space.",
			value_hint = ValueHint::CommandWithArguments
		)]
		target: Option<Vec<String>>
	}
}

/// environment variable that we expect the user might fill
// !WARNING! If you change this, make sure the value the same as documented in CliArgs above.
#[allow(dead_code)] // unused from inside build.rs
pub const SUDO_ENVIRONMENT_VARIABLE_NAME: &str = "RUA_SUDO_COMMAND";
