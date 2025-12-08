use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::structs::PackageInfo;

static DEB_TO_NIX_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // --- Basic System Libraries ---
    m.insert("libc6", "glibc");
    m.insert("libasound2", "alsa-lib");
    m.insert("ca-certificates", "cacert");
    m.insert("libglib2.0-0", "glib");
    m.insert("libgcc-s1", "gcc.cc.lib");
    m.insert("libstdc++6", "gcc.cc.lib");
    m.insert("zlib1g", "zlib");

    // --- Graphics Stack and Sound ---
    m.insert("libatk-bridge2.0-0", "at-spi2-atk");
    m.insert("libatspi2.0-0", "at-spi2-core");
    m.insert("libatk1.0-0", "atk");
    m.insert("libcairo2", "cairo");
    m.insert("libcups2", "cups");
    m.insert("libdbus-1-3", "dbus");
    m.insert("libexpat1", "expat");
    m.insert("libgbm1", "libgbm");
    m.insert("libgtk-3-0", "gtk3");
    m.insert("libpango-1.0-0", "pango");
    m.insert("libudev1", "systemd");
    m.insert("libvulkan1", "vulkan-loader");
    m.insert("fonts-liberation", "liberation_ttf");

    // --- X11 / Xorg ---
    m.insert("libx11-6", "xorg.libX11");
    m.insert("libxcb1", "xorg.libxcb");
    m.insert("libxcomposite1", "xorg.libXcomposite");
    m.insert("libxdamage1", "xorg.libXdamage");
    m.insert("libxext6", "xorg.libXext");
    m.insert("libxfixes3", "xorg.libXfixes");
    m.insert("libxkbcommon0", "libxkbcommon");
    m.insert("libxrandr2", "xorg.libXrandr");
    m.insert("libx11-xcb1", "xorg.libX11");

    // --- Network and Security ---
    m.insert("libcurl4", "curl");
    m.insert("libcurl3-gnutls", "curl");
    m.insert("libnspr4", "nspr");
    m.insert("libnss3", "nss");
    m.insert("libssl3", "openssl");
    m.insert("libssl1.1", "openssl_1_1");

    // --- Qt ---
    m.insert("libqt5core5a", "qt5.qtbase");
    m.insert("libqt5gui5", "qt5.qtbase");
    m.insert("libqt5widgets5", "qt5.qtbase");
    m.insert("libqt5dbus5", "qt5.qtbase");
    m.insert("libqt5network5", "qt5.qtbase");
    m.insert("libqt5qml5", "qt5.qtdeclarative");
    m.insert("libqt5quick5", "qt5.qtdeclarative");
    m.insert("libqt5webchannel5", "qt5.qtwebchannel");
    m.insert("libqt5websockets5", "qt5.qtwebsockets");
    m.insert("libqt5x11extras5", "qt5.qtx11extras");
    m.insert("libqt6core6", "qt6.qtbase");
    m.insert("libqt6gui6", "qt6.qtbase");
    m.insert("libqt6widgets6", "qt6.qtbase");
    m.insert("libqt6dbus6", "qt6.qtbase");

    // --- Utilities ---
    m.insert("xdg-utils", "xdg-utils");
    m.insert("wget", "wget");
    m.insert("jq", "jq");
    m.insert("squashfs-tools", "squashfsTools");
    m.insert("binutils", "binutils");

    // --- Desktop / Notifications / Secrets ---
    m.insert("libnotify4", "libnotify");
    m.insert("libsecret-1-0", "libsecret");

    // --- X11 (specific versions) ---
    m.insert("libxss1", "xorg.libXScrnSaver");
    m.insert("libxtst6", "xorg.libXtst");

    // --- System Libraries ---
    m.insert("libuuid1", "libuuid");
    m.insert("libdrm2", "libdrm");
    m.insert("libgconf-2-4", "gconf"); 
    
    m
});

fn guess_library_filenames(debian_name: &str) -> Vec<String> {
    let mut guesses = Vec::new();
    // Match libnameN or libname-version
    // Heuristic: start with 'lib', take everything until the first digit (or digit after separator)
    let re_lib = Regex::new(r"^lib(.+?)[-\.]?(\d+(?:\.\d+)*)$").unwrap();

    if let Some(caps) = re_lib.captures(debian_name) {
        let name = &caps[1];
        let version = &caps[2];
        guesses.push(format!("lib{}.so.{}", name, version));
        guesses.push(format!("lib{}.so", name));
    } else if debian_name.starts_with("lib") {
         guesses.push(format!("{}.so", debian_name));
    }

    guesses
}

fn search_nix_locate(filename: &str) -> Option<String> {
    // nix-locate --top-level --minimal --defined --whole-name filename
    // Returns e.g. "nixpkgs.zlib.out"

    let output = Command::new("nix-locate")
        .args(["--top-level", "--minimal", "--defined", "--whole-name", filename])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        let parts: Vec<&str> = trimmed.split('.').collect();
        // Expecting nixpkgs.<package>...
        if parts.len() >= 2 && parts[0] == "nixpkgs" {
             // Handle output suffixes like .out, .lib, .bin
             let end_index = if matches!(parts.last(), Some(&"out") | Some(&"lib") | Some(&"bin") | Some(&"dev")) {
                 parts.len() - 1
             } else {
                 parts.len()
             };

             if end_index > 1 {
                 return Some(parts[1..end_index].join("."));
             }
        }
    }
    None
}

fn find_nix_package_from_db(debian_name: &str) -> Result<String, Box<dyn Error>> {
    // 1. Check Map (Direct)
    if let Some(nix_name) = DEB_TO_NIX_MAP.get(debian_name) {
        return Ok(nix_name.to_string());
    }

    // 2. Clean version number suffix
    let re = Regex::new(r"[-0-9\.]+$").unwrap();
    let cleaned_name = re.replace(debian_name, "").to_string();
    if cleaned_name != debian_name {
        if let Some(nix_name) = DEB_TO_NIX_MAP.get(cleaned_name.as_str()) {
            return Ok(nix_name.to_string());
        }
    }

    // 3. Remove "lib" prefix
    if let Some(without_lib) = cleaned_name.strip_prefix("lib") {
         if let Some(nix_name) = DEB_TO_NIX_MAP.get(without_lib) {
            return Ok(nix_name.to_string());
        }
    }

    // 4. Heuristic Search with nix-locate
    // Only attempt this if it looks like a library or we are desperate
    let guesses = guess_library_filenames(debian_name);
    if !guesses.is_empty() {
        println!(">>> Trying to resolve '{}' using nix-locate...", debian_name);
        for filename in guesses {
            if let Some(pkg) = search_nix_locate(&filename) {
                println!(">>> Found '{}' in package '{}' via nix-locate", filename, pkg);
                return Ok(pkg);
            }
        }
    }

    Err(format!("'{}' not found in knowledge base or via nix-locate.", debian_name).into())
}

pub fn get_nix_shell(filename: &str, skip_deps: bool) -> Result<PackageInfo, Box<dyn Error>> {
    if filename.is_empty() {
        return Err("Filename cannot be empty".into());
    }

    let mut package_info = PackageInfo::default();

    let cmd = format!("dpkg-deb -f '{}'", filename);
    let output = Command::new("nix-shell")
        .args(["-p", "dpkg", "--run", &cmd])
        .output()
        .map_err(|e| format!("Failed to execute nix-shell: {}", e))?;

    if output.status.success() {
        let stdout_raw = String::from_utf8_lossy(&output.stdout);
        let stdout = stdout_raw.trim().to_owned();
        let lines = stdout.lines();

        for line in lines {
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
            } else if line.starts_with("Depends: ") {
                if skip_deps {
                    println!("Skipping dependency resolution.");
                    continue;
                }
                let dependencies = line["Depends: ".len()..]
                    .trim()
                    .split(", ")
                    .collect::<Vec<_>>();

                for dep in dependencies {
                    let clean_name = dep.split('(').next().unwrap_or(dep).trim();
                    if clean_name.is_empty() {
                        continue;
                    }

                    if clean_name.contains('|') {
                        let alternatives: Vec<&str> = clean_name.split(" | ").map(|s| s.trim()).collect();
                        let mut found_match = false;
                        for name in alternatives {
                            if name.is_empty() { continue; }
                            match find_nix_package_from_db(name) {
                                Ok(found) => {
                                    println!("Dependency found: {} -> {}", name, found);
                                    if !package_info.deps.contains(&found) {
                                        package_info.deps.push(found);
                                    }
                                    found_match = true;
                                    break;
                                }
                                Err(_) => continue,
                            }
                        }
                        if !found_match {
                            eprintln!("Warning: No alternative found for '{}'.", clean_name);
                        }
                    } else {
                        match find_nix_package_from_db(clean_name) {
                            Ok(found) => {
                                println!("Dependency found: {} -> {}", clean_name, found);
                                if !package_info.deps.contains(&found) {
                                    package_info.deps.push(found);
                                }
                            },
                            Err(e) => eprintln!("Dependency search failed: {}", e),
                        }
                    }
                }
            }
        }

        Ok(package_info)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("Nix-shell failed: {}", stderr).into())
    }
}
