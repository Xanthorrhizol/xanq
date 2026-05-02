use xancode::Codec;

#[derive(Codec, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Anycast,
    Broadcast,
}

pub trait Address: Codec {
    fn delivery_mode(&self) -> DeliveryMode {
        DeliveryMode::Anycast
    }
    fn retention_time(&self) -> u64 {
        0 // unlimited
    }
    fn retention_size(&self) -> u64 {
        0 // unlimited
    }
}
