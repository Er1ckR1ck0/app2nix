# app2nix

`app2nix` is a tool designed to simplify the process of packaging AppImage applications for Nix/NixOS. It automates the generation of Nix expressions, making it easier to integrate AppImages into your Nix environment.

## Description

This project provides a script that takes an AppImage file as input and generates a corresponding Nix derivation. It handles the extraction of metadata, such as the application name and version, and creates a wrapper to run the AppImage within the Nix ecosystem. This is particularly useful for users who want to run software distributed as AppImages on NixOS without manually writing complex Nix expressions.

## Usage

To use `app2nix`, run the script with the path to your AppImage file:

```bash
./app2nix <path-to-AppImage>
```

### Example

```bash
./app2nix my-application-x86_64.AppImage
```

This will generate a `default.nix` (or similar output) that you can use to build and install the application using `nix-build` or `nix-env`.

## Features

*   **Automatic Metadata Extraction:** Reads necessary information directly from the AppImage.
*   **Nix Expression Generation:** Creates ready-to-use Nix files.
*   **Sandboxing Support:** (If applicable) Configures the environment to run the AppImage securely.

## Requirements

*   Nix package manager installed.
*   `appimage-run` (usually required to