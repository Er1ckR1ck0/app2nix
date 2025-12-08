use std::fs;
use std::env;
use std::process::Command;
use std::path::Path;

mod structs;
mod readfile_nix;
mod generation_nix;

fn main() -> Result<(), Box<dyn std::error::Error>> {

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
        eprintln!("Could not determine filename from URL. Take temp name 'downloaded_file.deb'");
        temp_filename = "downloaded_file.deb";
    }

    let temp_path = Path::new(temp_filename);
    if !temp_path.exists() {
        println!(">>> [1/4] Downloading file from {}", url);
        let download_status = Command::new("wget").args(&["-O", temp_filename, url]).status()?;
        if !download_status.success() {
            return Err("Failed to download the file using wget.".into());
        }
    } else {
        println!(">>> [1/4] File {} already exists, skipping download.", temp_filename);
    }

    println!(">>> [2/4] Calculating SHA256 hash...");
    let current_dir = env::current_dir()?;
    let absolute_path = current_dir.join(temp_filename);

    let file_path_str = absolute_path.to_str().ok_or("Invalid path")?;
    let output = Command::new("nix")
        .args(["hash", "file", "--type", "sha256", file_path_str])
        .env("NIX_CONFIG", "experimental-features = nix-command flakes")
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to get SHA256 hash: {}", stderr).into());
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