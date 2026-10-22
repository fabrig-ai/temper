//! Resolve configured sign-in transports before starting the accounts component.
use crate::{config, server_name, service, trust};
use skein_lib::List;
use skein_oauth_accounts as driver;

pub(super) fn resolve(engine: &config::Engine) -> Result<service::AccountsRuntime, String> {
    let mut runtime = service::AccountsRuntime {
        providers: List::with_capacity(u32::try_from(engine.llm.providers.len()).expect("provider count")),
        trust_roots: 0,
        trust_root_bytes: 0,
    };
    for provider in &engine.llm.providers {
        let Some(registration) = &provider.sign_in else { continue };
        let endpoint = endpoint(&registration.token, &registration.endpoint_transport, &mut runtime)?;
        let mut origins = List::<driver::Origin>::with_capacity(3);
        for uri in [Some(&*registration.issuer), Some(&*registration.keys), registration.revocation.as_deref()]
            .into_iter()
            .flatten()
        {
            let (origin, _) = origin(uri)?;
            if origins.iter().any(|item| &*item.uri == origin) {
                continue;
            }
            origins
                .push(driver::Origin {
                    uri: origin.into(),
                    endpoint: endpoint_for_origin(origin, &registration.endpoint_transport, &mut runtime)?,
                })
                .expect("three configured origins");
        }
        runtime.providers.push((provider.number, endpoint, origins)).expect("provider count");
    }
    Ok(runtime)
}
fn transport(
    configured: &config::Transport,
    runtime: &mut service::AccountsRuntime,
) -> Result<driver::Transport, String> {
    Ok(match configured {
        config::Transport::Plaintext => driver::Transport::Plaintext,
        config::Transport::Tls { server_name: name, trust: file } => {
            let (trust, count, each) = trust(file.as_deref())?;
            runtime.trust_roots = runtime.trust_roots.checked_add(count).ok_or("trust count")?;
            runtime.trust_root_bytes = runtime.trust_root_bytes.max(each);
            driver::Transport::Tls { server_name: server_name(name)?, trust }
        }
    })
}
fn origin(uri: &[u8]) -> Result<(&[u8], u16), String> {
    let (start, port) = if uri.starts_with(b"https://") {
        (8, 443)
    } else if uri.starts_with(b"http://") {
        (7, 80)
    } else {
        return Err("sign-in endpoint requires HTTP or HTTPS".into());
    };
    let end =
        uri[start..].iter().position(|byte| matches!(byte, b'/' | b'?' | b'#')).map_or(uri.len(), |at| start + at);
    Ok((&uri[..end], port))
}
fn endpoint(
    uri: &[u8],
    configured: &config::Transport,
    runtime: &mut service::AccountsRuntime,
) -> Result<driver::Endpoint, String> {
    let (origin, _) = origin(uri)?;
    endpoint_for_origin(origin, configured, runtime)
}
fn endpoint_for_origin(
    uri: &[u8],
    configured: &config::Transport,
    runtime: &mut service::AccountsRuntime,
) -> Result<driver::Endpoint, String> {
    use std::net::ToSocketAddrs;
    let (_, port) = origin(uri)?;
    let authority =
        std::str::from_utf8(uri).map_err(|why| why.to_string())?.split_once("://").expect("checked scheme").1;
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').ok_or("invalid endpoint host")?;
        let port = match suffix.strip_prefix(':') {
            Some(value) => value.parse::<u16>().map_err(|why| why.to_string())?,
            None if suffix.is_empty() => port,
            None => return Err("invalid endpoint port".into()),
        };
        (host, port)
    } else if let Some((host, value)) = authority.rsplit_once(':') {
        (host, value.parse::<u16>().map_err(|why| why.to_string())?)
    } else {
        (authority, port)
    };
    let address = (host, port)
        .to_socket_addrs()
        .map_err(|why| why.to_string())?
        .next()
        .ok_or("sign-in endpoint has no address")?;
    Ok(driver::Endpoint { address, transport: transport(configured, runtime)? })
}
