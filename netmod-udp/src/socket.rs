//! Socket handler module

use crate::{AddrTable, Envelope, FrameExt, Peer};
use async_notify::Notify;
use async_std::{
    future::{self, Future},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    pin::Pin,
    sync::{Arc, RwLock},
    task::{self, Poll},
};
use conjoiner;
use netmod::{Frame, Target};
use std::collections::VecDeque;

const MULTI: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
const SELF: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

/// Wraps around a UDP socket an the input queue
pub(crate) struct Socket {
    sock: Arc<UdpSocket>,
    inbox: Arc<RwLock<Notify<VecDeque<FrameExt>>>>,
}

impl Socket {
    /// Create a new socket handler and return a management reference
    pub(crate) async fn with_addr(addr: &str, table: Arc<AddrTable>) -> Arc<Self> {
        let sock = UdpSocket::bind(addr).await.unwrap();
        sock.join_multicast_v4(MULTI, SELF)
            .expect("Failed to join multicast. Error");

        // sock.set_multicast_loop_v4(true).unwrap();

        let arc = Arc::new(Self {
            sock: Arc::new(sock),
            inbox: Default::default(),
        });

        Self::incoming_handle(Arc::clone(&arc), table);
        arc.multicast(Envelope::Announce).await;
        dbg!();
        arc
    }

    /// Send a message to one specific client
    pub(crate) async fn send(&self, frame: &Frame, peer: Peer) {
        let data = Envelope::frame(frame);
        self.sock
            .send_to(&data, SocketAddr::new(peer.ip, peer.port))
            .await
            .unwrap();
    }

    /// Send a frame to many recipients (via multicast)
    pub(crate) async fn send_many(&self, frame: &Frame, ips: Vec<Peer>) {
        let data = Envelope::frame(frame);
        for peer in ips.iter() {
            self.send(frame, *peer).await
        }
    }

    /// Send a multicast with an Envelope
    pub(crate) async fn multicast(&self, env: Envelope) {
        dbg!("Sending mulitcast {}", &env);
        self.sock
            .send_to(
                &dbg!(env.as_bytes()),
                SocketAddr::new(IpAddr::V4(MULTI.clone()), 12322),
            )
            .await;
    }

    pub(crate) async fn next(&self) -> FrameExt {
        future::poll_fn(|ctx| {
            let lock = &mut self.inbox.write();
            match unsafe { Pin::new_unchecked(lock).poll(ctx) } {
                Poll::Ready(ref mut inc) => match inc.pop_front() {
                    Some(f) => Poll::Ready(f),
                    None => {
                        Notify::register_waker(inc, ctx.waker());
                        Poll::Pending
                    }
                },
                Poll::Pending => Poll::Pending,
            }
        })
        .await
    }

    fn incoming_handle(arc: Arc<Self>, table: Arc<AddrTable>) {
        task::spawn(async move {
            loop {
                // This is a bad idea
                let mut buf = vec![0; 8192];

                match arc.sock.recv_from(&mut buf).await {
                    Ok((_, peer)) => {
                        let env = Envelope::from_bytes(&buf);
                        match env {
                            Envelope::Announce => {
                                dbg!("Receiving announce");
                                table.set(peer).await;
                                arc.multicast(Envelope::Reply).await;
                            }
                            Envelope::Reply => {
                                dbg!("Receiving announce");
                                table.set(peer).await;
                            }
                            Envelope::Data(_) => {
                                let frame = env.get_frame();
                                dbg!(&frame);

                                let id = table.id(dbg!(peer.into())).await.unwrap();

                                // Append to the inbox and wake
                                let mut inbox = arc.inbox.write().await;
                                inbox.push_back(FrameExt(frame, Target::Single(id)));
                                Notify::wake(&mut inbox);
                            }
                        }
                    }
                    val => {
                        // TODO: handle errors more gracefully
                        dbg!(val).expect("Crashed UDP thread!");
                    }
                }
            }
        });
    }
}

#[test]
fn test_init() {
    task::block_on(async move {
        let table = Arc::new(AddrTable::new());
        let sock = Socket::with_addr("0.0.0.0:12322", table).await;
        println!("Multicasting");
        sock.multicast(Envelope::Announce);
    });
}

#[test]
fn test_single_unicast() {
    task::block_on(async {
        let p1 = Peer {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 10000,
        };
        let p2 = Peer {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 10001,
        };

        let t1 = Arc::new(AddrTable::new());
        let t2 = Arc::new(AddrTable::new());

        // This is a hack for this test to "introduce" the two
        // endpoints to each other.  It's the "Haaaave you met..." of
        // wire protocols
        t1.set(p2).await;
        t2.set(p1).await;

        // Create two sockets on two ports
        let s1 = Socket::with_addr(&p1.to_string(), t1).await;
        let s2 = Socket::with_addr(&p2.to_string(), t2).await;

        let f = Frame::dummy();
        s1.send(&f, p2).await;

        assert_eq!(s2.next().await.0, f);
    });
}
