use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

mod generation_nix;
mod readfile_nix;
mod structs;

fn ensure_nix_shell() {
    let tools = ["patchelf", "nix-locate", "ar", "tar"];
    let has_tools = tools.iter().all(|t| {
        Command::new("which")
            .arg(t)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });

    if has_tools {
        return;
    }

    println!(">>> 🪄  Missing tools. Auto-escalating to nix-shell...");
    let args: Vec<String> = env::args().collect();
    let cmd = args
        .iter()
        .map(|a| format!("'{}'", a.replace("'", "'\\''")))
        .collect::<Vec<_>>()
        .join(" ");

    let err = Command::new("nix-shell")
        .args(["-p", "patchelf", "binutils", "nix-index", "--run", &cmd])
        .exec(); 

    panic!("Failed to auto-restart in nix-shell: {}", err);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ensure_nix_shell();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <url> [--skip-deps]", args[0]);
        std::process::exit(1);
    }
    let url = &args[1];
    let skip_deps = args.contains(&"--skip-deps".to_string());

    if url.is_empty() {
        eprintln!("Error: URL is empty");
        std::process::exit(1);
    }

    let mut temp_filename = url.rsplit('/').next().unwrap_or("downloaded_file.deb");
    if temp_filename.is_empty() {
        temp_filename = "downloaded_file.deb";
    }

    let temp_path = Path::new(temp_filename);
    if !temp_path.exists() {
        println!(">>> [1/4] Downloading file from {}", url);
        let status = Command::new("wget").args(&["-O", temp_filename, url]).status()?;
        if !status.success() {
            return Err("Failed to download file.".into());
        }
    } else {
        println!(">>> [1/4] File {} exists, skipping download.", temp_filename);
    }

    println!(">>> [2/4] Calculating SHA256 hash...");
    let abs_path = env::current_dir()?.join(temp_filename);
    let path_str = abs_path.to_str().ok_or("Invalid path")?;
    
    let output = Command::new("nix")
        .args(["hash", "file", "--type", "sha256", path_str])
        .env("NIX_CONFIG", "experimental-features = nix-command flakes")
        .output()?;
        
    if !output.status.success() {
        return Err(format!("Hash failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    let sha256 = String::from_utf8(output.stdout)?.trim().to_string();

    println!(">>> [3/4] Reading package info...");
    let package_info = readfile_nix::get_nix_shell(temp_filename, skip_deps)?;

    println!(">>> [4/4] Generating default.nix...");
    let nix_content = generation_nix::generate_nix_content(
        &structs::PackageType::Deb,
        &package_info,
        url,
        &sha256,
        false,
    );

    fs::write("default.nix", nix_content)?;
    println!("\ndefault.nix has been generated successfully.");

    Ok(())
}