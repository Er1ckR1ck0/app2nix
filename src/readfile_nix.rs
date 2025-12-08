use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::structs::PackageInfo;

static DEB_TO_NIX_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // --- Базовые системные библиотеки ---
    m.insert("libc6", "glibc");
    m.insert("libasound2", "alsa-lib");
    m.insert("ca-certificates", "cacert");
    m.insert("libglib2.0-0", "glib");
    m.insert("libgcc-s1", "gcc.cc.lib");
    m.insert("libstdc++6", "gcc.cc.lib");

    // --- Графический стек и звук ---
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
    m.insert("libx11-xcb1", "xorg.libX11"); // Часто идет вместе с libX11

    // --- Сеть и безопасность ---
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

    // --- Утилиты ---
    m.insert("xdg-utils", "xdg-utils");
    m.insert("wget", "wget");
    m.insert("jq", "jq");
    m.insert("squashfs-tools", "squashfsTools");
    m.insert("binutils", "binutils");

    // --- Desktop / Notifications / Secrets ---
    m.insert("libnotify4", "libnotify");      // Уведомления рабочего стола
    m.insert("libsecret-1-0", "libsecret");   // Хранение паролей (нужен VS Code/Chrome)

    // --- X11 (конкретные версии, которые часто просят) ---
    m.insert("libxss1", "xorg.libXScrnSaver"); // Скринсейвер
    m.insert("libxtst6", "xorg.libXtst");      // Эмуляция ввода (нужен для автотестов UI)

    // --- Системные библиотеки ---
    m.insert("libuuid1", "libuuid");           // Генерация UUID
    m.insert("libdrm2", "libdrm");
    m.insert("libgbm1", "mesa"); // В NixOS libgbm.so часто лежит в mesa
    m.insert("libasound2", "alsa-lib");
    m.insert("libsecret-1-0", "libsecret");
    m.insert("libnotify4", "libnotify");
    m.insert("libuuid1", "libuuid");
    m.insert("libxss1", "xorg.libXScrnSaver");
    m.insert("libxtst6", "xorg.libXtst");
    m.insert("libgconf-2-4", "gconf"); 
    m.insert("libnss3", "nss");
    m.insert("libatk-bridge2.0-0", "at-spi2-atk");
    m.insert("libatspi2.0-0", "at-spi2-core");
    
    m
});

fn find_nix_package_from_db(debian_name: &str) -> Result<String, Box<dyn Error>> {
    if let Some(nix_name) = DEB_TO_NIX_MAP.get(debian_name) {
        return Ok(nix_name.to_string());
    }

    let re = Regex::new(r"[-0-9\.]+$").unwrap();
    let cleaned_name = re.replace(debian_name, "").to_string();
    if cleaned_name != debian_name {
        if let Some(nix_name) = DEB_TO_NIX_MAP.get(cleaned_name.as_str()) {
            return Ok(nix_name.to_string());
        }
    }

    if let Some(without_lib) = cleaned_name.strip_prefix("lib") {
         if let Some(nix_name) = DEB_TO_NIX_MAP.get(without_lib) {
            return Ok(nix_name.to_string());
        }
    }

    Err(format!("'{}' не найден в базе знаний.", debian_name).into())
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
                    println!("Пропуск разрешения зависимостей.");
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
                                    println!("Найдена зависимость: {} -> {}", name, found);
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
                            eprintln!("Ошибка: Ни один из альтернативных пакетов для '{}' не найден.", clean_name);
                        }
                    } else {
                        match find_nix_package_from_db(clean_name) {
                            Ok(found) => {
                                println!("Найдена зависимость: {} -> {}", clean_name, found);
                                if !package_info.deps.contains(&found) {
                                    package_info.deps.push(found);
                                }
                            },
                            Err(e) => eprintln!("Ошибка поиска зависимости: {}", e),
                        }
                    }
                }
            }
        }

        Ok(package_info)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("Nix-shell не удался: {}", stderr).into())
    }
}