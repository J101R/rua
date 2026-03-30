use std::process::ExitCode;

use crate::print_package_table;
use anyhow::Context;
use anyhow::Ok;
use raur::blocking::Raur;
use raur::Package;
use raur::SearchBy;
use anyhow::Result;

fn contains_keyword(pkg: &Package, keyword: &str) -> bool {
	let filter = keyword.to_lowercase();
	pkg.name.to_lowercase().contains(filter.as_str())
		|| pkg
			.description
			.iter()
			.any(|descr| descr.to_lowercase().contains(filter.as_str()))
}

pub fn action_search(keywords: &[String]) -> Result<ExitCode> {
	let mut keywords = Vec::from(keywords);
	keywords.sort_by_key(|t| -(t.len() as i16));
	let query = keywords
		.first()
		.context("Zero search arguments, should be impossible in clap")?;
	let raur_handle = raur::blocking::Handle::new();
	let mut result = raur_handle.search_by(query, SearchBy::NameDesc)
		.context("Search error")?;
	if result.is_empty() {
		return Ok(ExitCode::FAILURE);
	};
	for keyword in &keywords[1..] {
		result.retain(|pkg| contains_keyword(pkg, keyword));
	}
	print_package_table::print_package_table(result, &keywords);
	Ok(ExitCode::SUCCESS)
}
