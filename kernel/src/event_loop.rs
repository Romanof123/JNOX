//! Cooperative FIFO dispatcher. Each turn runs one handler to completion.
//! A handler may append events for later turns.

pub(crate) struct EventLoop<E, const N: usize> {
    slots: [Option<E>; N],
    read: usize,
    write: usize,
    len: usize,
}

impl<E, const N: usize> EventLoop<E, N> {
    pub(crate) const fn new() -> Self {
        Self { slots: [const { None }; N], read: 0, write: 0, len: 0 }
    }

    pub(crate) fn depth(&self) -> usize { self.len }

    fn advance(index: usize) -> usize {
        if index + 1 == N { 0 } else { index + 1 }
    }

    /// On overflow return the original event to the sender.
    pub(crate) fn enqueue(&mut self, event: E) -> Result<(), E> {
        if self.depth() == N {
            return Err(event);
        }
        self.slots[self.write] = Some(event);
        self.write = Self::advance(self.write);
        self.len += 1;
        Ok(())
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &E> {
        let mut index = self.read;
        (0..self.len).map(move |_| {
            let value = self.slots[index].as_ref().expect("occupied FIFO slot");
            index = Self::advance(index);
            value
        })
    }

    pub(crate) fn pop(&mut self) -> Option<E> {
        if self.len == 0 {
            return None;
        }
        let event = self.slots[self.read].take().expect("occupied FIFO slot");
        self.read = Self::advance(self.read);
        self.len -= 1;
        Some(event)
    }

    /// Release the oldest event's slot before entering the handler. The
    /// callback receives only the enqueue operation for this loop.
    pub(crate) fn dispatch_one(
        &mut self,
        handler: impl FnOnce(E, &mut dyn FnMut(E) -> Result<(), E>),
    ) -> bool {
        let Some(event) = self.pop() else { return false };
        handler(event, &mut |next| self.enqueue(next));
        true
    }
}
