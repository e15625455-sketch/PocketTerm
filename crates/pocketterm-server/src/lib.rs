//! Local HTTP backend and JNI entry points.

use axum::{Router, routing::get};
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Starts the backend on an ephemeral loopback port and returns its bound address.
pub async fn bind_local() -> std::io::Result<(TcpListener, SocketAddr)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let addr = listener.local_addr()?;
    Ok((listener, addr))
}

/// Minimal health endpoint used while the API is being assembled.
pub fn router() -> Router {
    Router::new().route("/health", get(|| async { "ok" }))
}

/// JNI version queried by Android when loading this library.
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(
    _vm: *mut jni::sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    jni::sys::JNI_VERSION_1_6
}

#[cfg(test)]
mod tests {
    use super::bind_local;

    #[tokio::test]
    async fn binds_to_loopback_on_an_ephemeral_port() {
        let (listener, addr) = bind_local().await.expect("loopback listener should bind");
        assert!(addr.ip().is_loopback());
        assert_ne!(addr.port(), 0);
        drop(listener);
    }
}
