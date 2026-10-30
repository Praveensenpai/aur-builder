# ⚡ abuild

> **Compile AUR packages on GitHub's 4-core runners. Install the finished binary on your Arch box. Your CPU stays cold.**

[![Actions](https://img.shields.io/badge/Actions-unlimited%20on%20public%20repos-a6e3a1?style=flat-square&logo=githubactions&logoColor=white)](https://github.com/Praveensenpai/aur-builder/actions)
[![Platform](https://img.shields.io/badge/Platform-Arch%20Linux-1793D1?style=flat-square&logo=archlinux&logoColor=white)](https://archlinux.org)
[![Shell](https://img.shields.io/badge/Shell-bash-4EAA25?style=flat-square&logo=gnubash&logoColor=white)](https://www.gnu.org/software/bash/)
[![Cost](https://img.shields.io/badge/Cost-%240.00-cba6f7?style=flat-square)](https://github.com/Praveensenpai/aur-builder/actions)

Some AUR packages take 20 minutes to compile. This tool moves that compile to a disposable GitHub Actions container, then hands you back a `.pkg.tar.zst` ready for `pacman -U`.

No Codespaces quota. No storage bills. No daemon on your machine.

> [!TIP]
> **Free forever on a public repo** — GitHub gives unlimited Actions minutes and free artifact storage to public repositories. A 20-minute build costs you nothing but 20 minutes of *their* CPU.

---

## 🔄 How it works

```text
┌─────────────────────────────┐
│  💻 Your Arch box           │
│  $ abuild zen-browser-bin   │
└──────────────┬──────────────┘
               │  1. workflow_dispatch (gh CLI)
               ▼
┌─────────────────────────────┐
│  ☁️ GitHub Actions runner   │
│  ubuntu-latest · 4 vCPU     │
└──────────────┬──────────────┘
               │  2. docker run archlinux:base-devel
               ▼
┌──────────────────────────────────────────┐
│  🏗️ Build container (throwaway)          │
│  pacman -Syu → paru (from source)        │
│  builds the package AND its AUR deps     │
└──────────────┬───────────────────────────┘
               │  3. artifact upload (.pkg.tar.zst)
               ▼
┌─────────────────────────────┐
│  📦 /tmp/abuild/<pkgname>/  │
│  4. sudo pacman -U  → done  │
└─────────────────────────────┘
```

You never install `paru`, `base-devel`, or a chroot locally. The container is destroyed when the job ends.

### Repo layout

| File | Runs on | Role |
|---|---|---|
| `abuild` | your machine | Dispatches the workflow, downloads the artifact, runs `pacman -U` |
| `.github/workflows/build.yml` | Actions runner | Spins up the container, uploads artifacts |
| `ci/build.sh` | inside the container | Bootstraps `paru`, validates the package name, builds |

Keeping the build logic in `ci/build.sh` means it runs as a normal script instead of a string nested inside YAML — easier to read, lint (`bash -n`), and modify.

---

## 🍴 Step 0 — Fork this repo (required)

**You must fork before anything else.** `abuild` dispatches a workflow on *your* GitHub account; it cannot use someone else's repo.

```bash
gh repo fork Praveensenpai/aur-builder --clone --remote
```

Or click **Fork** at the top-right of this page.

> [!IMPORTANT]
> **Fork, don't clone.** A plain `git clone` still points `origin` at `Praveensenpai/aur-builder`, so `gh workflow run` would target my repo and fail. The command above sets your fork as `origin` and keeps mine as `upstream`.
>
> Your fork must stay **public** — that's what makes Actions minutes and artifact storage free. A private fork burns your 2,000 monthly minutes.

---

## 🚀 Quick start

```bash
# 1. fork + clone in one step (see Step 0 above)
gh repo fork Praveensenpai/aur-builder --clone --remote
cd aur-builder

# 2. install the wrapper
install -Dm755 abuild ~/.local/bin/abuild

# 3. verify — resolves to <your-login>/aur-builder automatically
command -v abuild

# 4. build something that actually compiles
abuild zen-browser-bin
```

`abuild` derives your repo from `gh api user`, so there is **no config file and no env var to set** as long as your fork is named `aur-builder`.

> [!NOTE]
> `~/.local/bin` is on `PATH` by default on Arch/Omarchy. If `command -v abuild` prints nothing, add `export PATH="$HOME/.local/bin:$PATH"` to your shell rc.

---

## 📖 Usage

```bash
abuild <pkgname> [aur-dep ...]
```

| Command | What happens |
|---|---|
| `abuild zen-browser-bin` | Build + install one package |
| `abuild some-tool libfoo-git` | Build `some-tool`, pulling in `libfoo-git` from the AUR in the same run |
| `AUR_BUILDER_NOINSTALL=1 abuild foo` | Download only — prints the `pacman -U` command |
| `abuild foo \| tee build.log` | Safe to background: `Ctrl-C` does **not** stop the remote build |

Artifacts land in `/tmp/abuild/<pkgname>/`.

### Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `AUR_BUILDER_REPO` | `<your-login>/aur-builder` | Override the repo (different fork name) |
| `AUR_BUILDER_WORKFLOW` | `build.yml` | Alternate workflow file |
| `AUR_BUILDER_DL` | `/tmp/abuild` | Where artifacts are downloaded |
| `AUR_BUILDER_NOINSTALL` | `0` | `1` = skip `pacman -U` |

---

## ⚡ What makes it fast

Five caches survive between runs via `actions/cache`:

| Cache | Saved |
|---|---|
| `.cache/pacman` | Downloaded repo packages |
| `.cache/paru` | Cloned PKGBUILDs and source tarballs |
| `.cache/paru-pkg` | The compiled `paru` package itself (see caveat below) |
| `.cache/cargo` | Rust registry for `-git` / `-rs` packages |
| `.cache/ccache` | Compiled C/C++ objects |

First build of a package is cold; every later build is warm. `restore-keys` prefix-matches, so `abuild foo` benefits from `aur-` caches even before it has its own.

Measured reference — cold build of a 141-dependency Rust project on a 4-thread i3-1115G4: **2 min 42 s**, `target/` 184 MB. The runner has 4 dedicated vCPUs and no thermal throttling.

---

## ⚠️ Caveats

> [!WARNING]
> **Version skew will bite you.** The container runs `archlinux:base-devel` at the latest snapshot. If your local system is weeks behind, the built package may demand a newer `glibc` and `pacman -U` will refuse it. Run `sudo pacman -Syu` before `abuild`.

- **AUR packages only.** Names that exist in `core`/`extra`/`multilib` are rejected — paru would install a prebuilt binary and produce no artifact. Use `sudo pacman -S` for those.
- **Unsigned packages.** Hence `pacman -U`, not `pacman -S`. The default Arch `SigLevel` accepts local installs.
- **Not a `yay` replacement.** `abuild` takes package names, not `-Syu` semantics. Keep `yay` for routine updates and use `abuild` for the handful of packages that compile for 20 minutes.
- **Job timeout is 5 hours.** Artifacts are kept 3 days to stay inside free storage.
- **Public repo required** for the free tier. A private repo burns your 2,000 monthly Actions minutes — and a single large package can eat a chunk of that.

---

## 🔧 Workflow inputs

Both inputs are optional when using `abuild`; they matter if you trigger the workflow by hand:

| Input | Required | Meaning |
|---|---|---|
| `pkgname` | yes | AUR package to build |
| `deps` | no | Space-separated extra AUR packages built in the same run |

You can also trigger it from the web UI: **Actions → build-aur → Run workflow**.

> [!NOTE]
> The workflow **rejects packages that exist in the official repos** (`core`/`extra`/`multilib`). Those are already compiled — install them with `sudo pacman -S`. Only packages that must be built from source belong here.

---

## 📜 License

MIT — do whatever you want.
© Praveen Senpai ([@Praveensenpai](https://github.com/Praveensenpai))
