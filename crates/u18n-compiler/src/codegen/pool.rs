use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
    sync::Arc,
};

pub(super) const MAX_POOL_OPTIMIZATION_BYTES: usize = 1_048_576;
const MAX_OVERLAP_WORK_BYTES: usize = 268_435_456;

struct TextReference {
    offset: u32,
    length: u32,
}

pub(super) struct StringPool {
    value: String,
    references: BTreeMap<String, TextReference>,
    fragment_count: usize,
    overlap_optimization_enabled: bool,
}

struct PoolComponentPosition {
    text: String,
    base: u32,
}

struct PoolComponent {
    text: Arc<str>,
    prefix: Vec<usize>,
}

#[derive(Eq, PartialEq)]
struct OverlapCandidate {
    length: usize,
    left_index: usize,
    right_index: usize,
    left: Arc<str>,
    right: Arc<str>,
}

impl Ord for OverlapCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.length
            .cmp(&other.length)
            .then_with(|| other.left.cmp(&self.left))
            .then_with(|| other.right.cmp(&self.right))
            .then_with(|| other.left_index.cmp(&self.left_index))
            .then_with(|| other.right_index.cmp(&self.right_index))
    }
}

impl PartialOrd for OverlapCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl StringPool {
    pub(super) fn build(fragments: BTreeSet<String>, limit: usize) -> Result<Self, String> {
        let fragment_count = fragments.len();
        let total_fragment_bytes = fragments.iter().fold(0usize, |total, fragment| {
            total.saturating_add(fragment.len())
        });
        let estimated_work = fragment_count
            .checked_mul(fragment_count)
            .and_then(|pairs| pairs.checked_mul(total_fragment_bytes));
        let overlap_optimization_enabled = fragment_count <= limit
            && total_fragment_bytes <= MAX_POOL_OPTIMIZATION_BYTES
            && estimated_work.is_some_and(|work| work <= MAX_OVERLAP_WORK_BYTES);
        let all_fragments = fragments;
        let mut components = if overlap_optimization_enabled {
            Self::optimized_components(&all_fragments)?
        } else {
            all_fragments.iter().cloned().collect()
        };
        components.sort();
        let mut pool = String::new();
        let mut positioned = Vec::new();
        let mut references = BTreeMap::new();
        for component in components {
            let base = Self::checked_index(pool.len())?;
            let length = Self::checked_index(component.len())?;
            pool.push_str(&component);
            if !overlap_optimization_enabled {
                references.insert(
                    component.clone(),
                    TextReference {
                        offset: base,
                        length,
                    },
                );
            }
            positioned.push(PoolComponentPosition {
                text: component,
                base,
            });
        }
        Self::checked_index(pool.len())?;
        if overlap_optimization_enabled {
            for fragment in all_fragments {
                let mut found = None;
                for component in &positioned {
                    if let Some(position) = component.text.find(&fragment) {
                        let base = usize::try_from(component.base).map_err(|_| {
                            "string pool offset is outside the host index range".to_string()
                        })?;
                        let offset = base
                            .checked_add(position)
                            .ok_or_else(|| "string pool offset overflowed".to_string())?;
                        found = Some(TextReference {
                            offset: Self::checked_index(offset)?,
                            length: Self::checked_index(fragment.len())?,
                        });
                        break;
                    }
                }
                let reference = found
                    .ok_or_else(|| "optimized string pool lost a literal fragment".to_string())?;
                references.insert(fragment, reference);
            }
        }
        Ok(Self {
            value: pool,
            references,
            fragment_count,
            overlap_optimization_enabled,
        })
    }

    fn optimized_components(fragments: &BTreeSet<String>) -> Result<Vec<String>, String> {
        let initial = Self::remove_contained(fragments.iter().cloned().collect());
        let mut components = initial
            .into_iter()
            .map(|text| Some(PoolComponent::new(text)))
            .collect::<Vec<_>>();
        let mut overlaps = BinaryHeap::new();
        for left_index in 0..components.len() {
            for right_index in 0..components.len() {
                if left_index != right_index {
                    Self::queue_overlap(&mut overlaps, &components, left_index, right_index)?;
                }
            }
        }
        while let Some(choice) = Self::next_overlap(&mut overlaps, &components) {
            let left = components
                .get_mut(choice.left_index)
                .and_then(Option::take)
                .ok_or_else(|| "string pool overlap source disappeared".to_string())?;
            let right = components
                .get_mut(choice.right_index)
                .and_then(Option::take)
                .ok_or_else(|| "string pool overlap target disappeared".to_string())?;
            let right_tail = right
                .text
                .get(choice.length..)
                .ok_or_else(|| "string pool overlap split is not a UTF-8 boundary".to_string())?;
            let merged_capacity = left
                .text
                .len()
                .checked_add(right_tail.len())
                .ok_or_else(|| "string pool component length overflowed".to_string())?;
            let mut merged = String::with_capacity(merged_capacity);
            merged.push_str(&left.text);
            merged.push_str(right_tail);
            let Some(merged_index) = Self::insert_merged(merged, &mut components) else {
                continue;
            };
            for other_index in 0..components.len() {
                if other_index != merged_index
                    && components.get(other_index).is_some_and(Option::is_some)
                {
                    Self::queue_overlap(&mut overlaps, &components, other_index, merged_index)?;
                    Self::queue_overlap(&mut overlaps, &components, merged_index, other_index)?;
                }
            }
        }
        Ok(components
            .into_iter()
            .flatten()
            .map(|component| component.text.to_string())
            .collect())
    }

    fn insert_merged(merged: String, components: &mut Vec<Option<PoolComponent>>) -> Option<usize> {
        if components.iter().flatten().any(|component| {
            component.text.as_ref() == merged
                || (component.text.len() > merged.len() && component.text.contains(merged.as_str()))
        }) {
            return None;
        }
        for component in components.iter_mut() {
            if let Some(existing) = component.as_ref()
                && merged.len() > existing.text.len()
                && merged.contains(existing.text.as_ref())
            {
                *component = None;
            }
        }
        let component = PoolComponent::new(merged);
        let index = components.len();
        components.push(Some(component));
        Some(index)
    }

    fn queue_overlap(
        overlaps: &mut BinaryHeap<OverlapCandidate>,
        components: &[Option<PoolComponent>],
        left_index: usize,
        right_index: usize,
    ) -> Result<(), String> {
        let left = components
            .get(left_index)
            .and_then(Option::as_ref)
            .ok_or_else(|| "string pool overlap source disappeared".to_string())?;
        let right = components
            .get(right_index)
            .and_then(Option::as_ref)
            .ok_or_else(|| "string pool overlap target disappeared".to_string())?;
        let length = Self::overlap_length(&left.text, &right.text, &right.prefix);
        if length > 0 {
            overlaps.push(OverlapCandidate {
                length,
                left_index,
                right_index,
                left: left.text.clone(),
                right: right.text.clone(),
            });
        }
        Ok(())
    }

    fn next_overlap(
        overlaps: &mut BinaryHeap<OverlapCandidate>,
        components: &[Option<PoolComponent>],
    ) -> Option<OverlapCandidate> {
        while let Some(candidate) = overlaps.pop() {
            if components
                .get(candidate.left_index)
                .is_some_and(Option::is_some)
                && components
                    .get(candidate.right_index)
                    .is_some_and(Option::is_some)
            {
                return Some(candidate);
            }
        }
        None
    }

    fn remove_contained(components: Vec<String>) -> Vec<String> {
        let unique = components.into_iter().collect::<BTreeSet<_>>();
        unique
            .iter()
            .filter(|candidate| {
                !unique.iter().any(|container| {
                    container.len() > candidate.len() && container.contains(candidate.as_str())
                })
            })
            .cloned()
            .collect()
    }

    fn prefix_function(text: &str) -> Vec<usize> {
        let pattern = text.as_bytes();
        let mut prefix = vec![0usize; pattern.len()];
        for index in 1..pattern.len() {
            let mut matched = prefix.get(index - 1).copied().unwrap_or_default();
            while matched > 0 && pattern.get(index) != pattern.get(matched) {
                matched = prefix.get(matched - 1).copied().unwrap_or_default();
            }
            if pattern.get(index) == pattern.get(matched) {
                matched += 1;
            }
            if let Some(value) = prefix.get_mut(index) {
                *value = matched;
            }
        }
        prefix
    }

    fn overlap_length(left: &str, right: &str, prefix: &[usize]) -> usize {
        let pattern = right.as_bytes();
        if pattern.is_empty() {
            return 0;
        }
        let scan_start = left.len().saturating_sub(pattern.len());
        let mut matched = 0usize;
        for byte in left.as_bytes().get(scan_start..).unwrap_or_default() {
            while matched > 0 && (matched == pattern.len() || pattern.get(matched) != Some(byte)) {
                matched = prefix.get(matched - 1).copied().unwrap_or_default();
            }
            if pattern.get(matched) == Some(byte) {
                matched += 1;
            }
        }
        while matched > 0 {
            let left_start = left.len().saturating_sub(matched);
            if left.is_char_boundary(left_start) && right.is_char_boundary(matched) {
                return matched;
            }
            matched = prefix.get(matched - 1).copied().unwrap_or_default();
        }
        0
    }

    pub(super) fn value(&self) -> &str {
        &self.value
    }

    pub(super) fn fragment_count(&self) -> usize {
        self.fragment_count
    }

    pub(super) fn overlap_optimization_enabled(&self) -> bool {
        self.overlap_optimization_enabled
    }

    pub(super) fn reference(&self, text: &str) -> Option<(u32, u32)> {
        self.references
            .get(text)
            .map(|reference| (reference.offset, reference.length))
    }

    fn checked_index(value: usize) -> Result<u32, String> {
        u32::try_from(value).map_err(|_| "string pool exceeds the u32 index limit".to_string())
    }
}

impl PoolComponent {
    fn new(text: String) -> Self {
        let text: Arc<str> = Arc::from(text);
        let prefix = StringPool::prefix_function(&text);
        Self { text, prefix }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fmt::Debug};

    use super::{MAX_POOL_OPTIMIZATION_BYTES, StringPool};

    #[test]
    fn merges_utf8_boundary_overlaps_and_keeps_exact_references() -> Result<(), String> {
        let fragments = ["café", "éclair"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, 20)?;

        check_equal(pool.value(), "caféclair");
        check_equal(pool.reference("café"), Some((0, 5)));
        check_equal(pool.reference("éclair"), Some((3, 7)));
        Ok(())
    }

    #[test]
    fn keeps_greedy_tie_breaking_deterministic() -> Result<(), String> {
        let fragments = ["ab", "bc", "ca"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, 20)?;

        check_equal(pool.value(), "abca");
        Ok(())
    }

    #[test]
    fn disables_overlap_scans_when_fragment_bytes_exceed_the_budget() -> Result<(), String> {
        let fragment = "x".repeat(MAX_POOL_OPTIMIZATION_BYTES + 1);
        let fragments = [fragment.clone()].into_iter().collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, 20)?;

        check_equal(pool.overlap_optimization_enabled(), false);
        let length = u32::try_from(fragment.len()).map_err(|error| error.to_string())?;
        check_equal(pool.reference(&fragment), Some((0, length)));
        Ok(())
    }

    #[test]
    fn disables_overlap_scans_when_estimated_pair_work_exceeds_the_budget() -> Result<(), String> {
        let fragments = (0..256)
            .map(|index| format!("{index:04}-abcdefghijklmnop"))
            .collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, 256)?;

        check_equal(pool.overlap_optimization_enabled(), false);
        Ok(())
    }

    #[test]
    fn skips_containment_scans_above_the_fragment_limit() -> Result<(), String> {
        let fragments = ["abc", "b"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, 0)?;

        check_equal(pool.value(), "abcb");
        check_equal(pool.reference("abc"), Some((0, 3)));
        check_equal(pool.reference("b"), Some((3, 1)));
        check_equal(pool.overlap_optimization_enabled(), false);
        Ok(())
    }

    #[test]
    fn removes_contained_fragments_below_the_fragment_limit() -> Result<(), String> {
        let fragments = ["abc", "b"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let pool = StringPool::build(fragments, 2)?;

        check_equal(pool.value(), "abc");
        check_equal(pool.reference("abc"), Some((0, 3)));
        check_equal(pool.reference("b"), Some((1, 1)));
        Ok(())
    }

    fn check_equal<T: Debug + PartialEq>(actual: T, expected: T) {
        assert_eq!(actual, expected);
    }
}
