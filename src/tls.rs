use std::path::PathBuf;
use std::process::Command;
use tracing::info;

/// Resolve TLS certificate and key paths.
/// If explicit paths are given, use those.
/// Otherwise, look for mkcert certs in the CAROOT, generating if needed.
pub fn resolve_certs(
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
) -> Result<(PathBuf, PathBuf), String> {
    if let (Some(c), Some(k)) = (cert, key) {
        if !c.exists() {
            return Err(format!("Certificate file not found: {}", c.display()));
        }
        if !k.exists() {
            return Err(format!("Key file not found: {}", k.display()));
        }
        return Ok((c, k));
    }

    // Find mkcert CAROOT
    let caroot_output = Command::new("mkcert")
        .arg("-CAROOT")
        .output()
        .map_err(|_| "mkcert is not installed. Install it with: brew install mkcert".to_string())?;

    if !caroot_output.status.success() {
        return Err("Failed to run mkcert -CAROOT".to_string());
    }

    let caroot = String::from_utf8_lossy(&caroot_output.stdout)
        .trim()
        .to_string();
    let caroot = PathBuf::from(caroot);

    let cert_path = caroot.join("localhost.pem");
    let key_path = caroot.join("localhost-key.pem");

    if cert_path.exists() && key_path.exists() {
        info!(
            cert = %cert_path.display(),
            key = %key_path.display(),
            "Using existing mkcert certificates"
        );
        return Ok((cert_path, key_path));
    }

    // Ensure mkcert CA is initialized
    if !caroot.exists() || !caroot.join("rootCA.pem").exists() {
        return Err(format!(
            "mkcert CA not initialized. Run: mkcert -install\n\
             Then retry with --tls."
        ));
    }

    // Generate certs in CAROOT
    info!("Generating mkcert certificates for localhost");
    let gen_output = Command::new("mkcert")
        .current_dir(&caroot)
        .arg("localhost")
        .output()
        .map_err(|e| format!("Failed to run mkcert: {e}"))?;

    if !gen_output.status.success() {
        let stderr = String::from_utf8_lossy(&gen_output.stderr);
        return Err(format!("mkcert failed: {stderr}"));
    }

    if cert_path.exists() && key_path.exists() {
        info!(
            cert = %cert_path.display(),
            key = %key_path.display(),
            "Generated mkcert certificates"
        );
        Ok((cert_path, key_path))
    } else {
        Err("mkcert ran but certificates not found at expected paths".to_string())
    }
}
