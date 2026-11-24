use super::mutex_policy::{LockSet, LockSpec, Protocol};

pub(crate) const PRI: [u8; 4] = [2, 5, 9, 0];
pub(crate) fn locks(ceiling: bool) -> Option<LockSet<4, 2>> {
    let specs = if ceiling {
        [
            LockSpec {
                users: 3,
                protocol: Protocol::Immediate(5),
            },
            LockSpec {
                users: 5,
                protocol: Protocol::Immediate(9),
            },
        ]
    } else {
        // Second slot is unreachable through the PI SVC interface.
        [
            LockSpec {
                users: 5,
                protocol: Protocol::Inherit,
            },
            LockSpec {
                users: 1,
                protocol: Protocol::Inherit,
            },
        ]
    };
    LockSet::new(PRI, specs)
}
