use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Debug, PartialEq, Clone)]
enum PackageType {
    AppImage,
    Deb,
}

struct PackageMeta {
    name: String,
    version: String,
}

fn check_common_libs(lib: &str) -> Option<&'static str> {
    match lib {
        // GTK / GUI
        "libgtk-3.so.0" | "libgdk-3.so.0" => Some("gtk3"),
        "libgtk-x11-2.0.so.0" => Some("gtk2"),
        "libglib-2.0.so.0" | "libgobject-2.0.so.0" | "libgio-2.0.so.0" => Some("glib"),
        "libpango-1.0.so.0" | "libpangocairo-1.0.so.0" | "libpangoft2-1.0.so.0" => Some("pango"),
        "libcairo.so.2" | "libcairo-gobject.so.2" => Some("cairo"),
        "libgdk_pixbuf-2.0.so.0" => Some("gdk-pixbuf"),
        "libatk-1.0.so.0" | "libatk-bridge-2.0.so.0" | "libatspi.so.0" => Some("at-spi2-atk"),

        // Qt5 / Qt6
        "libQt5Core.so.5" | "libQt5Gui.so.5" | "libQt5Widgets.so.5" => Some("qt5.qtbase"),
        "libQt5X11Extras.so.5" => Some("qt5.qtx11extras"),
        "libQt6Core.so.6" | "libQt6Gui.so.6" | "libQt6Widgets.so.6" => Some("qt6.qtbase"),

        // X11
        "libX11.so.6" | "libX11-xcb.so.1" => Some("xorg.libX11"),
        "libXcomposite.so.1" => Some("xorg.libXcomposite"),
        "libXdamage.so.1" => Some("xorg.libXdamage"),
        "libXext.so.6" => Some("xorg.libXext"),
        "libXfixes.so.3" => Some("xorg.libXfixes"),
        "libXrandr.so.2" => Some("xorg.libXrandr"),
        "libXrender.so.1" => Some("xorg.libXrender"),
        "libxcb.so.1" | "libxcb-dri3.so.0" => Some("xorg.libxcb"),
        "libxkbcommon.so.0" => Some("libxkbcommon"),
        "libxshmfence.so.1" => Some("libxshmfence"),

        // OpenGL / Graphics
        "libgbm.so.1" => Some("mesa"),
        "libdrm.so.2" => Some("libdrm"),
        "libGL.so.1" | "libEGL.so.1" | "libGLESv2.so.2" => Some("libglvnd"),
        "libvulkan.so.1" => Some("vulkan-loader"),

        // Sound / Media
        "libasound.so.2" => Some("alsa-lib"),
        "libpulse.so.0" => Some("libpulseaudio"),

        // Core / Utils
        "libnss3.so" | "libnssutil3.so" | "libsmime3.so" => Some("nss"),
        "libnspr4.so" => Some("nspr"),
        "libcups.so.2" => Some("cups"),
        "libdbus-1.so.3" => Some("dbus"),
        "libexpat.so.1" => Some("expat"),
        "libudev.so.1" => Some("systemd"),
        "libz.so.1" => Some("zlib"),
        "libc.so.6" | "libm.so.6" | "libpthread.so.0" | "libdl.so.2" => Some("glibc"),

        // libgcc correction
        "libgcc_s.so.1" | "libstdc++.so.6" => Some("libgcc"),

        _ => None,
    }
}

fn detect_file_type(path: &str) -> Result<PackageType, Box<dyn std::error::Error>> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; 8];
    file.read_exact(&mut buffer)?;
    if buffer.starts_with(b"!<arch>") { return Ok(PackageType::Deb); }
    if buffer.starts_with(b"\x7fELF") { return Ok(PackageType::AppImage); }
    Err("Unknown file type".into())
}

fn extract_metadata(filename: &str, pkg_type: &PackageType) -> PackageMeta {
    match pkg_type {
        PackageType::Deb => {
            let get_field = |field: &str| -> String {
                let out = Command::new("dpkg-deb").arg("--field").arg(filename).arg(field).output().expect("Failed to run dpkg-deb");
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            };
            let name = get_field("Package");
            let version = get_field("Version");
            if !name.is_empty() && !version.is_empty() { return PackageMeta { name: name.to_lowercase(), version }; }
        },
        PackageType::AppImage => {
            let stem = Path::new(filename).file_stem().unwrap().to_str().unwrap();
            let parts: Vec<&str> = stem.split(&['-', '_'][..]).collect();
            let mut name_parts = Vec::new();
            let mut version_part = "1.0.0".to_string();
            let mut found = false;
            for part in parts {
                if !found && part.chars().next().map_or(false, |c| c.is_numeric()) {
                    version_part = part.to_string(); found = true;
                } else if !found { name_parts.push(part); }
            }
            let name = if name_parts.is_empty() { "generated-app".to_string() } else { name_parts.join("-").to_lowercase() };
            return PackageMeta { name, version: version_part };
        }
    }
    PackageMeta { name: "generated-package".to_string(), version: "1.0.0".to_string() }
}

fn generate_nix_content(
    pkg_type: &PackageType, meta: &PackageMeta, url: &str, sha256: &str, pkgs_list: &[String], mode_upstream: bool
) -> String {
    let sanitize_var = |s: &str| s.replace(['-', '.'], "_");

    let deps_string = if mode_upstream {
        pkgs_list.iter()
            .map(|p| sanitize_var(p.split('.').last().unwrap()))
            .collect::<Vec<_>>()
            .join("\n    ")
    } else {
        pkgs_list.iter().map(|p| format!("    pkgs.{}", p)).collect::<Vec<_>>().join("\n")
    };

    let build_inputs_body = if mode_upstream {
        pkgs_list.iter().map(|p| format!("    {}", sanitize_var(p.split('.').last().unwrap()))).collect::<Vec<_>>().join("\n")
    } else {
        pkgs_list.iter().map(|p| format!("    pkgs.{}", p)).collect::<Vec<_>>().join("\n")
    };

    let lib_list_local = "[ pkgs.libglvnd pkgs.mesa pkgs.libdrm pkgs.vulkan-loader pkgs.libxkbcommon ]";
    let lib_list_upstream = "[ libglvnd mesa libdrm vulkan-loader libxkbcommon ]";

    let header = if mode_upstream {
        let inputs = vec!["lib", "stdenv", "fetchurl", "autoPatchelfHook", "dpkg", "makeWrapper"];
        let mut clean_deps: Vec<String> = pkgs_list.iter()
            .map(|p| sanitize_var(p.split('.').last().unwrap()))
            .collect();
        clean_deps.sort(); clean_deps.dedup();

        let mut input_str = inputs.join(", ");
        if !clean_deps.is_empty() { input_str.push_str(", "); input_str.push_str(&clean_deps.join(", ")); }
        format!("{{ {input_str} }}:")
    } else { "{ pkgs ? import <nixpkgs> {} }:".to_string() };

    let (stdenv, fetchurl, native, lib_path_expr) = if mode_upstream {
        ("stdenv.mkDerivation", "fetchurl", "    autoPatchelfHook\n    dpkg\n    makeWrapper", format!("lib.makeLibraryPath {}", lib_list_upstream))
    } else {
        ("pkgs.stdenv.mkDerivation", "pkgs.fetchurl", "    pkgs.autoPatchelfHook\n    pkgs.dpkg\n    pkgs.makeWrapper", format!("pkgs.lib.makeLibraryPath {}", lib_list_local))
    };

    match pkg_type {
        PackageType::AppImage => format!(
            include_str!("../templates/appimage.in"),
            header = header,
            name = meta.name,
            version = meta.version,
            fetchurl = fetchurl,
            url = url,
            sha256 = sha256,
            deps_string = deps_string
        ),

        PackageType::Deb => format!(
            include_str!("../templates/deb.in"),
            header = header,
            stdenv = stdenv,
            name = meta.name,
            version = meta.version,
            fetchurl = fetchurl,
            url = url,
            sha256 = sha256,
            native = native,
            build_inputs_body = build_inputs_body,
            lib_path_expr = lib_path_expr
        )
    }
}

fn create_pr(nixpkgs_path: &str, meta: &PackageMeta, content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo_path = Path::new(nixpkgs_path);
    if !repo_path.exists() { return Err("Nixpkgs path does not exist".into()); }

    let first_two = &meta.name[0..2];
    let package_dir = repo_path.join("pkgs").join("by-name").join(first_two).join(&meta.name);
    fs::create_dir_all(&package_dir)?;

    let package_file = package_dir.join("package.nix");
    let mut file = fs::File::create(&package_file)?;
    file.write_all(content.as_bytes())?;
    println!(">>> Created file: {:?}", package_file);

    let branch_name = format!("init-{}", meta.name);
    let run_git = |args: &[&str]| { Command::new("git").current_dir(repo_path).args(args).status() };

    run_git(&["checkout", "master"])?;
    run_git(&["pull"])?;
    run_git(&["checkout", "-b", &branch_name])?;
    run_git(&["add", "."])?;
    let commit_msg = format!("init: {} {}", meta.name, meta.version);
    run_git(&["commit", "-m", &commit_msg])?;

    println!(">>> GitHub: Creating PR...");
    let pr_status = Command::new("gh").current_dir(repo_path)
        .args(&["pr", "create", "--fill", "--title", &commit_msg]).status()?;

    if pr_status.success() { println!(">>> SUCCESS! Pull Request created."); }
    else { eprintln!(">>> Failed to create PR. Check 'gh auth status'."); }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { eprintln!("Usage: {} <url>", args[0]); std::process::exit(1); }
    let url = &args[1];

    println!(">>> [1/8] Initializing...");
    let temp_filename = "downloaded_temp_file";
    let _ = Command::new("nix-prefetch-url").arg(url).output()?;
    let output = Command::new("nix-prefetch-url").arg(url).output()?;
    let sha256 = String::from_utf8(output.stdout)?.trim().to_string();

    if !Path::new(temp_filename).exists() {
        let _ = Command::new("wget").arg("-q").arg("--user-agent=Mozilla/5.0").arg(url).arg("-O").arg(temp_filename).status()?;
    }

    let pkg_type = detect_file_type(temp_filename)?;
    let final_filename = match pkg_type { PackageType::Deb => "package.deb", PackageType::AppImage => "package.AppImage" };
    fs::rename(temp_filename, final_filename)?;
    if pkg_type == PackageType::AppImage { let _ = Command::new("chmod").arg("+x").arg(final_filename).status(); }

    let meta = extract_metadata(final_filename, &pkg_type);
    println!(">>> [Metadata] {} v{}", meta.name, meta.version);

    let extract_dir = "extracted_root";
    let _ = fs::remove_dir_all(extract_dir);
    match pkg_type {
        PackageType::AppImage => {
            let _ = fs::remove_dir_all("squashfs-root");
            let _ = Command::new(format!("./{}", final_filename)).arg("--appimage-extract").stdout(Stdio::null()).stderr(Stdio::null()).status();
            fs::rename("squashfs-root", extract_dir)?;
        },
        PackageType::Deb => {
            fs::create_dir(extract_dir)?;
            let _ = Command::new("dpkg-deb").arg("-x").arg(final_filename).arg(extract_dir).stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
    }

    // Dependency Analysis
    let mut internal_libs = HashSet::new();
    let mut all_needed = HashSet::new();
    for entry in WalkDir::new(extract_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.contains(".so") { internal_libs.insert(name.to_string()); }
        }
        if path.is_file() {
            if let Ok(out) = Command::new("patchelf").arg("--print-needed").arg(path).stderr(Stdio::null()).output() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    if !line.trim().is_empty() { all_needed.insert(line.trim().to_string()); }
                }
            }
        }
    }
    let missing_libs: Vec<_> = all_needed.difference(&internal_libs).collect();

    let mut pkg_map = HashMap::new();
    if pkg_type == PackageType::Deb {
        pkg_map.insert("glibc".to_string(), "implicit");
        pkg_map.insert("libgcc".to_string(), "implicit");
        pkg_map.insert("libglvnd".to_string(), "implicit-gpu");
        pkg_map.insert("mesa".to_string(), "implicit-gpu");
        pkg_map.insert("libdrm".to_string(), "implicit-gpu");
        pkg_map.insert("vulkan-loader".to_string(), "implicit-gpu");
    }

    println!(">>> [5/8] Resolving packages...");
    for lib in missing_libs {
        if lib.starts_with("ld-linux") { continue; }
        if let Some(pkg) = check_common_libs(lib) {
            pkg_map.insert(pkg.to_string(), lib.as_str());
            continue;
        }
        if let Ok(out) = Command::new("nix-locate").arg("--top-level").arg("--minimal").arg("--at-root").arg("--whole-name").arg(lib).output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(pkg) = stdout.lines().find(|l| !l.contains('(')) {
                if !pkg.trim().is_empty() { pkg_map.insert(pkg.trim().to_string(), lib.as_str()); }
            }
        }
    }

    if pkg_map.contains_key("qt6.qtbase") && pkg_map.contains_key("qt5.qtbase") {
        println!(">>> [Warning] Detected both Qt5 and Qt6 dependencies. Removing Qt5 to avoid conflicts.");
        pkg_map.retain(|k, _| !k.starts_with("qt5"));
    }

    let mut sorted_pkgs: Vec<String> = pkg_map.keys().cloned().collect();
    sorted_pkgs.sort();

    println!(">>> [6/8] Generating local default.nix...");
    let local_nix = generate_nix_content(&pkg_type, &meta, url, &sha256, &sorted_pkgs, false);
    let mut file = fs::File::create("default.nix")?;
    file.write_all(local_nix.as_bytes())?;

    println!(">>> [7/8] Running nix-build test...");
    let build_status = Command::new("nix-build").status()?;

    if build_status.success() {
        println!("\n✅ Build SUCCESSFUL!");
        println!("Would you like to submit a Pull Request to nixpkgs? (y/N)");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            println!("Enter absolute path to your local nixpkgs clone:");
            let mut path_input = String::new();
            io::stdin().read_line(&mut path_input)?;
            let nixpkgs_path = path_input.trim();
            if !nixpkgs_path.is_empty() {
                println!(">>> Generating upstream-compatible Nix file...");
                let upstream_nix = generate_nix_content(&pkg_type, &meta, url, &sha256, &sorted_pkgs, true);
                match create_pr(nixpkgs_path, &meta, &upstream_nix) {
                    Ok(_) => println!("🚀 PR process finished."),
                    Err(e) => eprintln!("❌ PR Error: {}", e),
                }
            }
        }
    } else {
        eprintln!("❌ Build failed.");
    }

    let _ = fs::remove_dir_all(extract_dir);
    let _ = fs::remove_file(final_filename);
    Ok(())
}
