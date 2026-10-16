#!/bin/sh
# Builds a PKGBUILD (default: ./PKGBUILD) in a clean Arch container with the multilib
# repository on, where umu-launcher lives. Run from the repository root; needs Docker.
set -eu
PKGBUILD=${1:-PKGBUILD}
docker run --rm -v "$PWD/$PKGBUILD:/PKGBUILD:ro" archlinux:base-devel bash -euc '
  printf "\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n" >> /etc/pacman.conf
  pacman -Syu --noconfirm --needed sudo >/dev/null
  useradd -m builder
  echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder
  install -d -o builder /home/builder/pkg
  install -o builder /PKGBUILD /home/builder/pkg/PKGBUILD
  su builder -c "cd ~/pkg && makepkg --syncdeps --noconfirm --nocheck"
'
