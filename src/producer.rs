use crate::message::Message;
use crate::queue::Queue;
use std::sync::Mutex;

pub trait Producer<T: Message> {
    fn queue(&self) -> &Mutex<Queue>;
    fn produce(&self, message: T) {
        self.queue().lock().unwrap().push_back(message.encode());
    }
}
