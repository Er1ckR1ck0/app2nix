use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LibrariesConfig {
    pub system_libs: Vec<String>,
    pub lib_to_pkg_map: std::collections::HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub deps: Vec<String>,
    pub arch: String,
    pub description: String
}

#[derive(Debug, PartialEq, Clone)]
pub enum PackageType {
    Deb,
}
