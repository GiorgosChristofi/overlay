//! Item sets shown in the in-game shop's "recommended" tab.

use crate::builds::Build;
use crate::static_catalog::StaticCatalog;

/// Item sets we create carry uids with this prefix so we only replace our own.
pub const UID_PREFIX: &str = "lol-overlay-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSet {
    pub uid: String,
    pub title: String,
    pub champion_id: i64,
    pub blocks: Vec<(String, Vec<i64>)>,
}

/// Builds an item set from a build, dropping items that no longer exist this patch.
/// Returns the set and the ids that were dropped.
pub fn item_set_for(build: &Build, catalog: &StaticCatalog) -> (ItemSet, Vec<i64>) {
    let mut dropped = Vec::new();
    let blocks = build
        .item_blocks
        .iter()
        .map(|block| {
            let items = block
                .items
                .iter()
                .copied()
                .filter(|id| {
                    let known = catalog.items.contains_key(id);
                    if !known {
                        dropped.push(*id);
                    }
                    known
                })
                .collect();
            (block.title.clone(), items)
        })
        .filter(|(_, items): &(String, Vec<i64>)| !items.is_empty())
        .collect();

    let set = ItemSet {
        uid: format!("{UID_PREFIX}{}", build.champion_id),
        title: format!(
            "LoL Overlay: {} {}",
            catalog.champion_name(build.champion_id),
            build.role.label()
        ),
        champion_id: build.champion_id,
        blocks,
    };
    (set, dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builds::ItemBlock;
    use crate::champ_select::Role;

    #[test]
    fn drops_items_removed_this_patch() {
        let mut catalog = StaticCatalog::default();
        catalog.items.insert(1055, "Doran's Blade".into());
        catalog.champions.insert(222, "Jinx".into());
        let build = Build {
            champion_id: 222,
            role: Role::Bottom,
            rune_pages: vec![],
            summoner_spells: Some([4, 7]),
            item_blocks: vec![
                ItemBlock {
                    title: "Start".into(),
                    items: vec![1055, 999_999],
                },
                ItemBlock {
                    title: "Gone".into(),
                    items: vec![888_888],
                },
            ],
            source: String::new(),
        };
        let (set, dropped) = item_set_for(&build, &catalog);
        assert_eq!(set.blocks, vec![("Start".to_string(), vec![1055])]);
        assert_eq!(dropped, vec![999_999, 888_888]);
        assert_eq!(set.uid, "lol-overlay-222");
        assert_eq!(set.title, "LoL Overlay: Jinx Bottom");
    }
}
