use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::process::Command;

use tempfile::tempdir;
use walkdir::WalkDir;

use crate::structs::PackageInfo;
use crate::configuration::{
    get_pkg_for_lib,
    is_system_lib,
};

fn ensure_tools_dependencies() -> Result<(), Box<dyn Error>> {
    let tools = vec!["patchelf", "ar", "tar"];
    let mut missing = Vec::new();

    for tool in tools {
        let output = Command::new("which").arg(tool).output();
        match output {
            Ok(out) if out.status.success() => {},
            _ => missing.push(tool),
        }
    }

    if !missing.is_empty() {
        return Err(format!("Missing required tools: {}", missing.join(", ")).into());
    }

    Ok(())
}

fn resolve_lib_via_locate(lib_name: &str) -> Option<String> {
    if let Some(pkg) = get_pkg_for_lib(lib_name) {
        return Some(pkg.clone());
    }

    let search_path = format!("/lib/{}", lib_name);


    let which_output = Command::new("which").arg("nix-locate").output();
    if which_output.is_err() || !which_output.unwrap().status.success() {
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
        .args(["--top-level", "--minimal", "--whole-name", lib_name])
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


    ensure_tools_dependencies()?;

    let tmp_dir = tempdir()?;
    let tmp_path = tmp_dir.path();
    let abs_deb_path = fs::canonicalize(deb_path)?;


    let ar_output = Command::new("ar")
        .arg("x")
        .arg(&abs_deb_path)
        .current_dir(tmp_path)
        .output()?;

    if !ar_output.status.success() {
        return Err("Failed to unpack deb archive with 'ar'".into());
    }


    let mut data_tar: Option<String> = None;
    for entry in fs::read_dir(tmp_path)? {
        let entry = entry?;
        let name_str = entry.file_name().to_string_lossy().to_string();
        if name_str.starts_with("data.tar") {
            data_tar = Some(name_str);
            break;
        }
    }

    let tar_name = data_tar.ok_or("Could not find data.tar.* archive inside deb")?;

    let tar_output = Command::new("tar")
        .arg("xf")
        .arg(&tar_name)
        .current_dir(tmp_path)
        .output()?;

    if !tar_output.status.success() {
        eprintln!("Warning: failed to extract {}", tar_name);
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
                    if lib.is_empty() {
                        continue;
                    }


                    if is_system_lib(lib) {
                        continue;
                    }



                    if get_pkg_for_lib(lib).is_some() || !bundled_files.contains(lib) {
                        needed_libs.insert(lib.to_string());
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
            }
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


    let output = Command::new("dpkg")
        .arg("--info")
        .arg(filename)
        .output();

    let output = match output {
        Ok(ref out) if out.status.success() => Ok(out.clone()),
        _ => {

            let cmd = format!("dpkg-deb -f '{}'", filename);
            Command::new("nix-shell")
                .args(["-p", "dpkg", "--run", &cmd])
                .output()
        }
    }.map_err(|e| format!("Failed to read deb info: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(value) = line.strip_prefix("Package: ") {
                package_info.name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("Version: ") {
                package_info.version = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("Architecture: ") {
                package_info.arch = match value.trim() {
                    "amd64" => "x86_64-linux".to_string(),
                    "arm64" => "aarch64-linux".to_string(),
                    arch => arch.to_string(),
                };
            } else if let Some(value) = line.strip_prefix("Description: ") {
                package_info.description = value.trim().to_string();
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
                    for lib in &missing {
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
