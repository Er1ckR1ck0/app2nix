use crate::structs::{PackageType, PackageInfo};

pub fn generate_nix_content(
    pkg_type: &PackageType,
    pkg_info: &PackageInfo,
    url: &str,
    sha256: &str,
    _mode_upstream: bool
) -> String {
    let clean_pkg_path = |p: &str| {
        let prefix = "legacyPackages.x86_64-linux.";
        if let Some(stripped) = p.strip_prefix(prefix) {
            stripped.to_string()
        } else {
            p.to_string()
        }
    };

    let deps_list: Vec<String> = pkg_info.deps.iter().map(|p| clean_pkg_path(p)).collect();

    // Standard build dependencies
    let build_deps = vec![
        "alsa-lib",
        "at-spi2-core",
        "cairo",
        "cups",
        "dbus",
        "expat",
        "glib",
        "glibc",
        "gtk3",
        "libdrm",
        "libnotify",
        "libsecret",
        "libxkbcommon",
        "mesa",
        "nspr",
        "nss",
        "pango",
        "systemd",
        "xorg.libX11",
        "xorg.libXcomposite",
        "xorg.libXdamage",
        "xorg.libXext",
        "xorg.libXfixes",
        "xorg.libXrandr",
        "xorg.libxcb",
    ];

    // Library path packages for wrapProgram
    let lib_path_packages = vec![
        "libglvnd",
        "mesa",
        "libdrm",
        "vulkan-loader",
        "libxkbcommon",
        "gtk3",
        "alsa-lib",
        "nss",
        "nspr",
        "expat",
        "dbus",
        "at-spi2-core",
        "pango",
        "cairo",
        "libsecret",
        "libnotify",
        "systemd",
    ];

    // Combine resolved deps with standard build deps
    let mut all_build_deps: Vec<String> = build_deps.iter().map(|s| s.to_string()).collect();
    for dep in &deps_list {
        let clean_dep = dep.split('.').last().unwrap_or(dep);
        if !all_build_deps.contains(&clean_dep.to_string()) {
            all_build_deps.push(clean_dep.to_string());
        }
    }
    all_build_deps.sort();
    all_build_deps.dedup();

    // Format buildInputs with pkgs. prefix
    let packages_string = all_build_deps
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if p.contains('.') {
                format!("    pkgs.{}", p)
            } else if i == 0 {
                format!("    pkgs.{} # Accessed via pkgs, so hyphens are fine", p)
            } else {
                format!("    pkgs.{}", p)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Format lib packages with pkgs. prefix and proper indentation
    let lib_packages_string = lib_path_packages
        .iter()
        .map(|p| format!("            pkgs.{}", p))
        .collect::<Vec<_>>()
        .join("\n");

    let header = "{ pkgs ? import <nixpkgs> {} }:";

    match pkg_type {
        PackageType::Deb => {
            let template = include_str!("../templates/deb.in");
            let content = template
                .replace("{header}", header)
                .replace("{name}", &pkg_info.name)
                .replace("{version}", &pkg_info.version)
                .replace("{url}", url)
                .replace("{sha256}", sha256)
                .replace("{packages}", &packages_string)
                .replace("{lib_packages}", &lib_packages_string)
                .replace("{description}", &pkg_info.description)
                .replace("{arch}", &pkg_info.arch);
            content
        }
    }
}
