//! Cost contract for one configured POL-007 manifest. All cost values are counter ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub(crate) enum Denial {
    Missing = 1,
    Format,
    Scope,
    Class,
    Frequency,
    Overflow,
    Irq,
    Preparation,
    Service,
}

fn field<'a>(lines: &mut core::str::Lines<'a>, name: &str) -> Result<&'a str, Denial> {
    let (key, value) = lines.next().and_then(|s| s.split_once('=')).ok_or(Denial::Format)?;
    if key != name || value.is_empty() {
        return Err(Denial::Format);
    }
    Ok(value)
}

fn ticks(lines: &mut core::str::Lines<'_>, name: &str) -> Result<u64, Denial> {
    let value = field(lines, name)?;
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Denial::Format);
    }
    value.parse().map_err(|_| Denial::Format)
}

fn total(lines: &mut core::str::Lines<'_>, start: u64, names: &[&str]) -> Result<u64, Denial> {
    names.iter().try_fold(start, |sum, name| {
        sum.checked_add(ticks(lines, name)?).ok_or(Denial::Overflow)
    })
}

pub(crate) fn check(report: &str, case: &str, hz: u64) -> Result<(), Denial> {
    if report.is_empty() {
        return Err(Denial::Missing);
    }
    let mut lines = report.lines();
    if field(&mut lines, "target")? != "virt-a72-gicv2-tcg-1cpu-128m"
        || field(&mut lines, "case")? != case
    {
        return Err(Denial::Scope);
    }
    if field(&mut lines, "class")? != "justified-bound" {
        return Err(Denial::Class);
    }
    if ticks(&mut lines, "hz")? != hz || hz == 0 || hz % 1000 != 0 {
        return Err(Denial::Frequency);
    }

    let irq = total(&mut lines, 0, &["irq_latency", "irq_interference", "irq_uncertainty"])?;
    let preparation = total(&mut lines, irq, &[
        "irq_handler", "submission", "arbitration", "preparation_switch",
        "preparation_instrumentation", "preparation_interference", "preparation_uncertainty",
    ])?;
    let worker = ticks(&mut lines, "server_work")?.max(ticks(&mut lines, "periodic_work")?);
    let service = total(&mut lines, worker, &[
        "service_switch", "receipt", "service_instrumentation", "service_interference",
        "service_uncertainty",
    ])?;
    if lines.next() != Some("---") || !lines.any(|s| !s.trim().is_empty()) {
        return Err(Denial::Format);
    }

    let ms = hz / 1000;
    if irq > ms {
        Err(Denial::Irq)
    } else if preparation > 5 * ms {
        Err(Denial::Preparation)
    } else if service > 45 * ms {
        Err(Denial::Service)
    } else {
        Ok(())
    }
}
