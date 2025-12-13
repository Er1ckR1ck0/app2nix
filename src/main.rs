use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

mod generation_nix;
mod readfile_nix;
mod structs;
mod configuration;

enum InputType<'a> {
    Url(&'a str),
    LocalFile(&'a str),
}

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
        eprintln!("Usage: {} <url_or_path> [--skip-deps]", args[0]);
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <url_or_path>   URL to download .deb file OR local path to .deb file");
        eprintln!("  --skip-deps     Skip automatic dependency resolution");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} https://example.com/package.deb", args[0]);
        eprintln!("  {} /home/user/downloads/package.deb", args[0]);
        eprintln!("  {} ./package.deb --skip-deps", args[0]);
        std::process::exit(1);
    }

    let input = &args[1];
    let skip_deps = args.contains(&"--skip-deps".to_string());

    let input_type = match input.as_str() {
        "" => {
            eprintln!("Error: Input path or URL is empty");
            std::process::exit(1);
        }
        s if !s.ends_with(".deb") => {
            eprintln!("Error: Input must be a .deb file (got: {})", s);
            std::process::exit(1);
        }
        s if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ftp://") => {
            InputType::Url(s)
        }
        s if Path::new(s).exists() => {
            InputType::LocalFile(s)
        }
        s => {
            eprintln!("Error: File not found: {}", s);
            std::process::exit(1);
        }
    };

    let (deb_path, url_for_nix, is_remote) = match input_type {
        InputType::Url(url) => {
            let temp_filename = url.rsplit('/').next().unwrap_or("downloaded_file.deb");
            let temp_filename = if temp_filename.is_empty() { "downloaded_file.deb" } else { temp_filename };

            if !Path::new(temp_filename).exists() {
                println!(">>> [1/4] Downloading file from {}", url);
                let status = Command::new("wget").args(["-O", temp_filename, url]).status()?;
                if !status.success() {
                    return Err("Failed to download file.".into());
                }
            } else {
                println!(">>> [1/4] File {} exists, skipping download.", temp_filename);
            }

            (temp_filename.to_string(), url.to_string(), true)
        }
        InputType::LocalFile(path) => {
            println!(">>> [1/4] Using local file: {}", path);
            let abs_path = fs::canonicalize(path)?;
            let abs_str = abs_path.to_string_lossy().to_string();
            (abs_str.clone(), abs_str, false)
        }
    };

    println!(">>> [2/4] Calculating SHA256 hash...");
    let abs_path = fs::canonicalize(&deb_path)?;
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
    let package_info = readfile_nix::get_nix_shell(&deb_path, skip_deps)?;

    println!(">>> [4/4] Generating default.nix...");
    let nix_content = generation_nix::generate_nix_content(
        &structs::PackageType::Deb,
        &package_info,
        &url_for_nix,
        &sha256,
        is_remote,
    );

    fs::write("default.nix", nix_content)?;
    println!("\n✅ default.nix has been generated successfully.");

    if !is_remote {
        println!("\n⚠️  Note: Local file was used. The generated default.nix uses file:// URL.");
        println!("   For distribution, replace the URL with a remote location.");
    }

    Ok(())
}
