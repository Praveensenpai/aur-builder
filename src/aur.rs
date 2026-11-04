use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct AurResponse {
    results: Vec<AurPackage>,
}

#[derive(Deserialize, Debug)]
struct AurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
}

pub fn verify_aur_packages(packages: &[String]) -> Result<HashMap<String, String>> {
    if packages.is_empty() {
        return Ok(HashMap::new());
    }

    let mut url = String::from("https://aur.archlinux.org/rpc/v5/info");
    for (i, pkg) in packages.iter().enumerate() {
        let prefix = if i == 0 { '?' } else { '&' };
        url.push(prefix);
        url.push_str("arg[]=");
        url.push_str(pkg);
    }

    let response: AurResponse = ureq::get(&url)
        .call()
        .context("Failed to communicate with AUR RPC service")?
        .into_json()
        .context("Failed to parse AUR RPC response")?;

    let map: HashMap<String, String> = response
        .results
        .into_iter()
        .map(|p| (p.name, p.version))
        .collect();

    for pkg in packages {
        if !map.contains_key(pkg) {
            bail!("Package '{pkg}' not found on AUR");
        }
    }

    Ok(map)
}
