//! `winuhid-web` — browser-based control panel for the WinUHid driver.

use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::{prelude::*, EnvFilter};

mod ffi;
mod manager;
mod server;

#[derive(Parser, Debug)]
#[command(
    name = "winuhid-web",
    about = "Web UI for creating and exercising WinUHid virtual devices.",
    version
)]
struct Args {
    /// Address to bind the HTTP server to.
    #[arg(long, env = "WINUHID_WEB_ADDR", default_value = "127.0.0.1:7878")]
    addr: String,

    /// Directory to load `WinUHid.dll` and `WinUHidDevs.dll` from.
    /// Multiple may be specified; the first hit wins.
    #[arg(long = "dll-dir", env = "WINUHID_DLL_DIR")]
    dll_dirs: Vec<PathBuf>,

    /// Optional shared secret required on every REST and WebSocket
    /// request. When set, clients must present the token via either an
    /// `Authorization: Bearer <token>` header or a `?token=<token>`
    /// query parameter (the latter so browsers can attach it to a
    /// `WebSocket` URL, since the WebSocket API can't set headers).
    ///
    /// Binding to a non-loopback `--addr` requires this flag — without
    /// it, anyone on the network could create virtual HID devices on
    /// this machine.
    #[arg(long, env = "WINUHID_WEB_TOKEN")]
    token: Option<String>,
}

/// Returns true when every address `addr` resolves to is loopback.
/// Used to decide whether `--token` is mandatory: if `--addr` listens
/// on anything reachable from outside the local machine, we refuse to
/// start without a token. Hostnames like `localhost` resolve to
/// loopback and are treated accordingly.
fn addr_is_loopback_only(addr: &str) -> Result<bool> {
    let resolved: Vec<_> = addr
        .to_socket_addrs()
        .with_context(|| format!("failed to resolve {addr}"))?
        .collect();
    if resolved.is_empty() {
        bail!("address {addr} did not resolve to any socket address");
    }
    Ok(resolved.iter().all(|a| a.ip().is_loopback()))
}

fn main() -> Result<()> {
    init_tracing();

    let args = Args::parse();

    if !cfg!(windows) {
        bail!("winuhid-web only runs on Windows; the WinUHid driver is Windows-only.");
    }

    // Refuse to start on a non-loopback address without a token before
    // we touch the driver / SDK — there's no point loading WinUHid.dll
    // just to fail validation.
    let loopback_only = addr_is_loopback_only(&args.addr)?;
    if !loopback_only && args.token.is_none() {
        bail!(
            "binding to a non-loopback address requires --token (or WINUHID_WEB_TOKEN). \
             Anyone on the network would otherwise be able to create virtual HID devices \
             on this machine."
        );
    }
    if let Some(t) = args.token.as_deref() {
        if t.is_empty() {
            bail!("--token must not be empty");
        }
    }

    let mut search_dirs = args.dll_dirs.clone();
    if let Ok(d) = std::env::var("WINUHID_DLL_DIR") {
        search_dirs.push(PathBuf::from(d));
    }
    if let Ok(cwd) = std::env::current_dir() {
        search_dirs.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            search_dirs.push(dir.to_path_buf());
        }
    }
    // De-duplicate.
    let mut seen = std::collections::HashSet::new();
    search_dirs.retain(|p| seen.insert(p.clone()));

    let sdk =
        Arc::new(ffi::Sdk::load(&search_dirs).context("failed to load WinUHid native libraries")?);

    let manager = Arc::new(manager::Manager::new(sdk));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to construct tokio runtime")?;

    runtime.block_on(async move {
        let driver_version = manager.driver_version();
        if driver_version == 0 {
            tracing::warn!(
                "WinUHidGetDriverInterfaceVersion returned 0 - the WinUHid kernel driver does not \
                 appear to be installed/loaded. The web UI will start but device creation will \
                 fail until the driver is available."
            );
        } else {
            tracing::info!("WinUHid driver interface version: {driver_version}");
        }
        if !manager.devs_available() {
            tracing::warn!(
                "WinUHidDevs.dll not loaded - the Mouse/PS4/PS5/Xbox One preset tabs will be \
                 disabled in the UI."
            );
        }

        let app = server::router(manager.clone(), args.token.clone());
        let listener = TcpListener::bind(&args.addr)
            .await
            .with_context(|| format!("failed to bind {}", args.addr))?;
        let local = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| args.addr.clone());

        tracing::info!("winuhid-web listening on http://{local}");
        eprintln!("\n  WinUHid web UI ready — open http://{local}/\n");

        let shutdown = tokio::signal::ctrl_c();
        tokio::select! {
            res = axum::serve(listener, app) => {
                if let Err(e) = res {
                    tracing::error!("server error: {e}");
                }
            }
            _ = shutdown => {
                tracing::info!("Ctrl-C received, shutting down");
            }
        }

        // Make sure we tear down all devices before the runtime drops so
        // the WinUHid driver releases its end of every IOCTL pump.
        manager.destroy_all();
        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[cfg(test)]
mod tests {
    use super::addr_is_loopback_only;

    #[test]
    fn ipv4_loopback_is_loopback_only() {
        assert!(addr_is_loopback_only("127.0.0.1:7878").unwrap());
    }

    #[test]
    fn ipv6_loopback_is_loopback_only() {
        assert!(addr_is_loopback_only("[::1]:7878").unwrap());
    }

    #[test]
    fn localhost_hostname_is_loopback_only() {
        // `localhost` should resolve only to loopback addresses on any
        // sane host. If a CI image somehow disagrees, this test will
        // surface that.
        assert!(addr_is_loopback_only("localhost:7878").unwrap());
    }

    #[test]
    fn wildcard_v4_is_not_loopback_only() {
        assert!(!addr_is_loopback_only("0.0.0.0:7878").unwrap());
    }

    #[test]
    fn wildcard_v6_is_not_loopback_only() {
        assert!(!addr_is_loopback_only("[::]:7878").unwrap());
    }

    #[test]
    fn malformed_addr_errors() {
        assert!(addr_is_loopback_only("not a socket addr").is_err());
    }
}
