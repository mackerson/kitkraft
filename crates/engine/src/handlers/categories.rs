//! The Object Window category of every record type (M7.S2, pipeline W), returned by
//! `schema` as its optional `category` member (`crate::shapes`): from the parity rows,
//! with the types no sitting observed in "Other", marked so.
//!
//! What a sitting has observed of the CK's category tree so far: its top node, "*All"
//! (`ck-20261101-03`, `docs/ck-parity.md` row "Object Window (category tree, list,
//! filter)"). The tree's other nodes and which types they hold wait on sitting 1's step
//! S1-09 (`docs/ck-observations.md`). Until then [`OBSERVED`] is empty, every defined type
//! is in [`OTHER`], and `schema` says so in its limitations. A sitting that records the
//! tree adds one `category` row per node to `docs/ck-parity.md` and the same node, in the
//! same order, here (a test holds the two equal).

/// The tree's top node, which lists every form (observed, `ck-20261101-03`). It is not a
/// category of its own: every type is under it.
#[cfg(test)]
pub(crate) const ALL: &str = "*All";

/// Where a type goes that no observed category holds (KitKraft's name, not the CK's).
pub(crate) const OTHER: &str = "Other";

/// The observed categories in the CK's tree order, each with the record types it holds.
/// Empty until a sitting records the tree (S1-09).
pub(crate) const OBSERVED: &[(&str, &[&str])] = &[];

/// Stated by `schema` while some type is in [`OTHER`].
pub(crate) const UNOBSERVED: &str = "Object Window categories: a sitting observed only the \
     tree's top node \"*All\" (ck-20261101-03); every type not in an observed category is in \
     \"Other\", KitKraft's own name, until sitting 1 (step S1-09) records the CK's tree (M7.S2)";

/// The category of record type `signature`: the observed category that holds it, else
/// [`OTHER`].
pub(crate) fn category_of(signature: &str) -> &'static str {
    OBSERVED
        .iter()
        .find(|(_, types)| types.iter().any(|t| t.eq_ignore_ascii_case(signature)))
        .map_or(OTHER, |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M7.S2: every defined type has exactly one category, and the categories' order is
    /// the parity rows' (the `category` rows of `docs/ck-parity.md`, none observed yet).
    #[test]
    fn every_type_has_one_category_in_the_parity_rows_order() {
        let schema = kitkraft_schema::builtin().unwrap();
        for record in schema.records() {
            let sig = record.signature.as_str();
            let holders = OBSERVED
                .iter()
                .filter(|(_, types)| types.contains(&sig))
                .count();
            assert!(holders <= 1, "{sig} is in {holders} categories");
            assert_eq!(category_of(sig) == OTHER, holders == 0, "{sig}");
        }
        let parity = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/ck-parity.md"
        ))
        .unwrap();
        let rows: Vec<String> = parity
            .lines()
            .filter(|l| l.starts_with("| category |"))
            .filter_map(|l| l.split('|').nth(2).map(|c| c.trim().to_string()))
            .collect();
        let names: Vec<&str> = OBSERVED.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            rows, names,
            "the observed categories are the parity rows, in order"
        );
        assert!(!names.contains(&OTHER) && !names.contains(&ALL));
        assert!(parity.contains("\"*All\""), "the top node's observation");
    }
}
