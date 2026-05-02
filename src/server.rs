use crate::address::{Address, DeliveryMode};
use crate::consumer::Consumer;
use crate::producer::Producer;
use crate::queue::Queue;
use bytes::Bytes;
use xancode::Codec;
use std::collections::HashMap;
use std::io;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

use log::{debug, warn};
use tokio::net::{TcpListener, ToSocketAddrs};

pub struct Server<A: Address> {
    inner: Arc<ServerInner>,
    _a: PhantomData<fn() -> A>,
}

pub(crate) struct ServerInner {
    addresses: Mutex<HashMap<Vec<u8>, AddrSlot>>,
    subscribers: Mutex<HashMap<u64, Subscription>>,
    next_sub_id: AtomicU64,
}

enum AddrSlot {
    Anycast {
        queue: Arc<Mutex<Queue>>,
        sub_count: usize,
    },
    Broadcast {
        subs: HashMap<u64, Arc<Mutex<Queue>>>,
    },
}

struct Subscription {
    addr: Vec<u8>,
    queue: Arc<Mutex<Queue>>,
    mode: DeliveryMode,
}

impl<A: Address> Default for Server<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address> Server<A> {
    pub fn new() -> Self {
        Server {
            inner: Arc::new(ServerInner {
                addresses: Mutex::new(HashMap::new()),
                subscribers: Mutex::new(HashMap::new()),
                next_sub_id: AtomicU64::new(1),
            }),
            _a: PhantomData,
        }
    }

    pub fn producer<T: Codec + Send + 'static>(&self, addr: A) -> LocalProducer<T, A> {
        LocalProducer {
            inner: self.inner.clone(),
            addr,
            _t: PhantomData,
        }
    }

    pub async fn produce<T: Codec>(&self, addr: &A, message: T) -> io::Result<()> {
        self.inner
            .produce(&addr.encode(), addr.delivery_mode(), message.encode())
            .await
    }

    pub async fn consumer<T: Codec + Send>(&self, addr: &A) -> io::Result<LocalConsumer<T>> {
        let mode = addr.delivery_mode();
        let sub_id = self.inner.subscribe(&addr.encode(), mode).await?;
        Ok(LocalConsumer {
            inner: self.inner.clone(),
            sub_id,
            _t: PhantomData,
        })
    }

    pub async fn listen(&self, addr: impl ToSocketAddrs) -> io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        self.serve(listener).await
    }

    /// Bind, spawn the accept loop on the current Tokio runtime, and return an
    /// Arc-shared server handle along with the actual bound address. Useful for
    /// tests (port 0 → real port) and for in-process embedding where the host
    /// also wants to call `produce`/`consumer` directly on the returned Arc.
    pub async fn spawn(bind: impl ToSocketAddrs) -> io::Result<(Arc<Self>, std::net::SocketAddr)>
    where
        A: 'static,
    {
        let listener = TcpListener::bind(bind).await?;
        let local_addr = listener.local_addr()?;
        let server = Arc::new(Self::new());
        let s = server.clone();
        tokio::spawn(async move {
            if let Err(e) = s.serve(listener).await {
                warn!("server accept loop terminated: {e}");
            }
        });
        Ok((server, local_addr))
    }

    pub async fn serve(&self, listener: TcpListener) -> io::Result<()> {
        loop {
            let (sock, peer) = listener.accept().await?;
            debug!("accepted connection from {peer}");
            let agent = crate::agent::Agent::new(self.inner.clone(), sock);
            tokio::spawn(async move {
                if let Err(e) = agent.run().await {
                    warn!("agent disconnected: {e}");
                }
            });
        }
    }
}

impl ServerInner {
    pub(crate) async fn produce(
        &self,
        addr: &[u8],
        mode: DeliveryMode,
        payload: Bytes,
    ) -> io::Result<()> {
        let mut addresses = self.addresses.lock().await;
        let slot = addresses
            .entry(addr.to_vec())
            .or_insert_with(|| empty_slot(mode));

        match (slot, mode) {
            (AddrSlot::Anycast { queue, .. }, DeliveryMode::Anycast) => {
                let q = queue.clone();
                drop(addresses);
                q.lock().await.push_back(payload);
                Ok(())
            }
            (AddrSlot::Broadcast { subs }, DeliveryMode::Broadcast) => {
                let qs: Vec<_> = subs.values().cloned().collect();
                drop(addresses);
                for q in qs {
                    q.lock().await.push_back(payload.clone());
                }
                Ok(())
            }
            _ => Err(mode_mismatch(addr)),
        }
    }

    pub(crate) async fn subscribe(&self, addr: &[u8], mode: DeliveryMode) -> io::Result<u64> {
        let mut addresses = self.addresses.lock().await;
        let slot = addresses
            .entry(addr.to_vec())
            .or_insert_with(|| empty_slot(mode));

        let sub_id = self.next_sub_id.fetch_add(1, Ordering::Relaxed);
        let queue = match (slot, mode) {
            (AddrSlot::Anycast { queue, sub_count }, DeliveryMode::Anycast) => {
                *sub_count += 1;
                queue.clone()
            }
            (AddrSlot::Broadcast { subs }, DeliveryMode::Broadcast) => {
                let q = Arc::new(Mutex::new(Queue::new()));
                subs.insert(sub_id, q.clone());
                q
            }
            _ => return Err(mode_mismatch(addr)),
        };
        drop(addresses);

        self.subscribers.lock().await.insert(
            sub_id,
            Subscription {
                addr: addr.to_vec(),
                queue,
                mode,
            },
        );
        Ok(sub_id)
    }

    pub(crate) async fn consume(&self, sub_id: u64) -> io::Result<Option<Bytes>> {
        let subs = self.subscribers.lock().await;
        let sub = subs.get(&sub_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("unknown subscriber id {sub_id}"),
            )
        })?;
        let q = sub.queue.clone();
        drop(subs);
        Ok(q.lock().await.pop_front())
    }

    pub(crate) async fn unsubscribe(&self, sub_id: u64) {
        let removed = self.subscribers.lock().await.remove(&sub_id);
        let Some(sub) = removed else {
            return;
        };

        let mut addresses = self.addresses.lock().await;
        let Some(slot) = addresses.get_mut(&sub.addr) else {
            return;
        };
        match (slot, sub.mode) {
            (AddrSlot::Anycast { sub_count, .. }, DeliveryMode::Anycast) => {
                *sub_count = sub_count.saturating_sub(1);
            }
            (AddrSlot::Broadcast { subs }, DeliveryMode::Broadcast) => {
                subs.remove(&sub_id);
            }
            _ => {}
        }
    }
}

fn empty_slot(mode: DeliveryMode) -> AddrSlot {
    match mode {
        DeliveryMode::Anycast => AddrSlot::Anycast {
            queue: Arc::new(Mutex::new(Queue::new())),
            sub_count: 0,
        },
        DeliveryMode::Broadcast => AddrSlot::Broadcast {
            subs: HashMap::new(),
        },
    }
}

fn mode_mismatch(addr: &[u8]) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("delivery mode mismatch for address {addr:?}"),
    )
}

pub struct LocalProducer<T: Codec, A: Address> {
    inner: Arc<ServerInner>,
    addr: A,
    _t: PhantomData<fn() -> T>,
}

#[async_trait::async_trait]
impl<T: Codec + Send + 'static, A: Address + Send + Sync> Producer<T> for LocalProducer<T, A> {
    async fn produce(&self, message: T) -> io::Result<()> {
        self.inner
            .produce(
                &self.addr.encode(),
                self.addr.delivery_mode(),
                message.encode(),
            )
            .await
    }
}

pub struct LocalConsumer<T: Codec> {
    inner: Arc<ServerInner>,
    sub_id: u64,
    _t: PhantomData<fn() -> T>,
}

impl<T: Codec> LocalConsumer<T> {
    pub fn sub_id(&self) -> u64 {
        self.sub_id
    }
}

impl<T: Codec> Drop for LocalConsumer<T> {
    fn drop(&mut self) {
        if tokio::runtime::Handle::try_current().is_ok() {
            let inner = self.inner.clone();
            let sub_id = self.sub_id;
            tokio::spawn(async move { inner.unsubscribe(sub_id).await });
        }
    }
}

#[async_trait::async_trait]
impl<T: Codec + Send> Consumer<T> for LocalConsumer<T> {
    async fn consume(&self) -> io::Result<Option<T>> {
        match self.inner.consume(self.sub_id).await? {
            Some(b) => match T::decode(&b) {
                Ok(m) => Ok(Some(m)),
                Err(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "decode failed")),
            },
            None => Ok(None),
        }
    }
}
