//! Resolve the four configured issuer hosts before starting any sign-in IO.
use crate::{server_name, service, trust};
use service::clients::fetch::{Endpoint, Runtime, Transport};
use skein_lib::List;
use std::net::ToSocketAddrs;
pub(super) fn resolve(sign_in: Option<&service::startup::SignIn>) -> Result<Runtime, String> {
    let mut runtime = Runtime { endpoints: List::<Endpoint>::with_capacity(4) };
    let Some(sign_in) = sign_in else { return Ok(runtime) };
    for (name, uri) in sign_in.endpoints.values() {
        let field = format!("application.sign_in.google.endpoints.{}", String::from_utf8_lossy(name));
        let (origin, host, port, tls) =
            service::startup::sign_in_url(uri).ok_or_else(|| format!("{field}: invalid URL"))?;
        let hostname = std::str::from_utf8(host).map_err(|why| format!("{field}: {why}"))?;
        let address = (hostname, port)
            .to_socket_addrs()
            .map_err(|why| format!("{field}: {hostname}: {why}"))?
            .next()
            .ok_or_else(|| format!("{field}: {hostname}: no addresses"))?;
        let transport = match &sign_in.endpoints.transport {
            Some(service::startup::Transport::Plaintext) => {
                if tls || !address.ip().is_loopback() {
                    return Err(format!("{field}: plaintext requires loopback"));
                }
                Transport::Plaintext
            }
            Some(service::startup::Transport::Tls { server_name: name, trust: file }) => {
                let trust = trust(file.as_deref()).map_err(|why| format!("{field}.transport: {why}"))?;
                Transport::Tls {
                    server_name: server_name(name)
                        .map_err(|why| format!("{field}.transport.tls.server_name: {why}"))?,
                    trust,
                }
            }
            None => {
                let trust = trust(None).map_err(|why| format!("{field}: {why}"))?;
                Transport::Tls { server_name: server_name(host).map_err(|why| format!("{field}: {why}"))?, trust }
            }
        };
        if runtime.endpoints.iter().any(|entry| entry.origin.as_ref() == origin) {
            continue;
        }
        runtime
            .endpoints
            .push(Endpoint { origin: origin.into(), address, transport })
            .expect("four configured origins");
    }
    Ok(runtime)
}
