//! Module only loaded when Ratman is running as a daemon

mod error;
mod parse;
mod state;
mod transform;

use std::net::SocketAddr;

use crate::{Message, Recipient, Router};
use async_std::{net::TcpListener, task::spawn};
use error::Result;
use state::{DaemonState, OnlineMap};

pub(crate) fn elog<S: Into<String>>(msg: S, code: u16) -> ! {
    error!("[FATAL!] {}", msg.into());
    std::process::exit(code.into());
}

async fn run_relay(r: Router, online: OnlineMap) {
    loop {
        let Message {
            id,
            sender,
            recipient,
            payload,
            timesig,
            sign,
        } = r.next().await;
        let recv = types::api::receive_default(types::message::received(
            id,
            sender,
            match recipient {
                Recipient::User(id) => Some(id),
                Recipient::Flood => None,
            },
            payload,
            format!("{:?}", timesig),
            sign,
        ));

        match recipient {
            Recipient::User(ref id) => {
                if let Some(io) = online.lock().await.get(id) {
                    let mut io = io.lock().await;
                    if let Err(e) = parse::forward_recv(io.as_io(), recv).await {
                        error!("Failed to forward received message: {}", e);
                    }
                }
            }
            Recipient::Flood => {
                for (_, io) in online.lock().await.iter_mut() {
                    let mut io = io.lock().await;
                    if let Err(e) = parse::forward_recv(io.as_io(), recv.clone()).await {
                        error!("Failed to forward received message: {}", e);
                    }
                }
            }
        }
    }
}

/// Run the daemon!
pub(crate) async fn run(r: Router, addr: SocketAddr) -> Result<()> {
    info!("Listening for API connections on socket {:?}", addr);
    let listener = TcpListener::bind(addr).await?;
    let mut state = DaemonState::new(&listener);
    let online = state.get_online().await;

    let relay = spawn(run_relay(r.clone(), online));

    while let Ok(io) = state.listen_for_connections().await {
        let io = match io {
            Some(io) => io,
            None => continue,
        };

        spawn(parse::parse_stream(r.clone(), io));
    }

    relay.cancel().await;
    Ok(())
}
