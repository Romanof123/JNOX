//! Compile with rustc --test kernel/src/native_virtio_reset_test.rs.
#[path = "native_virtio_reset.rs"]
mod protocol;

use std::cell::RefCell;
use protocol::{reset, ResetIo};

#[derive(Debug, PartialEq)]
enum Event { Fence, Request, Status, Select(u32), Ready(u32), Scrub }
use Event::*;

struct Registers<'a> {
    log: &'a RefCell<Vec<Event>>,
    pending: usize,
    selected: usize,
    ready: [u32; 2],
}
impl ResetIo for Registers<'_> {
    fn request_reset(&mut self) { self.log.borrow_mut().push(Request); }
    fn status(&mut self) -> u32 {
        self.log.borrow_mut().push(Status);
        if self.pending == 0 { return 0; }
        self.pending -= 1;
        1
    }
    fn select(&mut self, queue: u32) {
        self.selected = queue as usize;
        self.log.borrow_mut().push(Select(queue));
    }
    fn ready(&mut self) -> u32 {
        self.log.borrow_mut().push(Ready(self.selected as u32));
        self.ready[self.selected]
    }
    fn fence(&mut self) { self.log.borrow_mut().push(Fence); }
}

fn run(pending: usize, ready: [u32; 2]) -> (Result<(), i32>, Vec<Event>, [u8; 32]) {
    let log = RefCell::new(Vec::new());
    let mut dma = [0xa5; 32];
    let mut io = Registers { log: &log, pending, selected: 0, ready };
    let result = reset(&mut io, || {
        log.borrow_mut().push(Scrub);
        dma.fill(0);
    });
    (result, log.into_inner(), dma)
}

#[test]
fn held_ack_preserves_dma() {
    let (res, events, dma) = run(usize::MAX, [0, 0]);
    assert_eq!(res, Err(-5));
    assert_eq!(dma, [0xa5; 32]);
    assert_eq!(events.iter().filter(|e| **e == Status).count(), 1024);
    assert!(!events.iter().any(|e| matches!(e, Select(_) | Scrub)));
}

#[test]
fn either_ready_bit_preserves_dma() {
    for bits in [[1, 0], [0, 1]] {
        let (res, events, dma) = run(0, bits);
        assert_eq!(res, Err(-5));
        assert_eq!(dma, [0xa5; 32]);
        assert!(!events.contains(&Scrub));
    }
}

#[test]
fn ack_and_both_queue_reads_precede_scrub() {
    let (res, events, dma) = run(1, [0, 0]);
    assert_eq!(res, Ok(()));
    assert_eq!(dma, [0; 32]);
    assert_eq!(events, vec![
        Fence, Request, Fence, Status, Status, Fence, Select(0), Fence, Ready(0),
        Fence, Select(1), Fence, Ready(1), Fence, Scrub
    ]);
}

#[test]
fn late_device_recovery_cannot_regrant_quarantined_dma() {
    use protocol::{retirement_phase, start, Start, QUARANTINED};
    let log = RefCell::new(Vec::new());
    let mut dma = [0xa5; 32];
    for (pending, ready) in [(usize::MAX, [0, 0]), (0, [1, 0]), (0, [0, 1])] {
        let mut io = Registers { log: &log, pending, selected: 0, ready };
        let retired = reset(&mut io, || dma.fill(0));
        let phase = retirement_phase(retired, false);
        assert_eq!(phase, QUARANTINED);
        io.pending = 0;
        io.ready = [0, 0];
        assert_eq!(io.status(), 0);
        for q in [0, 1] {
            io.select(q);
            assert_eq!(io.ready(), 0);
        }
        for request in [Start::Restart, Start::Cold, Start::Restart] {
            assert_eq!(start(request, phase, false, || {
                dma.fill(1);
                Ok(())
            }), Err(-5));
            assert_eq!(dma, [0xa5; 32]);
        }
    }
}

#[test]
fn graceful_retirement_enables_controlled_restart() {
    use protocol::{retirement_phase, start, Start, ACTIVE, COLD, RETIRED, RETIRING};
    assert_eq!(start(Start::Restart, COLD, false, || panic!()), Err(-5));
    assert_eq!(start(Start::Restart, ACTIVE, false, || panic!()), Err(-5));
    assert_eq!(start(Start::Restart, RETIRING, false, || panic!()), Err(-5));
    let phase = retirement_phase(run(1, [0, 0]).0, false);
    assert_eq!(phase, RETIRED);
    assert_eq!(start(Start::Cold, phase, false, || panic!()), Err(-5));
    let mut setups = 0;
    assert_eq!(start(Start::Restart, phase, false, || { setups += 1; Ok(()) }), Ok(()));
    assert_eq!(setups, 1);
}
