use crate::message::Message;
use crate::queue::Queue;
use tokio::sync::Mutex;

#[async_trait::async_trait]
pub trait Consumer<T: Message> {
    fn queue(&self) -> &Mutex<Queue>;
    async fn consume(&self) -> Option<T> {
        match self.queue().lock().await.pop_front() {
            Some(m) => match T::decode(&m) {
                Ok(m) => Some(m),
                Err(_) => {
                    error!("failed to decode message");
                    None
                }
            },
            None => {
                warn!("failed to pop message");
                None
            }
        }
    }
    async fn peek(&self, id: usize) -> Option<T> {
        match self.queue().lock().await.peek(id) {
            Some(m) => match T::decode(&m) {
                Ok(m) => Some(m),
                Err(_) => {
                    error!("failed to decode message");
                    None
                }
            },
            None => {
                warn!("failed to peek message");
                None
            }
        }
    }
}
