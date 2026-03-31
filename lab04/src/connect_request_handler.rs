use anyhow;
use bytes::Bytes;
use http::{Request, Response};
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;


pub fn handle_connect_request(
    req: Request<hyper::body::Incoming>,
) -> anyhow::Result<http::Response<BoxBody<Bytes, anyhow::Error>>> {
    if let Some(addr) = host_addr(req.uri()) {
        tokio::task::spawn(async move {
            let uri = req.uri().clone();
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    if let Err(e) = tunnel(upgraded, addr).await {
                        log::debug!("Server {} io error: {}", uri, e);
                    };
                }
                Err(e) => log::error!("Upgrade of {} error: {}", uri, e),
            }
        });

        Ok(create_empty_response())
    } else {
        log::error!("CONNECT host is not socket addr: {}", req.uri());
        let mut resp = create_response("CONNECT must be to a socket address");
        *resp.status_mut() = http::StatusCode::BAD_REQUEST;

        Ok(resp)
    }
}

fn host_addr(uri: &http::Uri) -> Option<String> {
    uri.authority().map(|auth| auth.to_string())
}

fn create_empty_response() -> Response<BoxBody<Bytes, anyhow::Error>> {
    Response::new(
        Empty::<Bytes>::new()
            .map_err(|never| match never {})
            .boxed(),
    )
}

fn create_response<T: Into<Bytes>>(chunk: T) -> Response<BoxBody<Bytes, anyhow::Error>> {
    Response::new(
        Full::new(chunk.into())
            .map_err(|never| match never {})
            .boxed(),
    )
}

async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);

    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    log::debug!(
        "Client wrote {} bytes and received {} bytes",
        from_client,
        from_server,
    );

    Ok(())
}
