use bytes::Bytes;

pub trait Message
where
    Self: Sized,
{
    type Error: std::error::Error;
    fn encode(&self) -> Bytes;
    fn decode(data: &Bytes) -> Result<Self, Self::Error>;
}
