//! Build-time decoding of the validated package's compact hierarchy.
//! Wire fields are period;parent quotas;parent:child quota;child:leaf quota.

fn number(field: &str) -> u64 {
    assert!(
        !field.is_empty()
            && field.bytes().all(|b| b.is_ascii_digit())
            && (field == "0" || !field.starts_with('0')),
        "hierarchy numbers require canonical unsigned decimal digits"
    );
    field.parse().expect("hierarchy number exceeds u64")
}

fn quota(field: &str, period: u64) -> u64 {
    let value = number(field);
    assert!((1..=period).contains(&value), "hierarchy quota outside 1..=period");
    value
}

fn level(field: &str, owners: usize, period: u64) -> Vec<(usize, u64)> {
    let mut nodes = Vec::new();
    for row in field.split(',') {
        assert!(nodes.len() < 3, "hierarchy supports three nodes at this level");
        let (owner, limit) = row.split_once(':').expect("hierarchy requires owner:quota");
        let owner = usize::try_from(number(owner)).expect("hierarchy owner exceeds usize");
        assert!(owner < owners, "hierarchy references absent owner");
        nodes.push((owner, quota(limit, period)));
    }
    nodes
}

pub fn constants(raw: &str, slots: usize) -> String {
    let mut fields = raw.split(';');
    let period = number(fields.next().expect("hierarchy requires a period"));
    assert!(period > 0, "hierarchy period must be positive");
    let mut parents = Vec::new();
    for field in fields.next().expect("hierarchy requires parents").split(',') {
        assert!(parents.len() < 2, "hierarchy supports two parents");
        parents.push(quota(field, period));
    }
    let children = level(fields.next().expect("hierarchy requires children"), parents.len(), period);
    let leaves = level(fields.next().expect("hierarchy requires leaves"), children.len(), period);
    assert!(fields.next().is_none(), "hierarchy requires exactly three budget levels");
    assert_eq!(leaves.len(), slots, "each native ELF must own one leaf");
    format!(
        "pub const NATIVE_HIERARCHY_PERIOD: u64 = {period};\n\
         pub const NATIVE_HIERARCHY_PARENTS: [u64; {}] = {parents:?};\n\
         pub const NATIVE_HIERARCHY_CHILDREN: [(usize, u64); {}] = {children:?};\n\
         pub const NATIVE_HIERARCHY_LEAVES: [(usize, u64); {}] = {leaves:?};\n",
        parents.len(), children.len(), leaves.len()
    )
}
