//! Bounded engineering telemetry profile. Requests carry indices into immutable ingress frames.
use super::{
    batch_policy::BatchPolicy,
    batch_runtime::{self as batch, Error, Handle},
    uart_write,
};

#[derive(Clone, Copy)]
enum JobClass {
    Summarize,
    CountAtLeast42,
    Validate,
}

impl JobClass {
    fn limits(self) -> (usize, u64) {
        // Maximum frame length in bytes and the scheduling estimate in counter ticks.
        match self {
            Self::Summarize => (32, 30_000),
            Self::CountAtLeast42 => (24, 20_000),
            Self::Validate => (16, 10_000),
        }
    }
}

static REQUESTS: [(JobClass, &[u8]); 4] = [
    (JobClass::Summarize, b"0\n0007\n65535\n"),
    (JobClass::CountAtLeast42, b"18\n42\n43\n65535\n"),
    (JobClass::Validate, b"11\n65536\n"),
    (JobClass::Validate, b"0000000000000001\n"),
];

struct Summary {
    count: u64,
    min: u16,
    max: u16,
    sum: u64,
    at_least_42: u64,
}

enum InputError {
    Empty = 3,
    Invalid = 4,
}

fn summarize(frame: &[u8]) -> Result<Summary, InputError> {
    if frame.is_empty() {
        return Err(InputError::Empty);
    }
    let mut summary = Summary {
        count: 0,
        min: u16::MAX,
        max: 0,
        sum: 0,
        at_least_42: 0,
    };
    let mut value = 0u16;
    let mut has_digit = false;
    for &byte in frame {
        match byte {
            b'0'..=b'9' => {
                value = value
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(u16::from(byte - b'0')))
                    .ok_or(InputError::Invalid)?;
                has_digit = true;
            }
            b'\n' if has_digit => {
                // The largest admitted frame contains only 32 bytes of u16 records.
                summary.count += 1;
                summary.sum += u64::from(value);
                summary.min = summary.min.min(value);
                summary.max = summary.max.max(value);
                summary.at_least_42 += u64::from(value >= 42);
                value = 0;
                has_digit = false;
            }
            _ => return Err(InputError::Invalid),
        }
    }
    if has_digit {
        Err(InputError::Invalid)
    } else {
        Ok(summary)
    }
}

fn number(mut n: u64) {
    let mut digits = [0u8; 20];
    let mut first = digits.len();
    loop {
        first -= 1;
        digits[first] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    uart_write(b" ");
    uart_write(&digits[first..]);
}

fn process(id: usize, _: Handle) {
    let (class, frame) = REQUESTS[id];
    let start = batch::counter();
    let result = core::hint::black_box(summarize(core::hint::black_box(frame)));
    let end = batch::counter();
    uart_write(b"TELEMETRY JOB");
    number(id as u64);
    number(start);
    number(end);
    match result {
        Err(error) => {
            uart_write(b" ERROR");
            number(error as u64);
        }
        Ok(s) => match class {
            JobClass::Summarize => {
                uart_write(b" SUMMARY");
                for v in [s.count, u64::from(s.min), u64::from(s.max), s.sum] {
                    number(v);
                }
            }
            JobClass::CountAtLeast42 => {
                uart_write(b" AT_LEAST_42");
                number(s.at_least_42);
            }
            JobClass::Validate => uart_write(b" VALID"),
        },
    }
    uart_write(b"\n");
}

fn submit(id: usize) {
    let status: &[u8] = match REQUESTS.get(id) {
        None => b" UNKNOWN",
        Some(&(class, frame)) => {
            let (max_bytes, estimate) = class.limits();
            if frame.len() > max_bytes {
                b" LENGTH"
            } else {
                match batch::submit(process, id, estimate, 0) {
                    Ok(_) => b" ACCEPT",
                    Err(Error::Capacity) => b" FULL",
                    Err(_) => b" RUNTIME_ERROR",
                }
            }
        }
    };
    uart_write(b"TELEMETRY REQUEST");
    number(id as u64);
    uart_write(status);
    uart_write(b"\n");
}

fn coordinator(_: usize, _: Handle) {
    uart_write(b"TELEMETRY FREQUENCY");
    number(batch::frequency());
    uart_write(b"\n");
    for id in [4, 3, 0, 1, 2, 0] {
        submit(id);
    }
}

pub(super) fn run() -> ! {
    #[cfg(jnox_app = "batch-sjf")]
    let policy = BatchPolicy::Sjf;
    #[cfg(jnox_app = "batch-srtf")]
    let policy = BatchPolicy::Srtf;
    #[cfg(jnox_app = "batch-hrrn")]
    let policy = BatchPolicy::Hrrn;
    batch::start(policy, coordinator, 1)
}
