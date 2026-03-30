use crate::terminal_util;
extern crate libflate;
extern crate ruzstd;
use anyhow::Context;
use anyhow::Ok;
use anyhow::bail;
use colored::*;
use indexmap::IndexSet;
use libflate::gzip::Decoder;
use liblzma::read::XzDecoder;
use log::debug;
use ruzstd::decoding::StreamingDecoder;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use anyhow::Result;
use tar::*;

pub fn tar_check(tar_file: &Path, tar_str: &str) -> Result<()> {
	let archive = File::open(tar_file)
		.with_context(|| format!("cannot open file {}", tar_str))?;
	debug!("Checking file {}", tar_str);
	if tar_str.ends_with(".tar") {
		tar_check_archive(Archive::new(archive), tar_str)?;
	} else if tar_str.ends_with(".tar.xz") || tar_str.ends_with(".tar.lzma") {
		tar_check_archive(Archive::new(XzDecoder::new(archive)), tar_str)?;
	} else if tar_str.ends_with(".tar.gz") || tar_str.ends_with(".tar.gzip") {
		let decoded = Decoder::new(archive)
			.with_context(|| format!("File {:?} seems to be corrupted, could not decode the gzip contents", tar_file))?;
		tar_check_archive(Archive::new(decoded), tar_str)?;
	} else if tar_str.ends_with(".tar.zst") || tar_str.ends_with(".tar.zstd") {
		let mut archive = archive;
		let decoder = StreamingDecoder::new(&mut archive)
			.with_context(|| format!("File {:?} seems to be corrupted, could not decode the zstd contents", tar_file))?;
		tar_check_archive(Archive::new(decoder), tar_str)?;
	} else {
		bail!("Archive {:?} cannot be analyzed. Only .tar or .tar.xz or .tar.gz or .tar.zst files are supported", tar_file)
	}
	Ok(())
}

fn tar_check_archive<R: Read>(mut archive: Archive<R>, path_str: &str) -> Result<()> {
	let mut install_file = String::new();
	let mut all_files = Vec::new();
	let mut executable_files = Vec::new();
	let mut suid_files = Vec::new();
	let archive_files = archive
		.entries()
		.with_context(|| format!("cannot open archive {}", path_str))?;
	for file in archive_files {
		let mut file =
			file.with_context(|| format!("cannot access tar file in {}", path_str))?;
		let path = {
			let path = file.header().path().with_context(||
				format!(
					"Failed to extract tar file metadata for file in {}",
					path_str,
				)
			)?;
			path.to_str()
				.with_context(|| format!("{}:{} failed to parse file name", file!(), line!()))?
				.to_owned()
		};
		let mode = file.header().mode().with_context(||
			format!(
				"{}:{} Failed to get file mode for file {}",
				file!(),
				line!(),
				path
			)
		)?;
		let is_normal = !path.ends_with('/') && !path.starts_with('.');
		if is_normal {
			all_files.push(path.clone());
		}
		if is_normal && (mode & 0o111 > 0) {
			executable_files.push(path.clone());
		}
		if mode > 0o777 {
			suid_files.push(path.clone());
		}
		if &path == ".INSTALL" {
			file.read_to_string(&mut install_file).with_context(||
				format!("Failed to read INSTALL script from tar file {}", path_str)
			)?;
		}
	}

	let has_install = !install_file.is_empty();
	loop {
		if suid_files.is_empty() {
			eprintln!("Package {} has no SUID files.", path_str);
		}
		eprint!("{}=list executable files, ", "[E]".bold());
		eprint!("{}=list all files, ", "[L]".bold());
		eprint!("{}=list files not existing on filesystem, ", "[F]".bold());

		eprint!(
			"{}{}, ",
			"[T]".bold().cyan(),
			"=run shell to inspect".cyan()
		);

		if has_install {
			eprint!(
				"{}=show {}, ",
				"[I]".bold(),
				"install file".bold().bright_red()
			);
		};

		if !suid_files.is_empty() {
			eprint!(
				"{}=list {}, ",
				"[S]".bold(),
				"SUID files".bold().bright_red()
			);
		};
		eprint!("{}=ok, proceed. ", "[O]".bold());
		let string = terminal_util::read_line_lowercase();
		eprintln!();
		if &string == "s" && !suid_files.is_empty() {
			for path in &suid_files {
				eprintln!("{}", path);
			}
		} else if &string == "e" {
			for path in &executable_files {
				eprintln!("{}", path);
			}
		} else if &string == "f" {
			for path in &all_files {
				if !Path::exists(Path::new(&format!("/{}", &path))) {
					eprintln!("{}", path);
				}
			}
		} else if &string == "l" {
			for path in &all_files {
				eprintln!("{}", path);
			}
		} else if &string == "i" && has_install {
			eprintln!("{}", &install_file);
		} else if &string == "t" {
			let dir = PathBuf::from(path_str);
			let dir = dir.parent()
				.unwrap_or_else(|| Path::new("."));
			eprintln!("Exit the shell with `logout` or Ctrl-D...");
			terminal_util::run_env_command(dir, "SHELL", "bash", &[]);
		} else if &string == "o" {
			break;
		} else if &string == "q" {
			eprintln!("Exiting...");
			std::process::exit(-1);
		}
	}
	Ok(())
}

pub fn common_suffix_length(pkg_names: &[&str], archive_whitelist: &IndexSet<&str>) -> usize {
	let min_len = pkg_names.iter().map(|p| p.len()).min().unwrap_or(0);
	for suffix_length in 0..min_len {
		for pkg in pkg_names {
			let suffix_start = pkg.len() - suffix_length;
			let prefix = &pkg[..suffix_start];
			if archive_whitelist.contains(prefix) {
				return suffix_length;
			}
		}
	}
	min_len
}

#[cfg(test)]
mod tests {
	use crate::tar_check::*;
	use indexmap::IndexSet;

	fn test(files: &[&str], whitelist: &[&str], expected: usize) {
		let set: IndexSet<&str> = whitelist.iter().copied().collect();
		let result = common_suffix_length(files, &set);
		assert_eq!(result, expected)
	}

	#[test]
	fn test_all() {
		test(&["a-1.pkg.tar", "b-1.pkg.tar"], &["a"], 10);
		test(&["a-1.pkg.tar", "bbbb-1.pkg.tar"], &["a", "dinosaur"], 10);
		test(&["a-x-1.pkg.tar", "b-x-1.pkg.tar"], &["a-x"], 10);
		test(&["a-x-1.pkg.tar", "b-x-1.pkg.tar"], &["a"], 12);
	}
}
