pub trait Address
where
    Self: Sized + std::str::FromStr + std::string::ToString,
{
    fn retention_time(&self) -> u64 {
        0 // unlimited
    }
    fn retention_size(&self) -> u64 {
        0 // unlimited
    }
}
