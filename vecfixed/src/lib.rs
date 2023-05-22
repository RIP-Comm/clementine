use std::collections::VecDeque;

/// VecFixed is basically a vector that keep a fixed size. Every time new element is pushed
/// to the vector, the oldest element is removed and the latest pushed is added to the end.
#[derive(Default)]
pub struct VecFixed<const N: usize, T: Default + ToString> {
    next_index: usize,
    buffer: VecDeque<T>,
}

impl<const N: usize, T: Default + ToString> VecFixed<N, T> {
    pub fn new() -> Self {
        // let mut buffer = ;
        // for _ in 0..N {
        //     buffer.push_back(T::default());
        // }

        Self {
            next_index: 0,
            buffer: VecDeque::with_capacity(N),
        }
    }

    pub fn push(&mut self, element: T) {
        if self.next_index == N {
            self.buffer.pop_front();
        } else {
            self.next_index += 1;
        }

        self.buffer.push_back(element);
    }

    /// Join the elements of the VecFixed buffer into a string.
    pub fn join(&self, separator: &str) -> String {
        if self.buffer.is_empty() {
            return String::new();
        }

        let mut s = String::new();
        s.push_str(&self.buffer[0].to_string());

        for element in self.buffer.iter().skip(1) {
            s.push_str(separator);
            s.push_str(&element.to_string());
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let ring: VecFixed<10, u8> = VecFixed::new();
        assert_eq!(ring.next_index, 0);
        assert_eq!(ring.buffer.len(), 0);
        assert_eq!(ring.buffer.capacity(), 10);

        let ring: VecFixed<10, String> = VecFixed::new();
        assert_eq!(ring.next_index, 0);
        assert_eq!(ring.buffer.len(), 0);
        assert_eq!(ring.buffer.capacity(), 10);
    }

    #[test]
    fn push() {
        let mut ring: VecFixed<3, u8> = VecFixed::new();

        ring.push(1);
        assert_eq!(ring.next_index, 1);
        assert_eq!(ring.buffer, [1]);

        ring.push(2);
        assert_eq!(ring.next_index, 2);
        assert_eq!(ring.buffer, [1, 2]);

        ring.push(3);
        assert_eq!(ring.next_index, 3);
        assert_eq!(ring.buffer, [1, 2, 3]);

        ring.push(4);
        assert_eq!(ring.next_index, 3);
        assert_eq!(ring.buffer, [2, 3, 4]);

        ring.push(5);
        assert_eq!(ring.next_index, 3);
        assert_eq!(ring.buffer, [3, 4, 5]);
    }

    #[test]
    fn join() {
        let mut ring: VecFixed<3, u8> = VecFixed::new();
        assert_eq!(ring.join(","), "");

        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4);
        assert_eq!(ring.join(","), "2,3,4");

        let mut ring = VecFixed::<3, String>::new();
        ring.push("hello".to_string());
        ring.push("world".to_string());
        ring.push("!!!".to_string());

        assert_eq!(ring.join(" "), "hello world !!!");
    }
}
