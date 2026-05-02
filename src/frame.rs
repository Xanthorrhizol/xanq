use crate::address::DeliveryMode;
use bytes::Bytes;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use xancode::Codec;

#[derive(Codec, Debug, Clone)]
pub enum Frame {
    // client -> server
    Produce { addr: Vec<u8>, mode: DeliveryMode, payload: Vec<u8> },
    Subscribe { addr: Vec<u8>, mode: DeliveryMode },
    Consume { sub_id: u64 },
    Unsubscribe { sub_id: u64 },

    // server -> client
    ProduceOk,
    SubscribeOk { sub_id: u64 },
    ConsumeOk { payload: Option<Vec<u8>> },
    UnsubscribeOk,
    Err { msg: String },
}

impl Frame {
    pub async fn write_to<W: AsyncWrite + Unpin>(&self, w: &mut W) -> io::Result<()> {
        let buf = self.encode();
        w.write_all(&buf).await?;
        w.flush().await
    }

    pub async fn read_from<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Self> {
        let mut header = [0u8; 4];
        r.read_exact(&mut header).await?;
        let payload_len = u32::from_be_bytes(header) as usize;

        let mut buf = vec![0u8; 4 + payload_len];
        buf[..4].copy_from_slice(&header);
        r.read_exact(&mut buf[4..]).await?;

        let bytes = Bytes::from(buf);
        Frame::decode(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))
    }
}
