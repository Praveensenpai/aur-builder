#!/usr/bin/env bash
# Runs INSIDE the archlinux:base-devel container, as root.
# Env: PKGNAME (required), EXTRADEPS (optional), HOST_UID, HOST_GID
#
# Living in its own file means no nested quoting inside a YAML block scalar —
# which is exactly what broke the earlier inline version.
set -euo pipefail
shopt -s nullglob

: "${PKGNAME:?PKGNAME is required}"
EXTRADEPS="${EXTRADEPS:-}"
HOST_UID="${HOST_UID:-0}"
HOST_GID="${HOST_GID:-0}"

# Snapshot image ships without a db; a full upgrade avoids partial-upgrade
# breakage when makepkg --syncdeps pulls dependencies.
pacman -Syu --noconfirm --needed sudo git ccache rust &>/dev/null

# makepkg refuses to run as root.
id builder &>/dev/null || useradd -m builder
echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder
chmod 0440 /etc/sudoers.d/builder
mkdir -p /tmp/pkgs /tmp/makepkg
chown -R builder:builder /home/builder /parucache /tmp/pkgs /tmp/makepkg

# ---------------------------------------------------------------- paru ------
# paru-bin is prebuilt against an older libalpm and dies with
# "libalpm.so.15: cannot open shared object file" on a current container,
# so build paru from source and cache the resulting package.
if ! command -v paru &>/dev/null; then
  cached=(/parucache/paru-[0-9]*.pkg.tar.zst)
  if (( ${#cached[@]} )); then
    echo "==> installing cached paru package"
    pacman -U --noconfirm "${cached[0]}"
  else
    echo "==> building paru from source"
    su builder -c 'cd /tmp && rm -rf paru && git clone --depth=1 https://aur.archlinux.org/paru.git && cd paru && makepkg -sf --noconfirm'
    mapfile -t ppkgs < <(find /tmp/paru -maxdepth 1 -name 'paru-[0-9]*.pkg.tar.zst' -type f)
    (( ${#ppkgs[@]} )) || { echo "error: paru build produced no package" >&2; exit 1; }
    pacman -U --noconfirm "${ppkgs[0]}"
    cp -v "${ppkgs[0]}" /parucache/
    chown "${HOST_UID}:${HOST_GID}" /parucache/*.pkg.tar.zst
  fi
fi
paru --version

# ------------------------------------------------------------ makepkg conf --
{
  echo "MAKEFLAGS=\"-j$(nproc)\""
  echo "BUILDDIR=/tmp/makepkg"
  echo "PKGDEST=/tmp/pkgs"
  echo "CCACHE_DIR=/home/builder/.ccache"
} > /home/builder/.makepkg.conf
chown builder:builder /home/builder/.makepkg.conf

# ------------------------------------------------------------- validation ---
# Repo packages are already compiled: paru would install a binary and build
# nothing, leaving zero artifacts. Reject them with a useful message.
if pacman -Si "$PKGNAME" &>/dev/null; then
  echo "error: $PKGNAME is an official repo package, not an AUR package." >&2
  echo "       install it with: sudo pacman -S $PKGNAME" >&2
  exit 1
fi

# Same idea for typos: confirm the name actually exists in the AUR.
info="$(curl -sf --max-time 30 "https://aur.archlinux.org/rpc/v5/info?arg[]=${PKGNAME}" || true)"
case "$info" in
  *'"resultcount":0'*|'')
    echo "error: $PKGNAME was not found in the AUR." >&2
    exit 1
    ;;
esac

# ------------------------------------------------------------------ build ---
# --aur: never fall back to repo packages, always build from source.
su builder -c "cd /tmp && paru -S --aur --noconfirm --needed ${EXTRADEPS} ${PKGNAME}"

mapfile -t pkgs < <(find /tmp/pkgs /home/builder/.cache/paru/clone \
                      -name '*.pkg.tar.*' -type f 2>/dev/null)
(( ${#pkgs[@]} )) || { echo "error: no packages produced" >&2; exit 1; }

cp -v "${pkgs[@]}" /out/
chown -R "${HOST_UID}:${HOST_GID}" /out