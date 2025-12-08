use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::process::Command;
use once_cell::sync::Lazy;
use tempfile::tempdir;
use walkdir::WalkDir;
use crate::structs::PackageInfo;

static SYSTEM_LIBS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.insert("libc.so.6");
    s.insert("libm.so.6");
    s.insert("libdl.so.2");
    s.insert("libpthread.so.0");
    s.insert("librt.so.1");
    s.insert("libutil.so.1");
    s.insert("libresolv.so.2");
    s.insert("ld-linux-x86-64.so.2");
    s.insert("libgcc_s.so.1");
    s.insert("libstdc++.so.6");
    s
});


static LIB_TO_PKG_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // --- GLib / GTK / Pango / Cairo / ATK ---
    m.insert("libglib-2.0.so.0", "glib");
    m.insert("libgobject-2.0.so.0", "glib");
    m.insert("libgio-2.0.so.0", "glib");
    m.insert("libgtk-3.so.0", "gtk3");
    m.insert("libgdk-3.so.0", "gtk3");
    m.insert("libpango-1.0.so.0", "pango");
    m.insert("libpangocairo-1.0.so.0", "pango");
    m.insert("libcairo.so.2", "cairo");
    m.insert("libcairo-gobject.so.2", "cairo");
    m.insert("libgdk_pixbuf-2.0.so.0", "gdk-pixbuf");
    m.insert("libatk-1.0.so.0", "at-spi2-atk");
    m.insert("libatk-bridge-2.0.so.0", "at-spi2-atk");
    m.insert("libatspi.so.0", "at-spi2-core");
    m.insert("libdbus-1.so.3", "dbus");

    // --- X11 / Graphics / Desktop ---
    m.insert("libX11.so.6", "xorg.libX11");
    m.insert("libxcb.so.1", "xorg.libxcb");
    m.insert("libXcomposite.so.1", "xorg.libXcomposite");
    m.insert("libXdamage.so.1", "xorg.libXdamage");
    m.insert("libXext.so.6", "xorg.libXext");
    m.insert("libXfixes.so.3", "xorg.libXfixes");
    m.insert("libXrandr.so.2", "xorg.libXrandr");
    m.insert("libXrender.so.1", "xorg.libXrender");
    m.insert("libxshmfence.so.1", "libxshmfence");
    m.insert("libdrm.so.2", "libdrm");
    m.insert("libgbm.so.1", "mesa");
    m.insert("libGL.so.1", "libglvnd");
    m.insert("libEGL.so.1", "libglvnd");
    m.insert("libGLESv2.so.2", "libglvnd");
    m.insert("libvulkan.so.1", "vulkan-loader");
    
    // --- Network / Security / Cryptography ---
    m.insert("libnspr4.so", "nspr");
    m.insert("libnss3.so", "nss");
    m.insert("libnssutil3.so", "nss");
    m.insert("libsmime3.so", "nss");
    m.insert("libssl.so.3", "openssl");
    m.insert("libcrypto.so.3", "openssl");
    m.insert("libsecret-1.so.0", "libsecret");

    // --- Common Utils ---
    m.insert("libz.so.1", "zlib");
    m.insert("libexpat.so.1", "expat");
    m.insert("libuuid.so.1", "libuuid");
    m.insert("libcups.so.2", "cups");
    m.insert("libasound.so.2", "alsa-lib");
    m.insert("libfreetype.so.6", "freetype");
    m.insert("libfontconfig.so.1", "fontconfig");
    m.insert("libxkbcommon.so.0", "libxkbcommon");
    
    // --- Hacks for Electron ---
    // Electron bundles ffmpeg, but sometimes patchelf asks for it anyway.
    // We map it to ffmpeg-headless or just ffmpeg, hoping it helps if the bundle fails.
    m.insert("libffmpeg.so", "ffmpeg"); 
    m
});

fn ensure_tools_dependencies() -> Result<(), Box<dyn Error>> {
    let tools = vec!["patchelf", "ar", "tar"];
    let mut missing = Vec::new();

    for tool in tools {
        if Command::new("which").arg(tool).output().is_err() {
            missing.push(tool);
        }
    }
    Ok(())
}

fn resolve_lib_via_locate(lib_name: &str) -> Option<String> {
    if let Some(pkg) = LIB_TO_PKG_MAP.get(lib_name) {
        return Some(pkg.to_string());
    }

    
    let search_path = format!("/lib/{}", lib_name);
    
    
    if Command::new("which").arg("nix-locate").output().is_err() {
        return None;
    }

    let output = Command::new("nix-locate")
        .args(["--top-level", "--minimal", "--at-root", "--whole-name", &search_path])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().next() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.split('.').collect();
                return Some(parts.last().unwrap_or(&trimmed).to_string());
            }
        }
    }
    
    
    let output_loose = Command::new("nix-locate")
        .args(["--top-level", "--minimal", "--defined", "--whole-name", lib_name])
        .output()
        .ok()?;
        
    let stdout_loose = String::from_utf8_lossy(&output_loose.stdout);
    if let Some(line) = stdout_loose.lines().next() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
             let parts: Vec<&str> = trimmed.split('.').collect();
             return Some(parts.last().unwrap_or(&trimmed).to_string());
        }
    }

    None
}

fn scan_binary_and_resolve(deb_path: &str) -> Result<(Vec<String>, Vec<String>), Box<dyn Error>> {
    println!(">>> Unpacking and scanning binary dependencies (this may take a moment)...");
    
    let tmp_dir = tempdir()?;
    let tmp_path = tmp_dir.path();
    let abs_deb_path = fs::canonicalize(deb_path)?;
    
    let ar_status = Command::new("ar")
        .arg("x")
        .arg(&abs_deb_path)
        .current_dir(tmp_path)
        .status()?;

    if !ar_status.success() {
        return Err("Failed to unpack deb archive".into());
    }

    let mut data_tar = None;
    for entry in fs::read_dir(tmp_path)? {
        let entry = entry?;
        let name_str = entry.file_name().to_string_lossy().to_string();
        if name_str.starts_with("data.tar") {
            data_tar = Some(name_str);
            break;
        }
    }

    if let Some(tar_name) = data_tar {
        let tar_status = Command::new("tar")
            .arg("xf")
            .arg(&tar_name)
            .current_dir(tmp_path)
            .status()?;
        if !tar_status.success() {
             eprintln!("Warning: failed to extract {}", tar_name);
        }
    } else {
        return Err("Could not find data.tar.* archive inside deb".into());
    }

    let mut needed_libs = HashSet::new();
    let mut resolved_packages = HashSet::new();
    let mut missing_libs = Vec::new();

    let mut bundled_files = HashSet::new();
    for entry in WalkDir::new(tmp_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
             if let Some(fname) = entry.file_name().to_str() {
                 bundled_files.insert(fname.to_string());
             }
        }
    }

    for entry in WalkDir::new(tmp_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }

        let output = Command::new("patchelf")
            .arg("--print-needed")
            .arg(entry.path())
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    let lib = line.trim();
                    if !lib.is_empty() && !SYSTEM_LIBS.contains(lib) {
                        
                        
                        
                        if LIB_TO_PKG_MAP.contains_key(lib) {
                            needed_libs.insert(lib.to_string());
                        } else if !bundled_files.contains(lib) {
                             needed_libs.insert(lib.to_string());
                        }
                    }
                }
            }
        }
    }

    println!(">>> Identified {} unique shared libraries required by binaries.", needed_libs.len());

    for lib in needed_libs {
        match resolve_lib_via_locate(&lib) {
            Some(pkg) => {
                println!("    [+] Resolved: {} -> pkgs.{}", lib, pkg);
                resolved_packages.insert(pkg);
            },
            None => {
                println!("    [!] Warning: Could not find package for library '{}'", lib);
                missing_libs.push(lib);
            }
        }
    }

    let mut result_pkgs: Vec<String> = resolved_packages.into_iter().collect();
    result_pkgs.sort();
    missing_libs.sort();
    
    Ok((result_pkgs, missing_libs))
}

pub fn get_nix_shell(filename: &str, skip_deps: bool) -> Result<PackageInfo, Box<dyn Error>> {
    if filename.is_empty() {
        return Err("Filename cannot be empty".into());
    }

    let mut package_info = PackageInfo::default();

    
    let cmd = format!("dpkg-deb -f '{}'", filename);
    let output = Command::new("dpkg")
        .arg("--info")
        .arg(filename)
        .output();

    let output = if output.is_ok() && output.as_ref().unwrap().status.success() {
        output
    } else {
         Command::new("nix-shell")
            .args(["-p", "dpkg", "--run", &cmd])
            .output()
    }.map_err(|e| format!("Failed to read deb info: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        for line in stdout.lines() {
            if line.starts_with("Package: ") {
                package_info.name = line["Package: ".len()..].trim().to_string();
            } else if line.starts_with("Version: ") {
                package_info.version = line["Version: ".len()..].trim().to_string();
            } else if line.starts_with("Architecture: ") {
                let arch = line["Architecture: ".len()..].trim();
                package_info.arch = match arch {
                    "amd64" => "x86_64-linux".to_string(),
                    "arm64" => "aarch64-linux".to_string(),
                    _ => arch.to_string(),
                };
            } else if line.starts_with("Description: ") {
                package_info.description = line["Description: ".len()..].trim().to_string();
            }
        }
    }

    if !skip_deps {
        match scan_binary_and_resolve(filename) {
            Ok((deps, missing)) => {
                package_info.deps = deps;
                
                if !missing.is_empty() {
                    println!("\n========================================================");
                    println!(" WARNING: MISSING DEPENDENCIES DETECTED");
                    println!("========================================================");
                    for lib in missing {
                        println!(" - {}", lib);
                    }
                    println!("========================================================\n");
                }
            }
            Err(e) => {
                eprintln!("Error during binary scan: {}. Generating minimal config.", e);
            }
        }
    }

    Ok(package_info)
}