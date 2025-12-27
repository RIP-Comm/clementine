use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// A fixed-capacity ring buffer that maintains the most recent N elements.
///
/// When the buffer is full and a new element is pushed, the oldest element
/// is removed to make room. This is useful for maintaining a sliding window
/// of recent items, such as disassembled instructions.
#[derive(Default, Serialize, Deserialize)]
pub struct RingBuffer<T> {
    capacity: usize,
    buffer: VecDeque<T>,
}

impl<T> RingBuffer<T> {
    /// Creates a new ring buffer with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    /// Pushes an element to the back of the buffer.
    ///
    /// If the buffer is at capacity, the oldest (front) element is removed.
    pub fn push(&mut self, element: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(element);
    }

    /// Returns an iterator over the elements in order (oldest to newest).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    /// Returns the number of elements in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let ring: RingBuffer<u8> = RingBuffer::new(10);
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn push_within_capacity() {
        let mut ring: RingBuffer<u8> = RingBuffer::new(3);

        ring.push(1);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec![1]);

        ring.push(2);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec![1, 2]);

        ring.push(3);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn push_over_capacity_removes_oldest() {
        let mut ring: RingBuffer<u8> = RingBuffer::new(3);

        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4]);

        ring.push(5);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn works_with_strings() {
        let mut ring: RingBuffer<String> = RingBuffer::new(3);

        ring.push("hello".to_string());
        ring.push("world".to_string());
        ring.push("!!!".to_string());

        let joined: String = ring.iter().cloned().collect::<Vec<_>>().join(" ");
        assert_eq!(joined, "hello world !!!");
    }
}
