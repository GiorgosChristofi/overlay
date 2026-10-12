//! Serves an axum router over TLS (with WebSocket upgrades) or plain HTTP.

use axum::Router;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Accepts connections until the task is dropped. `tls: None` serves plain HTTP.
pub async fn serve(listener: TcpListener, router: Router, tls: Option<Arc<rustls::ServerConfig>>) {
    let acceptor = tls.map(TlsAcceptor::from);
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            continue;
        };
        let router = router.clone();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            match acceptor {
                Some(acceptor) => match acceptor.accept(stream).await {
                    Ok(tls) => connection(TokioIo::new(tls), router, peer).await,
                    // Clients that reject our certificate end up here.
                    Err(e) => tracing::debug!(%peer, error = %e, "TLS handshake failed"),
                },
                None => connection(TokioIo::new(stream), router, peer).await,
            }
        });
    }
}

async fn connection<I>(io: TokioIo<I>, router: Router, peer: SocketAddr)
where
    I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let service = TowerToHyperService::new(router);
    if let Err(e) = hyper::server::conn::http1::Builder::new()
        .serve_connection(io, service)
        .with_upgrades()
        .await
    {
        tracing::debug!(%peer, error = %e, "connection ended with an error");
    }
}
