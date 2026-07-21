## RUA

RUA is a build tool for Arch Linux AUR packages.

This fork keeps the original workflow, while updating dependencies, removing unmaintained crates, and hardening the security wrapper.

## What’s changed in this fork

- Updated dependencies
- Removed unmaintained crates
- Hardened the security wrapper
- Switched seccomp to use `KillProcess` instead of `killthread`
- Updated `tar` to address [RUSTSEC-2026-0068](https://rustsec.org/advisories/RUSTSEC-2026-0068)

## Features

- Allows local patch application
- Provides detailed information:
  - show upstream changes upon package upgrade
  - check PKGBUILDs with `shellcheck`, handling PKGBUILD-specific variables
  - warn if SUID files are present in an already built package, and show them
  - show the file list, executable list, and INSTALL script in already built packages
- Minimizes user distractions:
  - verify build scripts once, then build without interruptions
  - group built packages for batch review
- Uses a security namespace jail:
  - supports `--offline` builds
  - builds in an isolated filesystem; see [Safety](#Safety)
  - uses `seccomp` to limit available syscalls (e.g. the build cannot call `ptrace`)
  - prevents builds from executing `sudo` by mounting the filesystem with `nosuid`
- Written in Rust

## Use

```sh
rua search wesnoth
rua info freecad
rua install pinta      # install or upgrade a package
rua upgrade            # upgrade all AUR packages
rua upgrade brave-bin  # upgrade specific AUR package
```

By default, `rua upgrade` upgrades all installed AUR packages.

If needed, you can specify package names to upgrade only selected packages:

```sh
rua upgrade A B C
```

Alternatively:

```sh
rua install A B C
```

Packages can be ignored using the `--ignore` flag or by adding them to
`IgnorePkg` in `pacman.conf`. This uses the same mechanism as ignoring
non-AUR packages in `pacman`.

```sh
rua shellcheck path/to/my/PKGBUILD
```

run `shellcheck` on a PKGBUILD, discovering potential problems with the build instruction. Takes care of PKGBUILD-specific variables.

```sh
rua tarcheck xcalib.pkg.tar
```

if you already have a *.pkg.tar package built, run RUA checks on it (SUID, executable list, INSTALL script review etc).

```sh
rua builddir --offline /path/to/pkgbuild/directory
```

Build a directory in offline mode.

```sh
rua --help
rua subcommand --help
```

Show CLI help.

## Install dependencies

```sh
sudo pacman -S --needed --asdeps git base-devel bubblewrap-suid libseccomp xz shellcheck cargo
```


## Install (the AUR way)

```sh
sudo pacman -S --needed base-devel git
git clone https://aur.archlinux.org/rua.git
cd rua
makepkg -si
```

In the web interface, the package is [rua](https://aur.archlinux.org/packages/rua-fork/).

## How it works / directories

| directory | meaning |
| --- | --- |
| `~/.config/rua/pkg/` | Step 1: AUR packages are cloned here. You review and make local modifications here. |
| `~/.cache/rua/build/` | Step 2: reviewed packages are copied here, and then built |
| `~/.local/share/rua/checked_tars/` | Step 3: built and tarchecked packages are stored here (`*.pkg.tar.xz`). |
| `~/.config/rua/wrap_args.d/` | Entry point for basic configuration of the security wrapper script. |
| `~/.config/rua/.system/` | Internal files. |
| `$GNUPGHOME/pubring.kbx` <br/> `$GNUPGHOME/pubring.gpg` | Read-only access is granted during builds to allow signature verification. |
| All other files | All other files in `~` are not accessed by RUA and inaccessible by built packages; see [Safety](#Safety). |

Note that directories above follow the XDG specification, so:

- `XDG_CONFIG_HOME` overrides `~/.config`
- `XDG_CACHE_HOME` overrides `~/.cache`
- `XDG_DATA_HOME` overrides `~/.local/share`

## How it works / reviewing

Knowing the underlying machinery is not required to work with RUA,
but if you're curious anyway, this section is for you.

All AUR packages are stored in designated `git` repositories,
with `upstream/master` pointing to remote AUR head and
local `master` meaning your reviewed and accepted state.
Local branch does not track the remote one.

RUA works by fetching remote updates when needed,
presenting remote changes to you and merging them if you accept them.
Merging and basic diff view are built-in commands in RUA, and you can
drop to shell and do more from git CLI if you want.


## How it works / dependency grouping and installation

RUA will:

1. Fetch the AUR package and all recursive dependencies.
2. Prepare a summary of all pacman and AUR packages that will need installing.
3. Show the summary and ask for confirmation.
4. Review all AUR dependencies, ensuring the user accepts recursive changes.
5. Propose installing all pacman dependencies.
6. Build all AUR packages at the maximum dependency depth.
7. Let the user review built artifacts in batch.
8. Install them.
9. Repeat the process for the next dependency layer.

If you have a dependency structure like this:

```text
your_original_package
├── dependency_a
│   ├── a1
│   └── a2
└── dependency_b
    ├── b1
    └── b2
```

RUA will thus interrupt you 3 times, not 7 as if it would be plainly recursive. It also avoids unnecessary disruption when it knows recursion will fail later because of unsatisfiable dependencies.


## Limitations

- This tool focuses on AUR packages only; it cannot `-Suy` your system. Use `pacman` for that.
- Optional dependencies (optdepends) are not installed. They are skipped, so check them manually when reviewing the PKGBUILD.
- Version handling is not implemented. RUA always installs the latest available version and assumes it is sufficient.
- Development packages such as `-git` packages are only rebuilt when running `rua upgrade --devel`. No version checks are done to avoid unnecessary rebuilds. Merge requests welcomed.
- Unless explicitly enabled, builds do not share your home directory. This may cause tools like Maven, npm, or Cargo to re-download dependencies for every build. See [Safety](#Safety) for how to whitelist directories.
- `PKGDEST` and `BUILDDIR` from `makepkg.conf` are not supported. Packages are built in isolation from each other, and artifacts are stored in standard locations used by this tool.
- Due to safety restrictions, [X11 access might not work](./docs/x11access.md) during builds.
- Due to safety restrictions, [ccache usage will fail](./docs/ccache.md) during builds.
- Due to a [bug in fakeroot](https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=909727), creation of root-owned packages inside PKGBUILD-s `package()` does not work. This happens when archives are extracted in `package()` function. Doing it in `prepare()` or giving a key like `tar --no-same-owner` is the work-around.


## Safety

Do not install AUR packages you don't trust. RUA only adds build-time isolation and install-time control/review.

When building packages, RUA uses the following filesystem isolation:

- The build directory is mounted read-write.
- `$GNUPGHOME/pubring.kbx` and `$GNUPGHOME/pubring.gpg` are mounted read-only, if present, so signature verification works.
- The rest of `~` is hidden from the build process and mounted under `tmpfs`.
- `/tmp`, `/dev`, and `/proc` are remounted with empty `tmpfs`, `devtmpfs`, and `procfs` respectively.
- The rest of `/` is mounted read-only.
- You can whitelist or add mount points by configuring `wrap_args`; see `~/.config/rua/.system/wrap_args.sh.example`.

In addition, all builds run in a namespace jail with `seccomp` enabled. The `user`, `ipc`, `pid`, `uts`, and `cgroup` namespaces are unshared by default.

If requested from the CLI, builds can be run in offline mode.


## Other

The RUA name is an inversion of "AUR".

This project builds on excellent libraries such as:

- [raur](https://gitlab.com/davidbittner/raur),
- [srcinfo](https://github.com/Morganamilo/srcinfo.rs)
- and many others.

This project is licensed under GPLv3+.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project (rua) by you,
shall be licensed as GPLv3+, without any additional terms or conditions.

For authors, see [Cargo.toml](Cargo.toml) and git history.
