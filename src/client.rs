use crate::address::Address;
use crate::consumer::Consumer;
use crate::frame::Frame;
use crate::producer::Producer;
use bytes::Bytes;
use xancode::Codec;
use std::io;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::Mutex;

pub struct Client<A: Address> {
    conn: Arc<Mutex<TcpStream>>,
    _a: PhantomData<fn() -> A>,
}

impl<A: Address> Client<A> {
    pub async fn connect(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let sock = TcpStream::connect(addr).await?;
        Ok(Client {
            conn: Arc::new(Mutex::new(sock)),
            _a: PhantomData,
        })
    }

    pub fn producer<T: Codec + Send + 'static>(&self, addr: A) -> RemoteProducer<T, A> {
        RemoteProducer {
            conn: self.conn.clone(),
            addr,
            _t: PhantomData,
        }
    }

    pub async fn produce<T: Codec>(&self, addr: &A, message: T) -> io::Result<()> {
        let resp = round_trip(
            &self.conn,
            Frame::Produce {
                addr: addr.encode().to_vec(),
                mode: addr.delivery_mode(),
                payload: message.encode().to_vec(),
            },
        )
        .await?;
        match resp {
            Frame::ProduceOk => Ok(()),
            Frame::Err { msg } => Err(io::Error::other(msg)),
            other => Err(unexpected(other)),
        }
    }

    pub async fn consumer<T: Codec + Send>(&self, addr: &A) -> io::Result<RemoteConsumer<T>> {
        let mode = addr.delivery_mode();
        let resp = round_trip(
            &self.conn,
            Frame::Subscribe {
                addr: addr.encode().to_vec(),
                mode,
            },
        )
        .await?;
        let sub_id = match resp {
            Frame::SubscribeOk { sub_id } => sub_id,
            Frame::Err { msg } => return Err(io::Error::other(msg)),
            other => return Err(unexpected(other)),
        };
        Ok(RemoteConsumer {
            conn: self.conn.clone(),
            sub_id,
            _t: PhantomData,
        })
    }
}

pub struct RemoteProducer<T: Codec, A: Address> {
    conn: Arc<Mutex<TcpStream>>,
    addr: A,
    _t: PhantomData<fn() -> T>,
}

#[async_trait::async_trait]
impl<T: Codec + Send + 'static, A: Address + Send + Sync> Producer<T> for RemoteProducer<T, A> {
    async fn produce(&self, message: T) -> io::Result<()> {
        let payload = message.encode().to_vec();
        let resp = round_trip(
            &self.conn,
            Frame::Produce {
                addr: self.addr.encode().to_vec(),
                mode: self.addr.delivery_mode(),
                payload,
            },
        )
        .await?;
        match resp {
            Frame::ProduceOk => Ok(()),
            Frame::Err { msg } => Err(io::Error::other(msg)),
            other => Err(unexpected(other)),
        }
    }
}

pub struct RemoteConsumer<T: Codec> {
    conn: Arc<Mutex<TcpStream>>,
    sub_id: u64,
    _t: PhantomData<fn() -> T>,
}

impl<T: Codec> RemoteConsumer<T> {
    pub fn sub_id(&self) -> u64 {
        self.sub_id
    }
}

impl<T: Codec> Drop for RemoteConsumer<T> {
    fn drop(&mut self) {
        if tokio::runtime::Handle::try_current().is_ok() {
            let conn = self.conn.clone();
            let sub_id = self.sub_id;
            tokio::spawn(async move {
                let _ = round_trip(&conn, Frame::Unsubscribe { sub_id }).await;
            });
        }
    }
}

#[async_trait::async_trait]
impl<T: Codec + Send> Consumer<T> for RemoteConsumer<T> {
    async fn consume(&self) -> io::Result<Option<T>> {
        let resp = round_trip(&self.conn, Frame::Consume { sub_id: self.sub_id }).await?;
        match resp {
            Frame::ConsumeOk { payload: None } => Ok(None),
            Frame::ConsumeOk {
                payload: Some(bytes),
            } => match T::decode(&Bytes::from(bytes)) {
                Ok(m) => Ok(Some(m)),
                Err(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "decode failed")),
            },
            Frame::Err { msg } => Err(io::Error::other(msg)),
            other => Err(unexpected(other)),
        }
    }
}

async fn round_trip(conn: &Mutex<TcpStream>, frame: Frame) -> io::Result<Frame> {
    let mut sock = conn.lock().await;
    frame.write_to(&mut *sock).await?;
    Frame::read_from(&mut *sock).await
}

fn unexpected(f: Frame) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unexpected frame: {f:?}"),
    )
}
