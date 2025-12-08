use crate::structs::{PackageType, PackageInfo};

pub fn generate_nix_content(
    pkg_type: &PackageType,
    pkg_info: &PackageInfo,
    url: &str,
    sha256: &str,
    mode_upstream: bool
) -> String {
    let sanitize_var = |s: &str| s.replace(['-', '.'], "_");

    let clean_pkg_path = |p: &str| {
        let prefix = "legacyPackages.x86_64-linux.";
        if let Some(stripped) = p.strip_prefix(prefix) {
            stripped.to_string()
        } else {
            p.to_string()
        }
    };

    let deps_list: Vec<String> = pkg_info.deps.iter().map(|p| clean_pkg_path(p)).collect();

    let packages_string = if mode_upstream {
        deps_list.iter()
            .map(|p| format!("    {}", p.split('.').last().unwrap_or(p)))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        deps_list.iter()
            .map(|p| format!("    pkgs.{}", p))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let lib_list_local = "[ \
        pkgs.systemd \
        pkgs.libglvnd \
        pkgs.mesa \
        pkgs.libdrm \
        pkgs.vulkan-loader \
        pkgs.libxkbcommon \
        pkgs.gtk3 \
        pkgs.alsa-lib \
        pkgs.nss \
        pkgs.nspr \
        pkgs.expat \
        pkgs.dbus \
        pkgs.at-spi2-core \
        pkgs.pango \
        pkgs.cairo \
        pkgs.libsecret \
        pkgs.libnotify \
    ]";
    let lib_list_upstream = "[ libglvnd mesa libdrm vulkan-loader libxkbcommon ]";

    let additional_build_inputs = vec![
        "pkgs.glibc",
        "pkgs.gcc-unwrapped.lib",
        "pkgs.autoPatchelfHook",
        "pkgs.dpkg",
        "pkgs.makeWrapper",
        
        // Графика и системные
        "pkgs.alsa-lib",
        "pkgs.libdrm",
        "pkgs.mesa",
        "pkgs.nss",
        "pkgs.nspr",
        "pkgs.systemd", // Важно!
        "pkgs.libsecret",
        "pkgs.libnotify",
        
        // X11
        "pkgs.xorg.libX11",
        "pkgs.xorg.libXcomposite",
        "pkgs.xorg.libXdamage",
        "pkgs.xorg.libXext",
        "pkgs.xorg.libXfixes",
        "pkgs.xorg.libXrandr",
        "pkgs.xorg.libxcb",
        "pkgs.libxkbcommon",
    ];

    let header = if mode_upstream {
        let inputs = vec!["lib", "stdenv", "fetchurl", "autoPatchelfHook", "dpkg", "makeWrapper"];
        
        let mut clean_deps: Vec<String> = deps_list.iter()
            .map(|p| sanitize_var(p.split('.').last().unwrap_or(p)))
            .collect();
        clean_deps.sort();
        clean_deps.dedup();

        let mut input_str = inputs.join(", ");
        if !clean_deps.is_empty() { 
            input_str.push_str(", "); 
            input_str.push_str(&clean_deps.join(", ")); 
        }
        format!("{{ {input_str} }}:")
    } else { 
        "{ pkgs ? import <nixpkgs> {} }:".to_string() 
    };

    let (stdenv, fetchurl, native, lib_path_expr) = if mode_upstream {
        (
            "stdenv.mkDerivation", 
            "fetchurl", 
            "    autoPatchelfHook\n    dpkg\n    makeWrapper", 
            format!("lib.makeLibraryPath {}", lib_list_upstream)
        )
    } else {
        (
            "pkgs.stdenv.mkDerivation", 
            "pkgs.fetchurl", 
            "pkgs.autoPatchelfHook\npkgs.dpkg\npkgs.makeWrapper", 
            format!("pkgs.lib.makeLibraryPath {}", lib_list_local)
        )
    };

    let mut all_deps = deps_list.iter()
        .map(|p| format!("    pkgs.{}", p))
        .collect::<Vec<_>>();
    all_deps.extend(additional_build_inputs.iter().map(|s| format!("    {}", s)));
    all_deps.sort();
    all_deps.dedup();
    
    let packages_string = if mode_upstream {
        let mut upstream_deps: Vec<String> = deps_list.iter()
            .map(|p| format!("    {}", p.split('.').last().unwrap_or(p)))
            .collect();
        let additional_upstream = additional_build_inputs.iter()
            .map(|s| s.replace("pkgs.", ""))
            .map(|s| format!("    {}", s.split('.').last().unwrap_or(&s)));
        upstream_deps.extend(additional_upstream);
        upstream_deps.sort();
        upstream_deps.dedup();
        upstream_deps.join("\n")
    } else {
        all_deps.join("\n")
    };

    match pkg_type {
        PackageType::Deb => {
            let template = include_str!("../templates/deb.in");
            let content = template
                .replace("{header}", &header)
                .replace("{stdenv}", stdenv)
                .replace("{{name}}", &pkg_info.name) // Экранированный плейсхолдер
                .replace("{name}", &pkg_info.name)
                .replace("{version}", &pkg_info.version)
                .replace("{fetchurl}", fetchurl)
                .replace("{url}", url)
                .replace("{sha256}", sha256)
                .replace("{native}", native)
                .replace("{packages}", &packages_string)
                .replace("{lib_path_expr}", &lib_path_expr)
                .replace("{description}", &pkg_info.description)
                .replace("{arch}", &pkg_info.arch);
            content
        }
    }
}