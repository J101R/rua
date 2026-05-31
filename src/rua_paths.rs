use crate::rua_environment;
use crate::wrapped;
use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use anyhow::bail;
use colored::Colorize;
use directories::ProjectDirs;
use log::debug;
use std::env;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::Permissions;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// All directories must exist upon `RuaPaths` creation.
pub struct RuaPaths {
	/// Subdirectory of ~/.cache/rua where packages are built after review.
	/// Note: if you need to access a particular package's directory,
	/// use `build_dir(pkgbase: &str)` instead
	pub global_build_dir: PathBuf,
	/// Subdirectory of ~/.config/rua where the package is reviewed by user, and changes are kept
	global_review_dir: PathBuf,
	/// Directory where built and user-reviewed package artifacts are stored
	global_checked_tars_dir: PathBuf,
	/// Script used to wrap `makepkg` and related commands
	pub wrapper_bwrap_script: PathBuf,
	/// makepkg configuration for PKGEXT
	pub makepkg_pkgext: String,
	/// Global lock to prevent concurrent access to project dirs
	_global_lock: File,
}

impl RuaPaths {
	/// Calculates various paths and files related to RUA.
	/// Only use for actions that require `makepkg` execution,
	/// because it does root and single-instance checks as well.
	pub fn initialize_paths() -> Result<RuaPaths> {
		let dirs = &ProjectDirs::from("com.gitlab", "vn971", "rua")
			.context("Failed to determine XDG directories")?;
		std::fs::create_dir_all(dirs.config_dir())
			.context("Failed to create project config directory")?;
		let locked_file = File::open(dirs.config_dir())
			.with_context(|| format!(
				"Failed to open config dir {:?} for locking",
				dirs.config_dir()
			))?;
		locked_file.try_lock()
			.context("Error: another RUA instance already running.")?;
		rm_rf::ensure_removed(dirs.config_dir().join(".system"))?;
		std::fs::create_dir_all(dirs.config_dir().join(".system"))
			.context("Failed to create project config directory")?;
		std::fs::create_dir_all(dirs.config_dir().join("wrap_args.d"))
			.context("Failed to create project config directory")?;

		let seccomp_path = &dirs.config_dir().join(SECCOMP_PATH);
		overwrite_file(seccomp_path, SECCOMP_BPF)?;
		let seccomp_path = seccomp_path
			.to_str()
			.context("Failed to convert seccomp path to string")?;
		rua_environment::set_env_if_not_set("RUA_SECCOMP_FILE", seccomp_path);

		overwrite_script(&dirs.config_dir().join(WRAP_SCRIPT_PATH), WRAP_SH)?;
		overwrite_script(
			&dirs.config_dir().join(MAKEPKG_CONFIG_LOADER_PATH),
			CONFIG_LOADER,
		)?;
		ensure_script(
			&dirs.config_dir().join(".system/wrap_args.sh.example"),
			WRAP_ARGS_EXAMPLE,
		)?;
		let makepkg_config_loader_path = dirs.config_dir().join(MAKEPKG_CONFIG_LOADER_PATH);

		wrapped::check_bubblewrap_runnable();

		let global_build_dir = dirs.cache_dir().join("build");
		let global_checked_tars_dir = dirs.data_local_dir().join("checked_tars");
		let global_review_dir = dirs.config_dir().join("pkg");

		std::fs::create_dir_all(&global_build_dir)
			.context("Failed to create global build directory")?;
		let global_build_dir = global_build_dir.canonicalize().with_context(||
			format!(
				"Failed to canonicalize global build dir {:?}",
				global_build_dir
			)
		)?;
		show_legacy_dir_warnings(dirs, global_checked_tars_dir.as_path());
		std::fs::create_dir_all(&global_checked_tars_dir)
			.context("Failed to create global checked_tars directory")?;
		std::fs::create_dir_all(&global_review_dir)
			.context("Failed to create global review directory")?;

		// All directories must exist upon `RuaPaths` creation.
		Ok(RuaPaths {
			global_build_dir,
			global_review_dir,
			global_checked_tars_dir,
			wrapper_bwrap_script: dirs.config_dir().join(WRAP_SCRIPT_PATH),
			makepkg_pkgext: perform_makepkg_checks_and_return_pkgext(&makepkg_config_loader_path)?,
			_global_lock: locked_file,
		})
	}

	/// Same as `global_review_dir`, but for a specific pkgbase
	pub fn review_dir(&self, pkgbase: &str) -> PathBuf {
		self.global_review_dir.join(pkgbase)
	}

	/// Same as `global_build_dir`, but for a specific pkgbase
	pub fn build_dir(&self, pkgbase: &str) -> PathBuf {
		self.global_build_dir.join(pkgbase)
	}

	/// Same as `global_checked_tars_dir`, but for a specific pkgbase
	pub fn checked_tars_dir(&self, pkg_name: &str) -> PathBuf {
		self.global_checked_tars_dir.join(pkg_name)
	}
}

fn perform_makepkg_checks_and_return_pkgext(makepkg_config_loader_path: &Path) -> Result<String> {
	let mut pkgext = None;

	let config = Command::new(makepkg_config_loader_path)
		.output()
		.context("Internal error: failed to run makepkg config loader")?
		.stdout;
	let config = String::from_utf8(config)
		.context("makepkg config loader returned non-UTF-8 data")?;

	// format: `VAR=VALUE\0`
	let config_entries = config.split_terminator('\0').map(|line| {
		let sep_pos = line
			.find('=')
			.expect(&format!("Malformed config loader output, line: {}", line));
		(&line[..sep_pos], &line[sep_pos + 1..])
	});

	// config entries won't appear here unless set
	for (var, value) in config_entries {
		debug!("makepkg option: {} = {:?}", var, value);

		match var {
			"PKGDEST" | "SRCDEST" | "SRCPKGDEST" | "LOGDEST" | "BUILDDIR" => {
				let warn = "WARNING".yellow();
				eprintln!(
					"{}: Ignoring custom makepkg location {}. \
						RUA needs to use custom locations for its safety model, see: \
						https://github.com/vn971/rua#how-it-works--directories",
					warn, var
				);
			}

			"PKGEXT" => match value {
				".pkg.tar" | ".pkg.tar.xz" | ".pkg.tar.lzma" | ".pkg.tar.gz" | ".pkg.tar.gzip"
				| ".pkg.tar.zst" | ".pkg.tar.zstd" => {
					pkgext = Some(value.to_owned());
				}

				_ => bail!(
					"PKGEXT is set to an unsupported value: {}. \
					Only .pkg.tar or .pkg.tar.xz or .pkg.tar.gz or .pkg.tar.zst archives are \
					allowed for now. RUA needs those extensions to look inside the archives for \
					'tar_check' analysis.",
					value
				),
			},

			_ => {}
		}
	}

	for &var in &["PKGDEST", "SRCDEST", "SRCPKGDEST", "LOGDEST", "BUILDDIR"] {
		unsafe { env::set_var(var, "/dev/null") }; // make sure we override it later
	}

	Ok(
		pkgext
			.context("Internal error: no PKGEXT entry in makepkg configuration?!")?
	)
}

fn overwrite_file(path: &Path, content: &[u8]) -> Result<()> {
	let mut file = OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(true)
		.open(path)
		.with_context(|| format!("Failed to overwrite (initialize) file {:?}", path))?;
	file.write_all(content)
		.with_context(|| format!(
			"Failed to write to file {:?} during initialization",
			path
		))?;
	Ok(())
}

fn ensure_script(path: &Path, content: &[u8]) -> Result<()> {
	if !path.exists() {
		let mut file = OpenOptions::new()
			.create(true)
			.truncate(true)
			.write(true)
			.open(path)
			.with_context(|| format!("Failed to overwrite (initialize) file {:?}", path))?;
		file.write_all(content).with_context(||
			format!(
				"Failed to write to file {:?} during initialization",
				path
			)
		)?;
		fs::set_permissions(path, Permissions::from_mode(0o755))
			.with_context(|| format!("Failed to set permissions for {:?}", path))?;
	}
	Ok(())
}

fn overwrite_script(path: &Path, content: &[u8]) -> Result<()> {
	overwrite_file(path, content)?;
	fs::set_permissions(path, Permissions::from_mode(0o755))
		.with_context(|| format!("Failed to set permissions for {:?}", path))?;
	Ok(())
}

fn show_legacy_dir_warnings(dirs: &ProjectDirs, correct_dir: &Path) {
	let old_dir = dirs.cache_dir().join("checked_tars");
	if old_dir.exists() {
		let old_dir_str = old_dir
			.to_str()
			.unwrap_or("~/.cache/rua/checked_tars");
		eprintln!(
			"INFO: you have a legacy directory from an older RUA version: {}",
			&old_dir_str
		);
		eprintln!("Please delete it or move all contents to {:?}", correct_dir);
	};
}

pub const SHELLCHECK_WRAPPER: &str = include_str!("../res/shellcheck-wrapper");
pub const SECCOMP_BPF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/seccomp.bpf"));
pub const WRAP_SH: &[u8] = include_bytes!("../res/wrapper/security-wrapper.sh");
pub const WRAP_ARGS_EXAMPLE: &[u8] = include_bytes!("../res/wrapper/wrap_args.sh.example");
pub const CONFIG_LOADER: &[u8] = include_bytes!("../res/print_makepkg_config.sh");

pub const WRAP_SCRIPT_PATH: &str = ".system/security-wrapper.sh";
pub const MAKEPKG_CONFIG_LOADER_PATH: &str = ".system/print_makepkg_config.sh";
pub const SECCOMP_PATH: &str = ".system/seccomp.bpf";
