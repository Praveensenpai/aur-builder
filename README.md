# aur-builder

Build AUR packages on GitHub Actions' 4-core runners and install the result on your Arch box. Nothing heavy touches local CPU; the built `.pkg.tar.zst` is pulled back and installed with `pacman -U`.

Works because a **public** repo gets unlimited Actions minutes and free artifact storage.

## Setup (once)

Everything lives in this repo — no other files needed.

```bash
git clone https://github.com/Praveensenpai/aur-builder
install -Dm755 aur-builder/abuild ~/.local/bin/abuild
echo 'export AUR_BUILDER_REPO=Praveensenpai/aur-builder' >> ~/.bashrc
source ~/.bashrc
```

`~/.local/bin` is already on `PATH` on a standard Omarchy/Arch setup; check with `command -v abuild` after installing.

## Usage

```bash
abuild zen-browser-bin            # build + install
abuild parui some-aur-dep        # extra args = AUR deps built in the same run
AUR_BUILDER_NOINSTALL=1 abuild x  # download only, install later
```

Artifacts land in `/tmp/abuild/<pkgname>/` (override with `AUR_BUILDER_DL`).

## How it works

1. `abuild` fires `workflow_dispatch` on this repo with the package name.
2. Runner starts `archlinux:base-devel` in Docker.
3. Inside: `pacman -Syu`, then bootstrap `paru-bin`.
4. `paru -S --build --needed <pkg>` — builds the package **and any AUR deps** recursively, installing into the container only.
5. Built `.pkg.tar.zst` files are copied out as a workflow artifact.
6. `abuild` waits for the run, downloads the artifact, runs `sudo pacman -U`.

Local CPU usage: zero compilation, just the install.

## Caches that make repeat builds fast

| Path | Why |
|---|---|
| `.cache/pacman` | repo packages, avoids re-downloading |
| `.cache/paru` | cloned PKGBUILDs + downloaded sources |
| `.cache/cargo` | Rust registry for `-git`/`-rs` crates |
| `.cache/ccache` | C/C++ object reuse |

`restore-keys` prefix-matches, so the first build of a package is cold and later ones are warm.

## Timing reference

Measured locally on a 4-thread i3-1115G4, cold build of a 141-dependency Rust project: **2 min 42 s**, `target/` 184 MB, cargo registry 91 MB. The Actions runner has 4 dedicated vCPUs, so expect similar or better for Rust packages, and a few minutes for C/C++ projects that take ~20 min on older hardware.

## Caveats

- **glibc/pacman version skew.** The container is `archlinux:base-devel` (latest snapshot); if your local system is behind, a built package may want newer libs and `pacman -U` will refuse. Fix: `sudo pacman -Syu` locally first.
- **Signing.** Packages are unsigned, hence `pacman -U` rather than `-S`. If your `SigLevel` is strict you'll need a signing key; the default Arch config accepts local `-U` installs.
- **Not a full `yay` replacement.** `abuild` takes package names, not `-Syu` semantics. Keep `yay` for routine updates; use `abuild` for the handful of packages that compile for 20 minutes.
- **Big packages.** `timeout-minutes: 300`; artifacts are kept 3 days to stay within free storage.
- **Version pinning.** `pacman -Syu` inside the container means builds always target latest Arch. Do not run `abuild` on a system that hasn't been updated in weeks.
