use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: Ord + Serialize",
    deserialize = "T: Ord + Deserialize<'de>"
))]
pub struct OrSet<T> {
    pub adds: BTreeMap<T, BTreeSet<String>>,
    pub removes: BTreeMap<T, BTreeSet<String>>,
}

impl<T> Default for OrSet<T> {
    fn default() -> Self {
        Self {
            adds: BTreeMap::new(),
            removes: BTreeMap::new(),
        }
    }
}

impl<T: Ord + Clone> OrSet<T> {
    pub fn add(&mut self, element: T, tag: impl Into<String>) {
        self.adds.entry(element).or_default().insert(tag.into());
    }

    pub fn remove_observed(&mut self, element: &T, _remove_tag: impl Into<String>) {
        let Some(observed_tags) = self.adds.get(element) else {
            return;
        };
        let removed = self.removes.entry(element.clone()).or_default();
        for tag in observed_tags {
            removed.insert(tag.clone());
        }
    }

    pub fn contains(&self, element: &T) -> bool {
        self.visible_tags(element).next().is_some()
    }

    pub fn elements(&self) -> BTreeSet<T> {
        self.adds
            .keys()
            .filter(|element| self.contains(element))
            .cloned()
            .collect()
    }

    pub fn tags_for(&self, element: &T) -> BTreeSet<String> {
        self.visible_tags(element).cloned().collect()
    }

    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        for (element, tags) in &other.adds {
            merged
                .adds
                .entry(element.clone())
                .or_default()
                .extend(tags.iter().cloned());
        }
        for (element, tags) in &other.removes {
            merged
                .removes
                .entry(element.clone())
                .or_default()
                .extend(tags.iter().cloned());
        }
        merged
    }

    pub fn merge_mut(&mut self, other: &Self) {
        *self = self.merge(other);
    }

    fn visible_tags(&self, element: &T) -> impl Iterator<Item = &String> {
        let removed_tags = self.removes.get(element);
        self.adds
            .get(element)
            .into_iter()
            .flat_map(|tags| tags.iter())
            .filter(move |tag| {
                removed_tags
                    .map(|removed| !removed.contains(*tag))
                    .unwrap_or(true)
            })
    }
}
