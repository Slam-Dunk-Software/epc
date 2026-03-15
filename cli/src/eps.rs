use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Minimal subset of eps.toml that EPC cares about.
/// Full manifest spec lives in ADR-0003.
#[derive(Debug, Deserialize)]
pub struct EpsManifest {
    pub package: PackageMeta,
    pub service: Option<ServiceConfig>,
}

#[derive(Debug, Deserialize)]
pub struct PackageMeta {
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    #[serde(default)]
    pub enabled: bool,
    pub start: String,
    pub port: u16,
    pub health_check: Option<String>,
}

impl EpsManifest {
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Returns the service config or an error if the package is not deployable.
    pub fn require_service(&self) -> Result<&ServiceConfig> {
        match &self.service {
            Some(svc) if svc.enabled => Ok(svc),
            Some(_) => bail!(
                "'{}' has a [service] block but enabled = false",
                self.package.name
            ),
            None => bail!(
                "'{}' has no [service] block in eps.toml — not deployable via epc",
                self.package.name
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_toml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parse_minimal_manifest() {
        let f = write_toml(
            r#"
            [package]
            name = "my_pkg"
            version = "0.1.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""
            "#,
        );
        let m = EpsManifest::from_file(f.path()).unwrap();
        assert_eq!(m.package.name, "my_pkg");
        assert!(m.service.is_none());
    }

    #[test]
    fn parse_manifest_with_service() {
        let f = write_toml(
            r#"
            [package]
            name = "my_pkg"
            version = "0.2.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""

            [service]
            enabled = true
            start = "./serve.sh"
            port = 9000
            "#,
        );
        let m = EpsManifest::from_file(f.path()).unwrap();
        let svc = m.service.unwrap();
        assert!(svc.enabled);
        assert_eq!(svc.start, "./serve.sh");
        assert_eq!(svc.port, 9000);
    }

    #[test]
    fn require_service_errors_when_missing() {
        let f = write_toml(
            r#"
            [package]
            name = "no_service"
            version = "0.1.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""
            "#,
        );
        let m = EpsManifest::from_file(f.path()).unwrap();
        let err = m.require_service().unwrap_err();
        assert!(err.to_string().contains("no [service] block"));
    }

    #[test]
    fn require_service_errors_when_disabled() {
        let f = write_toml(
            r#"
            [package]
            name = "disabled_svc"
            version = "0.1.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""

            [service]
            enabled = false
            start = "./serve.sh"
            port = 9000
            "#,
        );
        let m = EpsManifest::from_file(f.path()).unwrap();
        let err = m.require_service().unwrap_err();
        assert!(err.to_string().contains("enabled = false"));
    }

    #[test]
    fn from_file_errors_on_missing_file() {
        let err = EpsManifest::from_file(std::path::Path::new("/no/such/file.toml")).unwrap_err();
        assert!(err.to_string().contains("failed to read"));
    }

    #[test]
    fn from_file_errors_on_bad_toml() {
        let f = write_toml("not valid toml ][[[");
        let err = EpsManifest::from_file(f.path()).unwrap_err();
        assert!(err.to_string().contains("failed to parse"));
    }
}
