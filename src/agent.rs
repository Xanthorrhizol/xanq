use crate::frame::Frame;
use crate::server::ServerInner;
use bytes::Bytes;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use tokio::net::TcpStream;

pub struct Agent {
    server: Arc<ServerInner>,
    sock: TcpStream,
    owned_subs: HashSet<u64>,
}

impl Agent {
    pub(crate) fn new(server: Arc<ServerInner>, sock: TcpStream) -> Self {
        Agent {
            server,
            sock,
            owned_subs: HashSet::new(),
        }
    }

    pub async fn run(mut self) -> io::Result<()> {
        let result = self.serve().await;
        for sub_id in self.owned_subs.drain() {
            self.server.unsubscribe(sub_id).await;
        }
        result
    }

    async fn serve(&mut self) -> io::Result<()> {
        loop {
            let frame = match Frame::read_from(&mut self.sock).await {
                Ok(f) => f,
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            };

            if let Some(resp) = self.handle(frame).await {
                resp.write_to(&mut self.sock).await?;
            }
        }
    }

    async fn handle(&mut self, frame: Frame) -> Option<Frame> {
        match frame {
            Frame::Produce {
                addr,
                mode,
                payload,
            } => match self
                .server
                .produce(&addr, mode, Bytes::from(payload))
                .await
            {
                Ok(()) => Some(Frame::ProduceOk),
                Err(e) => Some(Frame::Err {
                    msg: e.to_string(),
                }),
            },
            Frame::Subscribe { addr, mode } => match self.server.subscribe(&addr, mode).await {
                Ok(sub_id) => {
                    self.owned_subs.insert(sub_id);
                    Some(Frame::SubscribeOk { sub_id })
                }
                Err(e) => Some(Frame::Err {
                    msg: e.to_string(),
                }),
            },
            Frame::Consume { sub_id } => {
                if !self.owned_subs.contains(&sub_id) {
                    return Some(Frame::Err {
                        msg: format!("sub_id {sub_id} not owned by this connection"),
                    });
                }
                match self.server.consume(sub_id).await {
                    Ok(payload) => Some(Frame::ConsumeOk {
                        payload: payload.map(|b| b.to_vec()),
                    }),
                    Err(e) => Some(Frame::Err {
                        msg: e.to_string(),
                    }),
                }
            }
            Frame::Unsubscribe { sub_id } => {
                if self.owned_subs.remove(&sub_id) {
                    self.server.unsubscribe(sub_id).await;
                }
                Some(Frame::UnsubscribeOk)
            }
            Frame::ProduceOk
            | Frame::SubscribeOk { .. }
            | Frame::ConsumeOk { .. }
            | Frame::UnsubscribeOk
            | Frame::Err { .. } => Some(Frame::Err {
                msg: "unexpected server-direction frame from client".into(),
            }),
        }
    }
}
