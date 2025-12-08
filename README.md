# app2nix

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Nix](https://img.shields.io/badge/ecosystem-Nix-blue.svg)](https://nixos.org/)

**app2nix** is a CLI tool that automates the packaging of Debian (`.deb`) applications for Nix/NixOS.

It analyzes the `.deb` file, extracts dependencies, resolves them against `nixpkgs` using smart heuristics (handling library naming differences between Debian and Nix), and generates a ready-to-use `default.nix`.

## 🚀 Features

* **Automatic Dependency Resolution**: Parses `Depends` fields from the `.deb` file and finds the corresponding packages in `nixpkgs` (e.g., maps `libgtk-3-0` to `pkgs.gtk3`).
* **Smart Heuristics**: Uses fuzzy matching and JSON parsing to handle naming discrepancies (e.g., finding `xorg.libX11` when Debian asks for `libx11-6`).
* **Standalone Generation**: Produces a `default.nix` that wraps the binary using `autoPatchelfHook` and `makeWrapper` for immediate usage.
* **Metadata Extraction**: Automatically pulls version, description, and architecture from the package control file.

## 📦 Installation

### From Source

Requirements:
* Rust (cargo)
* Nix (with `nix-command` and `flakes` enabled)

```bash
git clone [https://github.com/yourusername/app2nix.git](https://github.com/yourusername/app2nix.git)
cd app2nix
cargo build --release
./target/release/app2nix [url]
nix-env -if default.nix
```
