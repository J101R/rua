fn main() {
	// Check that a single TLS feature has been used
	#[cfg(all(feature = "rustls-tls", feature = "native-tls"))]
	compile_error!("You must select either the `rustls-tls` or the `native-tls` feature.");

	#[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
	compile_error!("You must select either the `rustls-tls` or the `native-tls` feature.");

	// generate the shell completions
	shell_completions::generate();

	// generate seccomp.bpf for the target architecture
	seccomp::generate();
}

mod shell_completions {
	use clap::CommandFactory;
	use clap_complete::{Shell, generate_to};

	include!("src/cli_args.rs");

	pub fn generate() {
		println!("cargo:rerun-if-env-changed=COMPLETIONS_DIR");
		let directory = match std::env::var_os("COMPLETIONS_DIR") {
			None => return,
			Some(out_dir) => out_dir,
		};
		let mut cmd = CliArgs::command();
		for shell in [Shell::Bash, Shell::Fish, Shell::Zsh] {
			generate_to(shell, &mut cmd, env!("CARGO_PKG_NAME"), &directory)
				.unwrap();
		}
	}
}

mod seccomp {
	use std::fs::File;
	use std::path::Path;
	use libseccomp::{ScmpAction, ScmpArch, ScmpFilterContext, ScmpSyscall};

	const SYSCALL_BLACKLIST: &[&str] = &[
		"_sysctl",
		"acct",
		"add_key",
		"adjtimex",
		"chroot",
		"clock_adjtime",
		"create_module",
		"delete_module",
		"fanotify_init",
		"finit_module",
		"get_kernel_syms",
		"get_mempolicy",
		"init_module",
		"io_cancel",
		"io_destroy",
		"io_getevents",
		"io_setup",
		"io_submit",
		"ioperm",
		"iopl",
		"ioprio_set",
		"kcmp",
		"kexec_file_load",
		"kexec_load",
		"keyctl",
		"lookup_dcookie",
		"mbind",
		"migrate_pages",
		"modify_ldt",
		"mount",
		"move_pages",
		"name_to_handle_at",
		"nfsservctl",
		"open_by_handle_at",
		"perf_event_open",
		"pivot_root",
		"process_vm_readv",
		"process_vm_writev",
		"ptrace",
		"reboot",
		"remap_file_pages",
		"request_key",
		"set_mempolicy",
		"swapoff",
		"swapon",
		"sysfs",
		"syslog",
		"tuxcall",
		"umount2",
		"uselib",
		"vmsplice",
	];

	pub fn generate() {
		let mut ctx = ScmpFilterContext::new(ScmpAction::Allow)
			.expect("Failed to create a seccomp filter context.");

		// Restricts the filter to the native architecture only.
    	// This prevents the same filter from being bypassed on e.g. x86_64 vs. i386.
		ctx.add_arch(ScmpArch::Native)
			.expect("Failed to add systemcalls from native architecture to seccomp filter context.");

		// Deny these syscalls
		for name in SYSCALL_BLACKLIST {
			// Resolve the syscall number; if the name isn’t known on this arch,
        	// we panic and fail the build (e.g., some syscalls are only on older kernels).
			match ScmpSyscall::from_name(name) {
				Ok(syscall) => {
					// Add rule to filter. The syscall number will later be translated for all enabled architectures in the filter.
					ctx.add_rule(ScmpAction::KillThread, syscall)
					.unwrap_or_else(|e| {
						panic!(
							"Failed to compile seccomp filter, failed to add rule for syscall {}({}). Error: {}",
							name, syscall, e
						);
					});
				},
				Err(_) => {
					panic!(
						"Failed to compile seccomp filter, syscall {} could not be resolved.",
						name
					)
				},
			}
		}

		println!("cargo:rerun-if-env-changed=OUT_DIR");
		let out_dir = std::env::var("OUT_DIR")
			.expect("Failed to save generated seccomp filter, no compile-time OUT_DIR defined.");
		// Export the bpf file that will be "inlined" at RUA build time
		let fd = File::create(Path::new(&out_dir).join("seccomp.bpf"))
			.expect("Cannot create file seccomp.bpf in OUT_DIR.");
		ctx.export_bpf(fd)
			.expect("Failed to export seccomp.bpf.");

		// Export the pfc file for debugging (not used for the actual build)
		let fd = File::create(Path::new(&out_dir).join("seccomp.pfc"))
			.expect("Cannot create file seccomp.pfc in OUT_DIR.");
		ctx.export_pfc(fd)
			.expect("Failed to export seccomp.pfc.");
	}
}
