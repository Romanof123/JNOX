//! Construct a native-v1 startup tail using user virtual addresses.
const APP: &[u8] = b"/bin/app";
const PROBE: &[&[u8]] = &[b"/bin/native_startup_probe", b"8", b"13"];
const MAX_EXTRAS: usize = 15;
const MAX_EXTRA_BYTES: usize = 1024;
pub(crate) const MAX_TAIL: usize = 1200;

#[derive(Clone, Copy)]
pub(crate) struct Layout {
    pub(crate) sp: usize,
    pub(crate) len: usize,
    pub(crate) argc: usize,
}

pub(crate) fn build(
    extras: &[&[u8]], probe: bool, bottom: usize, top: usize, output: &mut [u8],
) -> Option<Layout> {
    if extras.len() > MAX_EXTRAS || (probe && !extras.is_empty()) || top % 16 != 0 {
        return None;
    }
    let extra_bytes = string_bytes(extras)?;
    let primary: &[&[u8]] = if probe { PROBE } else { &[APP] };
    let argc = primary.len().checked_add(extras.len())?;
    let (offset, len) = if extras.is_empty() {
        // Existing ordinary and probe images have a fixed string offset.
        (48, 80)
    } else {
        // argc, pointers, argv NULL and the empty envp NULL.
        let header = argc.checked_add(3)?.checked_mul(8)?;
        let used = header.checked_add(APP.len() + 1)?.checked_add(extra_bytes)?;
        (header, used.checked_add(15)? & !15)
    };
    encode(primary.iter().chain(extras.iter()).copied(), argc, offset, len, bottom, top, output)
}

fn string_bytes(args: &[&[u8]]) -> Option<usize> {
    let mut bytes = 0usize;
    for &arg in args {
        bytes = bytes.checked_add(arg.len().checked_add(1)?)?;
        if bytes > MAX_EXTRA_BYTES || arg.contains(&0) || core::str::from_utf8(arg).is_err() {
            return None;
        }
    }
    Some(bytes)
}

/// Spawn supplies the entire vector, including its executable's argv[0].
#[cfg(any(jnox_native_shell, test))]
pub(crate) fn build_argv(argv: &[&[u8]], bottom: usize, top: usize, output: &mut [u8]) -> Option<Layout> {
    let argc = argv.len();
    if argc == 0 || argc > MAX_EXTRAS + 1 || argv[0].is_empty() || top % 16 != 0 { return None; }
    let string_len = string_bytes(argv)?;
    let header = (argc + 3) * 8;
    let len = (header + string_len + 15) & !15;
    encode(argv.iter().copied(), argc, header, len, bottom, top, output)
}

fn encode<'a>(args: impl Iterator<Item = &'a [u8]>, argc: usize, mut offset: usize, len: usize,
              bottom: usize, top: usize, output: &mut [u8]) -> Option<Layout> {
    let sp = top.checked_sub(len)?;
    if sp < bottom || len > MAX_TAIL { return None; }
    let tail = output.get_mut(..len)?;
    tail.fill(0);
    tail[..8].copy_from_slice(&(argc as u64).to_le_bytes());
    for (i, arg) in args.enumerate() {
        let end = offset.checked_add(arg.len())?;
        let next = end.checked_add(1)?;
        let ptr_offset = i.checked_add(1)?.checked_mul(8)?;
        let ptr_end = ptr_offset.checked_add(8)?;
        let address = u64::try_from(sp.checked_add(offset)?).ok()?;
        if next > len { return None; }
        tail.get_mut(ptr_offset..ptr_end)?.copy_from_slice(&address.to_le_bytes());
        tail.get_mut(offset..end)?.copy_from_slice(arg);
        offset = next;
    }
    Some(Layout { sp, len, argc })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_arguments_keep_the_supplied_program_name_and_both_null_sentinels() {
        let mut bytes = [0xff; MAX_TAIL];
        let layout = build_argv(&[b"cat", b"sample.txt", b""], 0, 4096, &mut bytes).unwrap();
        let word = |index: usize| u64::from_le_bytes(bytes[index*8..index*8+8].try_into().unwrap());
        assert_eq!((layout.sp, layout.argc, layout.len), (4032, 3, 64));
        assert_eq!([word(0), word(1), word(2), word(3), word(4), word(5)], [3, 4080, 4084, 4095, 0, 0]);
        assert_eq!(&bytes[48..64], b"cat\0sample.txt\0\0");
        for invalid in [&[][..], &[&b""[..]][..], &[&b"wc"[..], &b"bad\0arg"[..]][..]] {
            assert!(build_argv(invalid, 0, 4096, &mut bytes).is_none());
        }
    }

    #[test]
    fn omitted_args_reproduce_legacy_ordinary_and_probe_tails() {
        for (probe, args) in [(false, &[APP][..]), (true, PROBE)] {
            let mut bytes = [0xff; MAX_TAIL];
            let layout = build(&[], probe, 0x1001000, 0x1011000, &mut bytes).unwrap();
            let mut expected = [0u8; 80];
            expected[..8].copy_from_slice(&(args.len() as u64).to_le_bytes());
            let mut string = 48;
            for (i, arg) in args.iter().enumerate() {
                let word = 8 + 8*i;
                expected[word..word+8].copy_from_slice(&(0x1010fb0u64 + string as u64).to_le_bytes());
                expected[string..string+arg.len()].copy_from_slice(arg);
                string += arg.len()+1;
            }
            assert_eq!((layout.sp, layout.len, layout.argc), (0x1010fb0, 80, args.len()));
            assert_eq!(bytes[..80], expected);
        }
    }

    #[test]
    fn empty_and_utf8_arguments_have_distinct_terminated_payloads() {
        let mut bytes = [0xff; MAX_TAIL];
        let args: &[&[u8]] = &[b"", b"- east", &[0xc3, 0xa9]];
        let layout = build(args, false, 0, 4096, &mut bytes).unwrap();
        assert_eq!((layout.sp, layout.len, layout.argc), (4016, 80, 4));
        assert_eq!(&bytes[40..56], &[0; 16]);
        assert_eq!(&bytes[56..76], b"/bin/app\0\0- east\0\xc3\xa9\0");
        for (i, offset) in [56,65,66,73].into_iter().enumerate() {
            let at = 8*(i+1);
            assert_eq!(u64::from_le_bytes(bytes[at..at+8].try_into().unwrap()), layout.sp as u64 + offset);
        }
    }

    #[test]
    fn stack_limits_and_malformed_inputs_are_rejected() {
        let mut bytes = [0u8; MAX_TAIL];
        let payload = [b'x'; 1009];
        let mut args: [&[u8]; 15] = [&[]; 15];
        args[0] = &payload;
        assert_eq!(build(&args, false, 0, 4096, &mut bytes).unwrap().len, MAX_TAIL);
        assert!(build(&args, false, 3000, 4096, &mut bytes).is_none());
        assert!(build(&args, false, 0, 4097, &mut bytes).is_none());
        assert!(build(&[&[] as &[u8]; 16], false, 0, 4096, &mut bytes).is_none());
        for arg in [&[b'x';1024][..], &[0xff], b"a\0b"] {
            assert!(build(&[arg], false, 0, 4096, &mut bytes).is_none());
        }
        assert!(build(&[b""], true, 0, 4096, &mut bytes).is_none());
    }
}
