//! The checked configuration's steps, supervised on one skein-io loop.
use skein_io::kernel::{Exit, Fd};
use skein_shell::{Clock, Config, Host, Kernel};
use std::os::unix::ffi::OsStrExt;
use temper_box::host::{Service, Settings};

/// Checks the document before starting its steps; refusals contain unprefixed lines.
pub fn run(directories: &super::Directories) -> Result<Exit, String> {
    let (bytes, held) = configuration(directories)?;
    let mut owned = held.iter().map(|(fd, _)| *fd).collect::<Vec<_>>();
    let opened = (|| {
        let super::setup::Deployment { named, checked, demo, limits } =
            super::setup::check(&bytes, &directories.jig())?;
        let directory = super::path(&directories.data);
        let program = std::env::current_exe().map_err(|why| why.to_string())?;
        let environment = std::env::vars_os()
            .map(|(name, value)| {
                let mut entry = name.as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_bytes());
                entry.into_boxed_slice()
            })
            .collect();
        let mut serve = vec![super::address(&named.clients)?];
        if let Some(webhooks) = &named.webhooks {
            serve.push(super::address(&webhooks.listener)?);
        }
        let steps = temper_fake::steps(&checked.setup, demo.as_ref(), &directories.flags(), serve.into_boxed_slice());
        let mut listeners = Vec::new();
        for step in
            steps.always.iter().chain(steps.asked.iter().flat_map(|group| group.steps.iter().map(|(_, step)| step)))
        {
            for addr in step.listeners() {
                if listeners.iter().any(|(_, previous)| previous == addr) {
                    return Err(format!(
                        "{}: port {} ({addr}): two steps name the same listener",
                        directories.document().display(),
                        addr.port()
                    ));
                }
                let fd = if let Some((fd, _)) = held.iter().find(|(_, port)| port.get() == addr.port()) {
                    *fd
                } else {
                    let (fd, _) = skein_shell::open_listener(
                        *addr,
                        jig_protocol_config::profile(jig_protocol_config::Profile::Small).io.backlog,
                    )
                    .map_err(|why| {
                        format!(
                            "{}: port {} ({addr}): cannot listen: {why:?}",
                            directories.document().display(),
                            addr.port()
                        )
                    })?;
                    owned.push(fd);
                    fd
                };
                listeners.push((fd, *addr));
            }
        }
        // Forgejo's port must be free for Forgejo to bind (M3a, protocol/setup.md 7.1).
        for (fd, _) in held {
            if !listeners.iter().any(|(listener, _)| *listener == fd) {
                super::close(vec![fd]);
                owned.retain(|owned| *owned != fd);
            }
        }
        let signals = skein_shell::open_termination_signals().map_err(|errno| format!("signals: errno {errno}"))?;
        owned.push(signals);
        let root = skein_shell::open_root(directory).map_err(|errno| format!("directory: errno {errno}"))?;
        owned.push(root);
        let settings = Settings {
            program: program.as_os_str().as_bytes().into(),
            environment,
            listeners: listeners.into_boxed_slice(),
            serve: b"serve".as_slice().into(),
            root,
            signals,
            stdout: Fd::new(1),
            stderr: Fd::new(2),
        };
        let service = Service::new(&limits, steps, settings).map_err(|why| format!("box limits: {why:?}"))?;
        let kernel = Kernel::open(Config { operations: service.operations() }).map_err(|why| why.to_string())?;
        Ok((service, kernel))
    })();
    let (mut service, mut kernel) = match opened {
        Ok(opened) => opened,
        Err(why) => {
            super::close(owned);
            return Err(why);
        }
    };
    Ok(skein_shell::drive(&mut kernel, &Clock::new(), &mut service))
}

type Configuration = (Box<[u8]>, Vec<(Fd, std::num::NonZeroU16)>);
/// Prepare a first configuration, keeping init's listeners, or read the existing one.
pub fn configuration(directories: &super::Directories) -> Result<Configuration, String> {
    let config = super::path(&directories.config);
    let empty = match std::fs::read_dir(config) {
        Ok(mut entries) => match entries.next() {
            None => true,
            Some(Ok(_)) => false,
            Some(Err(why)) => return Err(format!("{}: {why}", config.display())),
        },
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => true,
        Err(why) => return Err(format!("{}: {why}", config.display())),
    };
    let mut held = Vec::new();
    if empty {
        held = super::init::core(directories)?.listeners;
        eprintln!("box: wrote the configuration, {}", directories.document().display());
    }
    match directories.read() {
        Ok(bytes) => Ok((bytes, held)),
        Err(why) => {
            super::close(held.into_iter().map(|(fd, _)| fd).collect());
            Err(why)
        }
    }
}
