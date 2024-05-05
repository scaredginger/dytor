pub struct LocalQueue<T> {
    items: Vec<T>,
}

impl<T> Default for LocalQueue<T> {
    fn default() -> Self {
        Self {
            items: Default::default(),
        }
    }
}

impl<T: 'static> LocalQueue<T> {
    pub fn send(&mut self, value: T) {
        self.items.push(value);
    }

    pub fn recv(&mut self) -> Option<T> {
        self.items.pop()
    }

    pub fn unbounded() -> Self {
        Self { items: Vec::new() }
    }
}
