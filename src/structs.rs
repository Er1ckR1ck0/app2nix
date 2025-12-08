use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct NixPackageInfo {
    pub pname: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Default)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub deps: Vec<String>,
    pub arch: String,
    pub description: String,
    pub meta: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum PackageType {
    Deb,
}
