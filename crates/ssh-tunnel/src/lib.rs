//! SSH local port-forwarding so a connection can reach a database that only listens on a
//! private network (the common "bastion host" setup). Opens one SSH session, then bridges
//! every TCP connection accepted on a local ephemeral port to a fresh `direct-tcpip`
//! channel on that session, targeting the real database host:port — exactly what `ssh -L`
//! does, minus the shell.
use russh::keys::known_hosts;
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;

#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("ssh error: {0}")]
    Ssh(#[from] russh::Error),
    #[error("ssh key error: {0}")]
    Key(#[from] russh::keys::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ssh authentication failed")]
    AuthFailed,
    #[error("could not resolve a local data directory to store the SSH known-hosts file")]
    NoDataDir,
}

pub type TunnelResult<T> = Result<T, TunnelError>;

#[derive(Debug, Clone)]
pub enum SshAuth {
    Password(String),
    PrivateKey {
        path: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct SshTunnelSpec {
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub auth: SshAuth,
    pub remote_host: String,
    pub remote_port: u16,
}

/// Verifies the SSH server's host key on every connect, using an OpenSSH-format
/// known_hosts file private to this app (not `~/.ssh/known_hosts` — we don't want to read
/// or write the user's own SSH client state). Trust-on-first-use: the first time we see a
/// given host:port, its key is recorded and accepted; every later connection must present
/// that exact key. A recorded key that doesn't match (`KeyChanged`, or a second entry
/// under a different algorithm) fails the connection instead of silently accepting it —
/// that mismatch is exactly the signal a man-in-the-middle would produce.
///
/// This only covers plain host keys, not SSH certificate-based host auth — a server that
/// presents a certificate is rejected, since our known_hosts store has no way to express
/// "trust this CA" and accepting the embedded key would skip verification entirely.
struct HostKeyVerifier {
    host: String,
    port: u16,
    known_hosts_path: PathBuf,
}

impl russh::client::Handler for HostKeyVerifier {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = server_public_key else {
            return Ok(false);
        };

        match known_hosts::check_known_hosts_path(
            &self.host,
            self.port,
            key,
            &self.known_hosts_path,
        ) {
            Ok(true) => Ok(true),
            Ok(false) => {
                // `Ok(false)` also covers "nothing recorded for this host at all" — only
                // trust-on-first-use in that exact case. An existing entry under a
                // different algorithm is treated as suspicious rather than upgraded.
                let recorded = known_hosts::known_host_keys_path(
                    &self.host,
                    self.port,
                    &self.known_hosts_path,
                )
                .unwrap_or_default();
                if recorded.is_empty() {
                    let _ = known_hosts::learn_known_hosts_path(
                        &self.host,
                        self.port,
                        key,
                        &self.known_hosts_path,
                    );
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            // `KeyChanged`: recorded under the same algorithm but with different key
            // bytes — the strongest MITM signal there is.
            Err(_) => Ok(false),
        }
    }
}

/// Where per-app SSH host keys are recorded, e.g.
/// `~/Library/Application Support/structly/ssh_known_hosts` on macOS. Independent of the
/// Tauri app handle (this crate doesn't depend on Tauri) — same pattern as `secrets`
/// hardcoding its own keychain service name.
fn default_known_hosts_path() -> TunnelResult<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or(TunnelError::NoDataDir)?
        .join("structly");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("ssh_known_hosts"))
}

/// A live tunnel: `local_port` is where the app should point its DB driver instead of
/// `remote_host:remote_port`. Dropping this closes the accept loop and the SSH session.
pub struct SshTunnel {
    pub local_port: u16,
    accept_loop: tokio::task::JoinHandle<()>,
    _session: Arc<russh::client::Handle<HostKeyVerifier>>,
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.accept_loop.abort();
    }
}

pub async fn open_tunnel(spec: &SshTunnelSpec) -> TunnelResult<SshTunnel> {
    let handler = HostKeyVerifier {
        host: spec.ssh_host.clone(),
        port: spec.ssh_port,
        known_hosts_path: default_known_hosts_path()?,
    };
    let config = Arc::new(russh::client::Config::default());
    let mut session =
        russh::client::connect(config, (spec.ssh_host.as_str(), spec.ssh_port), handler).await?;

    let auth_result = match &spec.auth {
        SshAuth::Password(password) => {
            session
                .authenticate_password(&spec.ssh_user, password.clone())
                .await?
        }
        SshAuth::PrivateKey { path, passphrase } => {
            let key = load_secret_key(path, passphrase.as_deref())?;
            let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);
            session
                .authenticate_publickey(&spec.ssh_user, key_with_alg)
                .await?
        }
    };
    if !auth_result.success() {
        return Err(TunnelError::AuthFailed);
    }

    let session = Arc::new(session);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();

    let remote_host = spec.remote_host.clone();
    let remote_port = spec.remote_port;
    let session_for_loop = session.clone();
    let accept_loop = tokio::spawn(async move {
        loop {
            let (local_stream, peer_addr) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => break,
            };
            let session = session_for_loop.clone();
            let remote_host = remote_host.clone();
            // Each accepted local connection gets its own SSH channel — mirrors `ssh -L`,
            // and lets several concurrent DB connections share one tunnel.
            tokio::spawn(async move {
                let channel = match session
                    .channel_open_direct_tcpip(
                        remote_host,
                        remote_port as u32,
                        peer_addr.ip().to_string(),
                        peer_addr.port() as u32,
                    )
                    .await
                {
                    Ok(channel) => channel,
                    Err(_) => return,
                };
                let mut ssh_stream = channel.into_stream();
                let mut local_stream = local_stream;
                let _ = tokio::io::copy_bidirectional(&mut local_stream, &mut ssh_stream).await;
            });
        }
    });

    Ok(SshTunnel {
        local_port,
        accept_loop,
        _session: session,
    })
}

/// Opens a tunnel if `tunnel_config` is `Some`, mapping `db_core`'s engine-agnostic
/// `SshTunnelConfig` (+ the secret the Tauri command layer already looked up from the
/// keychain) into a `SshTunnelSpec`. Lets each DB driver's `connect()` stay a one-liner:
/// `if let Some(tunnel) = ssh_tunnel::maybe_open_tunnel(...).await? { use 127.0.0.1:tunnel.local_port }`.
pub async fn maybe_open_tunnel(
    tunnel_config: Option<&db_core::SshTunnelConfig>,
    ssh_secret: Option<&str>,
    remote_host: &str,
    remote_port: u16,
) -> TunnelResult<Option<SshTunnel>> {
    let Some(cfg) = tunnel_config else {
        return Ok(None);
    };

    let auth = match &cfg.auth {
        db_core::SshAuthMethod::Password => {
            SshAuth::Password(ssh_secret.unwrap_or_default().to_string())
        }
        db_core::SshAuthMethod::PrivateKey { path } => SshAuth::PrivateKey {
            path: path.clone(),
            passphrase: ssh_secret.map(str::to_string),
        },
    };

    let spec = SshTunnelSpec {
        ssh_host: cfg.host.clone(),
        ssh_port: cfg.port,
        ssh_user: cfg.username.clone(),
        auth,
        remote_host: remote_host.to_string(),
        remote_port,
    };

    Ok(Some(open_tunnel(&spec).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::client::Handler;
    use russh::keys::parse_public_key_base64;

    // Two distinct ed25519 keys (same fixtures russh's own known_hosts tests use).
    const KEY_A: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ";
    const KEY_B: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIA6rWI3G2sz07DnfFlrouTcysQlj2P+jpNSOEWD9OJ3X";

    fn verifier(known_hosts_path: PathBuf) -> HostKeyVerifier {
        HostKeyVerifier {
            host: "bastion.example.com".into(),
            port: 22,
            known_hosts_path,
        }
    }

    fn server_key(base64: &str) -> PublicKeyOrCertificate {
        PublicKeyOrCertificate::PublicKey {
            key: parse_public_key_base64(base64).unwrap(),
            hash_alg: None,
        }
    }

    #[tokio::test]
    async fn trusts_and_remembers_a_new_host() {
        let dir = tempfile::tempdir().unwrap();
        let mut verifier = verifier(dir.path().join("known_hosts"));

        assert!(verifier.check_server_key(&server_key(KEY_A)).await.unwrap());
        // Second connection presenting the same, now-recorded key: still trusted.
        assert!(verifier.check_server_key(&server_key(KEY_A)).await.unwrap());
    }

    #[tokio::test]
    async fn rejects_a_host_key_that_changed_after_first_trust() {
        let dir = tempfile::tempdir().unwrap();
        let mut verifier = verifier(dir.path().join("known_hosts"));

        assert!(verifier.check_server_key(&server_key(KEY_A)).await.unwrap());
        // Same host, a different key presented later — the MITM signal — must be
        // rejected, not silently accepted or silently re-learned.
        assert!(!verifier.check_server_key(&server_key(KEY_B)).await.unwrap());
    }
}
