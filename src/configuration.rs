use std::sync::OnceLock;
use std::path::Path;
use std::error::Error;
use std::fs;

use serde_json;

use crate::structs::{LibrariesConfig};

pub static LIBRARIES_CONFIG: OnceLock<LibrariesConfig> = OnceLock::new();

pub const LIBRARIES_JSON_PATH: &str = "libraries.json";


fn get_config_path() -> String {
    let paths = [
        LIBRARIES_JSON_PATH.to_string(),
        format!("../{}", LIBRARIES_JSON_PATH),
        format!("{}/{}", env!("CARGO_MANIFEST_DIR"), LIBRARIES_JSON_PATH),
    ];

    for path in &paths {
        if Path::new(path).exists() {
            return path.clone();
        }
    }

    LIBRARIES_JSON_PATH.to_string()
}

pub fn load_libraries_config() -> Result<LibrariesConfig, Box<dyn Error>> {
    let config_path = get_config_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read {}: {}", config_path, e))?;

    let config: LibrariesConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", config_path, e))?;

    Ok(config)
}

pub fn is_system_lib(lib_name: &str) -> bool {
    get_libraries_config().system_libs.contains(&lib_name.to_string())
}

pub fn get_pkg_for_lib(lib_name: &str) -> Option<&'static String> {
    get_libraries_config().lib_to_pkg_map.get(lib_name)
}

fn get_libraries_config() -> &'static LibrariesConfig {
    LIBRARIES_CONFIG.get_or_init(|| {
        load_libraries_config().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load libraries config: {}. Using defaults.", e);
            LibrariesConfig {
                system_libs: vec![
                    "libc.so.6".to_string(),
                    "libm.so.6".to_string(),
                    "libdl.so.2".to_string(),
                    "libpthread.so.0".to_string(),
                    "librt.so.1".to_string(),
                    "libutil.so.1".to_string(),
                    "libresolv.so.2".to_string(),
                    "ld-linux-x86-64.so.2".to_string(),
                    "libgcc_s.so.1".to_string(),
                    "libstdc++.so.6".to_string(),
                ],
                lib_to_pkg_map: std::collections::HashMap::new(),
            }
        })
    })
}
