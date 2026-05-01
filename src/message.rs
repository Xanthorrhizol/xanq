use crate::address::Address;
use xancode::Codec;

pub trait Message
where
    Self: Sized + Codec,
{
    fn address<T: Address>(&self) -> T;
}
