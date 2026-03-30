use crate::alpm_wrapper::AlpmWrapper;
use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use indexmap::IndexMap;
use indexmap::IndexSet;
use itertools::Itertools;
use std::sync::LazyLock;
use log::trace;
use raur::blocking::Handle;
use raur::blocking::Raur;
use raur::Package;
use regex::Regex;

type RaurInfo = IndexMap<String, Package>;
type PacmanDependencies = IndexSet<String>;
type DepthMap = IndexMap<String, i32>;
type RecursiveInfo = (RaurInfo, PacmanDependencies, DepthMap);

const BATCH_SIZE: usize = 200;

pub fn recursive_info(
	root_packages_to_process: &[String],
	alpm: &dyn AlpmWrapper,
) -> Result<RecursiveInfo> {
	let raur_handle = Handle::default();
	let mut queue: Vec<String> = Vec::from(root_packages_to_process);
	let mut depth_map = IndexMap::new();
	for pkg in &queue {
		depth_map.insert(pkg.to_string(), 0);
	}
	let mut pacman_deps: IndexSet<String> = IndexSet::new();
	let mut info_map: IndexMap<String, Package> = IndexMap::new();
	while !queue.is_empty() {
		let split_at = queue.len().max(BATCH_SIZE) - BATCH_SIZE;
		let to_process = queue.split_off(split_at);
		trace!("to_process: {:?}", to_process);
		for info in raur_handle.info(&to_process)? {
			let make_deps = info.make_depends.iter();
			let check_deps = info.check_depends.iter();
			let flat_deps = info.depends.iter();
			let deps = make_deps
				.chain(flat_deps)
				.chain(check_deps)
				.map(|d| 
					clean_and_check_package_name(d)
						.expect(
							&format!("Failed to clean and check package: {}", d)
						))
				.collect_vec();

			for dependency in deps.into_iter() {
				if alpm.is_installed(&dependency)? {
					// skip if already installed
				} else if !alpm.is_installable(&dependency)? {
					if !depth_map.contains_key(&dependency) {
						eprintln!(
							"Package {} depends on {}. Resolving...",
							info.name, dependency
						);
						queue.push(dependency.to_string())
					} else {
						eprintln!("Skipping already resolved dependency {}", dependency);
					}
					let parent_depth = depth_map
						.get(&info.name)
						.context("Internal error: queue element does not have depth")?;
					let new_depth = depth_map
						.get(&dependency)
						.map_or(parent_depth + 1, |d| (*d).max(parent_depth + 1));
					depth_map.insert(dependency.to_string(), new_depth);
				} else {
					pacman_deps.insert(dependency.to_owned());
				}
			}
			info_map.insert(info.name.to_string(), info);
		}
	}
	Ok((info_map, pacman_deps, depth_map))
}

/// Queries the AUR for the provided given package names and returns a map of all packages
/// that match.
///
/// # Arguments
/// * `packages_to_query` - A slice of package names to find in the AUR
pub fn info_map<S: AsRef<str>>(packages_to_query: &[S]) -> Result<IndexMap<String, Package>> {
	let raur_handle = Handle::new();
	let mut result = IndexMap::new();
	for group in packages_to_query.chunks(BATCH_SIZE) {
		let group_info = raur_handle.info(group)?;
		for pkg in group_info.into_iter() {
			result.insert(pkg.name.to_string(), pkg);
		}
	}
	Ok(result)
}

fn clean_and_check_package_name(name: &str) -> Result<String> {
	let value = clean_package_name(name) 
		.with_context(|| format!("Unexpected package name {}", name))?;
	Ok(value)
}

fn clean_package_name(name: &str) -> Option<String> {
	let cleanup: LazyLock<Regex> = LazyLock::new(|| 
		Regex::new(r"(=.*|>.*|<.*)")
			.expect(&format!(
				"{}:{} Failed to parse regexp",
				file!(),
				line!()
			))
	);
	let name: String = cleanup.replace_all(name, "").to_string();
	let name_regex: LazyLock<Regex> = LazyLock::new(||
		Regex::new(r"^[a-zA-Z0-9@_+][a-zA-Z0-9@_+.-]*$")
			.expect(&format!(
				"{}:{} Failed to parse regexp",
				file!(),
				line!()
			))
	);
	if name_regex.is_match(&name) {
		Some(name)
	} else {
		None
	}
}

#[cfg(test)]
mod tests {
	use crate::aur_rpc_utils::*;

	#[test]
	fn test_clean_package_name() {
		assert_eq!(clean_package_name("test"), Some("test".to_string()));
		assert_eq!(
			clean_package_name("abcdefghijklmnopqrstuvwxyz0123456789@_+.-"),
			Some("abcdefghijklmnopqrstuvwxyz0123456789@_+.-".to_string())
		);

		assert_eq!(clean_package_name(""), None);
		assert_eq!(clean_package_name("-test"), None);
		assert_eq!(clean_package_name(".test"), None);
		assert_eq!(clean_package_name("!"), None);
		assert_eq!(clean_package_name("german_ö"), None);

		assert_eq!(clean_package_name("@"), Some("@".to_string()));
		assert_eq!(clean_package_name("_"), Some("_".to_string()));
		assert_eq!(clean_package_name("+"), Some("+".to_string()));

		assert_eq!(clean_package_name("test>=0"), Some("test".to_string()));
		assert_eq!(clean_package_name("test>0"), Some("test".to_string()));
		assert_eq!(clean_package_name("test<0"), Some("test".to_string()));
		assert_eq!(clean_package_name("test<=0"), Some("test".to_string()));
		assert_eq!(clean_package_name("test=0"), Some("test".to_string()));
		assert_eq!(clean_package_name("test==0"), Some("test".to_string()));
	}
}
