use bytes::Bytes;
use std::collections::VecDeque;

pub struct Queue {
    data: VecDeque<Option<Bytes>>,
    head: usize,
    tail: usize,
    base: usize,
}

impl Queue {
    pub fn new() -> Queue {
        Queue {
            data: VecDeque::new(),
            head: 0,
            tail: 0,
            base: 0,
        }
    }
    // ##########################################
    // # push/pop/clear - Modify
    // ##########################################
    pub fn push_back(&mut self, data: Bytes) {
        self.data.push_back(Some(data));
        self.tail += 1;
    }
    pub fn pop_front(&mut self) -> Option<Bytes> {
        // it doesn't fully pop the data to remain id system
        let result = self.front();
        if result.is_some() {
            self.data[self.head].take();
            self.head += 1;
        }
        result
    }
    pub fn compact(&mut self) -> usize {
        let mut diff = 0;
        for _ in 0..self.head {
            let _ = self.data.pop_front();
            self.head -= 1;
            self.tail -= 1;
            self.base += 1;
            diff += 1;
        }

        diff
    }
    pub fn clear(&mut self) -> usize {
        let diff = self.tail - self.head;
        self.data.clear();
        self.base += self.tail;
        self.head = 0;
        self.tail = 0;
        diff
    }

    // ##########################################
    // # front/rear - View
    // ##########################################
    pub fn front(&self) -> Option<Bytes> {
        self.data.get(self.head).cloned().flatten()
    }
    pub fn rear(&self) -> Option<Bytes> {
        if self.data.is_empty() {
            return None;
        }
        self.data.get(self.tail - 1).cloned().flatten()
    }
    pub fn peek(&self, id: usize) -> Option<Bytes> {
        if self.head == self.tail {
            return None;
        }
        if id < self.base {
            return None;
        }
        self.data.get(id - self.base).cloned().flatten()
    }

    // ##########################################
    // # metadata
    // ##########################################
    pub fn len(&self) -> usize {
        self.tail - self.head
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
