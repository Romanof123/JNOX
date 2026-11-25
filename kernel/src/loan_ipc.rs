//! IPC-005 kernel metadata. Syscalls are serialized on one CPU. The ring has
//! one producer and one consumer; its counters publish kernel-owned handles,
//! never user pointers. No borrowed page bytes cross a mapping transition.
use core::sync::atomic::{AtomicU64, Ordering};

const PAGES: usize = 3;
const PAGE_SIZE: usize = 4096;
const CAP: usize = 2;
const MAX_GENERATION: u64 = u64::MAX >> 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapOp {
    ZeroMapWriter { pid: usize, slot: usize },
    RevokeWriterClearTail { pid: usize, slot: usize, len: usize },
    MapReader { pid: usize, slot: usize },
    Revoke { pid: usize, slot: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    pub slot: usize,
    pub handle: u64,
    pub len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    pub state: u8,
    pub handle: u64,
    pub len: usize,
}

#[derive(Clone, Copy)]
enum Page {
    Free,
    Writer(u64),
    Queued(Grant),
    Reader(Grant),
}

struct Ring {
    head: AtomicU64,
    tail: AtomicU64,
    cells: [AtomicU64; CAP],
}

impl Ring {
    const fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            cells: [const { AtomicU64::new(0) }; CAP],
        }
    }

    fn used(head: u64, tail: u64) -> usize {
        let n = tail.wrapping_sub(head);
        assert!(n <= CAP as u64, "loan ring distance exceeds capacity");
        n as usize
    }

    fn enqueue_position(&self) -> Result<(u64, usize), i32> {
        let tail = self.tail.load(Ordering::Relaxed);
        // Acquire pairs with the consumer's release of a used ring cell.
        let used = Self::used(self.head.load(Ordering::Acquire), tail);
        if used == CAP {
            Err(-11)
        } else {
            Ok((tail, used + 1))
        }
    }

    fn publish(&self, tail: u64, handle: u64) {
        self.cells[(tail % CAP as u64) as usize].store(handle, Ordering::Relaxed);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
    }

    fn front(&self) -> Option<(u64, u64)> {
        let head = self.head.load(Ordering::Relaxed);
        // Observe the producer's payload/descriptor publication before use.
        let tail = self.tail.load(Ordering::Acquire);
        if Self::used(head, tail) == 0 {
            return None;
        }
        Some((
            head,
            self.cells[(head % CAP as u64) as usize].load(Ordering::Relaxed),
        ))
    }

    fn consume(&self, head: u64) {
        self.cells[(head % CAP as u64) as usize].store(0, Ordering::Relaxed);
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }
}

pub struct LoanIpc {
    producer: usize,
    consumer: usize,
    writer_live: bool,
    reader_live: bool,
    generation: u64,
    pages: [Page; PAGES],
    ring: Ring,
    peak: usize,
}

impl LoanIpc {
    pub const fn new(producer: usize, consumer: usize) -> Self {
        assert!(producer < 3 && consumer < 3 && producer != consumer);
        Self {
            producer,
            consumer,
            writer_live: true,
            reader_live: true,
            generation: 0,
            pages: [Page::Free; PAGES],
            ring: Ring::new(),
            peak: 0,
        }
    }

    pub fn producer_alive(&self) -> bool {
        self.writer_live
    }
    pub fn consumer_alive(&self) -> bool {
        self.reader_live
    }
    pub fn head(&self) -> u64 {
        self.ring.head.load(Ordering::Acquire)
    }
    pub fn tail(&self) -> u64 {
        self.ring.tail.load(Ordering::Acquire)
    }
    pub fn occupancy(&self) -> usize {
        Ring::used(self.head(), self.tail())
    }
    pub fn high_water(&self) -> usize {
        self.peak
    }
    pub fn page(&self, slot: usize) -> Option<PageInfo> {
        let (state, handle, len) = match *self.pages.get(slot)? {
            Page::Free => (0, 0, 0),
            Page::Writer(h) => (1, h, 0),
            Page::Queued(g) => (2, g.handle, g.len),
            Page::Reader(g) => (3, g.handle, g.len),
        };
        Some(PageInfo { state, handle, len })
    }

    fn writer(&self, pid: usize) -> Result<(), i32> {
        if pid != self.producer {
            return Err(-13);
        }
        if !self.writer_live || !self.reader_live {
            return Err(-32);
        }
        Ok(())
    }

    fn slot(handle: u64) -> usize {
        (handle & 3) as usize
    }

    /// A successful hook completes the entire privileged mapping operation
    /// (including required synchronization). On error it must have made no
    /// change, or the runtime must halt before returning to user execution.
    pub fn acquire(
        &mut self,
        pid: usize,
        mut hook: impl FnMut(MapOp) -> Result<(), i32>,
    ) -> Result<Grant, i32> {
        self.writer(pid)?;
        let slot = self
            .pages
            .iter()
            .position(|p| matches!(p, Page::Free))
            .ok_or(-11)?;
        if self.generation == MAX_GENERATION {
            return Err(-75);
        }
        let generation = self.generation + 1;
        let handle = (generation << 2) | slot as u64;
        hook(MapOp::ZeroMapWriter { pid, slot })?;
        self.generation = generation;
        self.pages[slot] = Page::Writer(handle);
        Ok(Grant {
            slot,
            handle,
            len: 0,
        })
    }

    pub fn publish(
        &mut self,
        pid: usize,
        handle: u64,
        length: u64,
        mut hook: impl FnMut(MapOp) -> Result<(), i32>,
    ) -> Result<(), i32> {
        self.writer(pid)?;
        let slot = Self::slot(handle);
        if !matches!(self.pages.get(slot),Some(Page::Writer(h)) if *h==handle) {
            return Err(-116);
        }
        if length > PAGE_SIZE as u64 {
            return Err(-90);
        }
        let (tail, occupancy) = self.ring.enqueue_position()?;
        let len = length as usize;
        hook(MapOp::RevokeWriterClearTail { pid, slot, len })?;
        self.pages[slot] = Page::Queued(Grant { slot, handle, len });
        self.ring.publish(tail, handle);
        self.peak = self.peak.max(occupancy);
        Ok(())
    }

    pub fn receive(
        &mut self,
        pid: usize,
        mut hook: impl FnMut(MapOp) -> Result<(), i32>,
    ) -> Result<Grant, i32> {
        if pid != self.consumer {
            return Err(-13);
        }
        if !self.reader_live {
            return Err(-32);
        }
        let (head, handle) = self
            .ring
            .front()
            .ok_or(if self.writer_live { -11 } else { -32 })?;
        let slot = Self::slot(handle);
        let Page::Queued(g) = self.pages[slot] else {
            unreachable!("ring must name a queued loan")
        };
        assert_eq!(g.handle, handle);
        hook(MapOp::MapReader { pid, slot })?;
        self.pages[slot] = Page::Reader(g);
        self.ring.consume(head);
        Ok(g)
    }

    pub fn release(
        &mut self,
        pid: usize,
        handle: u64,
        mut hook: impl FnMut(MapOp) -> Result<(), i32>,
    ) -> Result<(), i32> {
        if pid != self.producer && pid != self.consumer {
            return Err(-13);
        }
        if !self.reader_live || (pid == self.producer && !self.writer_live) {
            return Err(-32);
        }
        let slot = Self::slot(handle);
        match self.pages.get(slot) {
            Some(Page::Writer(h)) if *h == handle && pid == self.producer => {}
            Some(Page::Reader(g)) if g.handle == handle && pid == self.consumer => {}
            Some(Page::Reader(g)) if g.handle == handle => return Err(-13),
            Some(Page::Writer(h)) if *h == handle => return Err(-13),
            _ => return Err(-116),
        }
        hook(MapOp::Revoke { pid, slot })?;
        self.pages[slot] = Page::Free;
        Ok(())
    }

    #[cfg(jnox_native_safety_channel)]
    pub fn invalidate(&mut self, mut hook: impl FnMut(MapOp) -> Result<(), i32>) -> Result<(), i32> {
        for (slot, page) in self.pages.iter_mut().enumerate() {
            let pid = match page {
                Page::Writer(_) => Some(self.producer),
                Page::Reader(_) => Some(self.consumer),
                _ => None,
            };
            if let Some(pid) = pid { hook(MapOp::Revoke { pid, slot })?; }
            *page = Page::Free;
        }
        while let Some((head, _)) = self.ring.front() { self.ring.consume(head); }
        // Keep the generation counter: stale handles never become valid again.
        Ok(())
    }

    pub fn exit(
        &mut self,
        pid: usize,
        mut hook: impl FnMut(MapOp) -> Result<(), i32>,
    ) -> Result<(), i32> {
        if pid == self.producer {
            self.writer_live = false;
        } else if pid == self.consumer {
            self.reader_live = false;
        } else {
            return Ok(());
        }
        // Latch closure before revocation. Each successful revocation is
        // committed individually; a retry never frees an unrevoked mapping.
        for slot in 0..PAGES {
            let owner = match self.pages[slot] {
                Page::Writer(_) => self.producer,
                Page::Reader(_) if !self.reader_live => self.consumer,
                _ => continue,
            };
            hook(MapOp::Revoke { pid: owner, slot })?;
            self.pages[slot] = Page::Free;
        }
        if !self.reader_live {
            while let Some((head, handle)) = self.ring.front() {
                let slot = Self::slot(handle);
                assert!(matches!(self.pages[slot],Page::Queued(g) if g.handle==handle));
                self.pages[slot] = Page::Free;
                self.ring.consume(head);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ok(_: MapOp) -> Result<(), i32> {
        Ok(())
    }
    fn unexpected(_: MapOp) -> Result<(), i32> {
        panic!("refusal must precede MMU work")
    }

    #[test]
    fn full_and_oversize_keep_writer_then_publish_in_order_across_wrap() {
        let mut c = LoanIpc::new(0, 1);
        c.ring.head.store(u64::MAX - 1, Ordering::Relaxed);
        c.ring.tail.store(u64::MAX - 1, Ordering::Relaxed);
        let a = c.acquire(0, ok).unwrap();
        let b = c.acquire(0, ok).unwrap();
        let d = c.acquire(0, ok).unwrap();
        assert_eq!(c.acquire(0, unexpected), Err(-11));
        for (g, n) in [(a, 0), (b, 4096)] {
            c.publish(0, g.handle, n, |op| {
                assert_eq!(
                    op,
                    MapOp::RevokeWriterClearTail {
                        pid: 0,
                        slot: g.slot,
                        len: n as usize
                    }
                );
                Ok(())
            })
            .unwrap();
        }
        assert_eq!((c.head(), c.tail(), c.occupancy()), (u64::MAX - 1, 0, 2));
        assert_eq!(c.publish(0, d.handle, 4097, unexpected), Err(-90));
        assert_eq!(c.publish(0, d.handle, 3, unexpected), Err(-11));
        assert_eq!(c.page(d.slot).unwrap().state, 1);
        assert_eq!(c.receive(1, ok).unwrap().handle, a.handle);
        c.publish(0, d.handle, 3, ok).unwrap();
        for (g, n) in [(b, 4096), (d, 3)] {
            let r = c.receive(1, ok).unwrap();
            assert_eq!((r.handle, r.len), (g.handle, n));
        }
        assert_eq!(
            (c.head(), c.tail(), c.occupancy(), c.high_water()),
            (1, 1, 0, 2)
        );
        assert_eq!(c.release(0, a.handle, unexpected), Err(-13));
        c.release(1, a.handle, ok).unwrap();
        let fresh = c.acquire(0, ok).unwrap();
        assert_eq!(fresh.slot, a.slot);
        assert_ne!(fresh.handle, a.handle);
        assert_eq!(c.release(1, a.handle, unexpected), Err(-116));
    }

    #[test]
    fn failed_mapping_never_publishes_or_reclaims() {
        let mut c = LoanIpc::new(1, 2);
        assert_eq!(c.acquire(0, unexpected), Err(-13));
        assert_eq!(c.acquire(1, |_| Err(-5)), Err(-5));
        let a = c.acquire(1, ok).unwrap();
        assert_eq!(c.publish(1, a.handle, 1, |_| Err(-5)), Err(-5));
        assert_eq!((c.page(a.slot).unwrap().state, c.occupancy()), (1, 0));
        c.publish(1, a.handle, 1, ok).unwrap();
        assert_eq!(c.receive(2, |_| Err(-5)), Err(-5));
        assert_eq!((c.page(a.slot).unwrap().state, c.occupancy()), (2, 1));
        c.receive(2, ok).unwrap();
        assert_eq!(c.release(2, a.handle, |_| Err(-5)), Err(-5));
        assert_eq!(c.page(a.slot).unwrap().state, 3);
        c.release(2, a.handle, ok).unwrap();
        c.generation = MAX_GENERATION;
        assert_eq!(c.acquire(1, unexpected), Err(-75));
    }

    #[test]
    fn producer_death_keeps_queued_and_active_reads() {
        let mut c = LoanIpc::new(0, 1);
        let a = c.acquire(0, ok).unwrap();
        c.publish(0, a.handle, 2, ok).unwrap();
        c.receive(1, ok).unwrap();
        let b = c.acquire(0, ok).unwrap();
        c.publish(0, b.handle, 3, ok).unwrap();
        let writer = c.acquire(0, ok).unwrap();
        c.exit(0, |op| {
            assert_eq!(
                op,
                MapOp::Revoke {
                    pid: 0,
                    slot: writer.slot
                }
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(c.page(writer.slot).unwrap().state, 0);
        assert_eq!(c.page(a.slot).unwrap().state, 3);
        assert_eq!(c.receive(1, ok).unwrap().handle, b.handle);
        assert_eq!(c.receive(1, unexpected), Err(-32));
        c.release(1, a.handle, ok).unwrap();
        c.release(1, b.handle, ok).unwrap();
    }

    #[test]
    fn consumer_close_retries_unrevoked_pages_and_drains_commits() {
        let mut c = LoanIpc::new(0, 1);
        let a = c.acquire(0, ok).unwrap();
        let b = c.acquire(0, ok).unwrap();
        let d = c.acquire(0, ok).unwrap();
        c.publish(0, b.handle, 4, ok).unwrap();
        c.receive(1, ok).unwrap();
        c.publish(0, d.handle, 0, ok).unwrap();
        assert_eq!(
            c.exit(1, |op| match op {
                MapOp::Revoke { pid: 0, slot } if slot == a.slot => Ok(()),
                MapOp::Revoke { pid: 1, slot } if slot == b.slot => Err(-5),
                _ => panic!("unexpected close mapping"),
            }),
            Err(-5)
        );
        assert_eq!(c.page(a.slot).unwrap().state, 0);
        assert_eq!(c.page(b.slot).unwrap().state, 3);
        assert_eq!(c.occupancy(), 1);
        assert_eq!(c.acquire(0, unexpected), Err(-32));
        c.exit(1, |op| {
            assert_eq!(
                op,
                MapOp::Revoke {
                    pid: 1,
                    slot: b.slot
                }
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(c.occupancy(), 0);
        assert!((0..PAGES).all(|slot| c.page(slot).unwrap().state == 0));
        c.exit(1, unexpected).unwrap();
    }
}
