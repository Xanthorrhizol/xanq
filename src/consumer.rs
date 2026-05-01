use crate::message::Message;
use crate::queue::Queue;
use std::sync::Mutex;

pub trait Consumer<T: Message> {
    fn queue(&self) -> &Mutex<Queue>;
    fn consume(&self) -> Option<T> {
        let message = self.queue().lock().unwrap().pop_front();
        message.map(|m| Message::decode(&m).ok()).flatten()
    }
    fn peek(&self, id: usize) -> Option<T> {
        self.queue()
            .lock()
            .unwrap()
            .peek(id)
            .map(|m| Message::decode(&m).ok())
            .flatten()
    }
}
