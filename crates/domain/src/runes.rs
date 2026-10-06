//! Rune pages: validation against the current patch's rune trees, and the
//! decision of which existing page (if any) to replace.

use crate::static_catalog::StaticCatalog;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Pages this app creates start with this prefix, so we only ever replace our own.
pub const PAGE_NAME_PREFIX: &str = "LoL Overlay: ";

/// A rune selection independent of the client's page bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuneSelection {
    pub primary_style_id: i64,
    pub sub_style_id: i64,
    /// Keystone, 3 primary runes, 2 secondary runes, 3 stat shards (9 total).
    pub perk_ids: Vec<i64>,
}

/// The body sent to `POST /lol-perks/v1/pages`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRunePage {
    pub name: String,
    pub primary_style_id: i64,
    pub sub_style_id: i64,
    pub selected_perk_ids: Vec<i64>,
    pub current: bool,
}

impl NewRunePage {
    pub fn from_selection(champion_name: &str, selection: &RuneSelection) -> Self {
        Self {
            name: format!("{PAGE_NAME_PREFIX}{champion_name}"),
            primary_style_id: selection.primary_style_id,
            sub_style_id: selection.sub_style_id,
            selected_perk_ids: selection.perk_ids.clone(),
            current: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuneError {
    #[error("expected 9 runes, got {0}")]
    WrongCount(usize),
    #[error("primary and secondary tree are the same ({0})")]
    SameTree(i64),
    #[error("unknown rune tree {0}")]
    UnknownTree(i64),
    #[error("rune {perk} is not valid in position {position}")]
    InvalidPerk { position: usize, perk: i64 },
    #[error("both secondary runes come from the same row")]
    SecondarySameRow,
}

/// Checks a selection against the trees of the current patch.
///
/// Builds are data, and Riot reworks runes; an invalid build is skipped with a
/// clear reason instead of sending a page the client would reject.
pub fn validate(selection: &RuneSelection, catalog: &StaticCatalog) -> Result<(), RuneError> {
    let perks = &selection.perk_ids;
    if perks.len() != 9 {
        return Err(RuneError::WrongCount(perks.len()));
    }
    if selection.primary_style_id == selection.sub_style_id {
        return Err(RuneError::SameTree(selection.primary_style_id));
    }
    let primary = catalog
        .rune_tree(selection.primary_style_id)
        .ok_or(RuneError::UnknownTree(selection.primary_style_id))?;
    let sub = catalog
        .rune_tree(selection.sub_style_id)
        .ok_or(RuneError::UnknownTree(selection.sub_style_id))?;

    // Keystone + one rune from each of the three primary rows, in order.
    for (position, perk) in perks.iter().take(4).enumerate() {
        let row = primary.slots.get(position).ok_or(RuneError::InvalidPerk {
            position,
            perk: *perk,
        })?;
        if !row.contains(perk) {
            return Err(RuneError::InvalidPerk {
                position,
                perk: *perk,
            });
        }
    }

    // Two secondary runes from different non-keystone rows.
    let row_of = |perk: i64| sub.slots.iter().skip(1).position(|row| row.contains(&perk));
    let mut secondary_rows = Vec::with_capacity(2);
    for (offset, perk) in perks.iter().skip(4).take(2).enumerate() {
        let row = row_of(*perk).ok_or(RuneError::InvalidPerk {
            position: 4 + offset,
            perk: *perk,
        })?;
        secondary_rows.push(row);
    }
    if secondary_rows.first() == secondary_rows.get(1) {
        return Err(RuneError::SecondarySameRow);
    }

    // Three stat shards, one per row.
    for (offset, perk) in perks.iter().skip(6).enumerate() {
        let valid = catalog
            .stat_shard_rows
            .get(offset)
            .is_some_and(|row| row.contains(perk));
        if !valid {
            return Err(RuneError::InvalidPerk {
                position: 6 + offset,
                perk: *perk,
            });
        }
    }
    Ok(())
}

/// A rune page already in the client (`GET /lol-perks/v1/pages`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExistingPage {
    pub id: i64,
    pub name: String,
    pub is_editable: bool,
    pub is_deletable: bool,
    pub current: bool,
}

/// What to do before creating our page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotPlan {
    /// A free slot exists; just create.
    Create,
    /// Delete this page first, then create.
    Replace(i64),
    /// Every slot is taken and the user does not allow replacing their pages.
    NoFreeSlot,
}

/// Decides which page to replace. Our own page is always reused first; a
/// player's own page is only replaced when they allow it in settings.
pub fn plan_slot(
    pages: &[ExistingPage],
    owned_page_count: u32,
    replace_current_when_full: bool,
) -> SlotPlan {
    let editable: Vec<&ExistingPage> = pages
        .iter()
        .filter(|p| p.is_editable && p.is_deletable)
        .collect();

    if let Some(ours) = editable
        .iter()
        .find(|p| p.name.starts_with(PAGE_NAME_PREFIX))
    {
        return SlotPlan::Replace(ours.id);
    }
    if let Some(active) = editable.iter().find(|p| p.current) {
        return SlotPlan::Replace(active.id);
    }
    if (editable.len() as u64) < u64::from(owned_page_count) {
        return SlotPlan::Create;
    }
    if !replace_current_when_full {
        return SlotPlan::NoFreeSlot;
    }
    editable
        .first()
        .map_or(SlotPlan::NoFreeSlot, |p| SlotPlan::Replace(p.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::static_catalog::{RuneTree, StaticCatalog};
    use proptest::prelude::*;

    fn catalog() -> StaticCatalog {
        StaticCatalog {
            version: "test".into(),
            rune_trees: vec![
                RuneTree {
                    id: 8000,
                    name: "Precision".into(),
                    icon: String::new(),
                    slots: vec![
                        vec![8005, 8008, 8021, 8010],
                        vec![9101, 9111, 8009],
                        vec![9104, 9105, 9103],
                        vec![8014, 8017, 8299],
                    ],
                },
                RuneTree {
                    id: 8200,
                    name: "Sorcery".into(),
                    icon: String::new(),
                    slots: vec![
                        vec![8214, 8229, 8230],
                        vec![8224, 8226, 8275],
                        vec![8210, 8234, 8233],
                        vec![8237, 8232, 8236],
                    ],
                },
            ],
            stat_shard_rows: vec![
                vec![5008, 5005, 5007],
                vec![5008, 5010, 5001],
                vec![5011, 5013, 5001],
            ],
            ..StaticCatalog::default()
        }
    }

    fn valid() -> RuneSelection {
        RuneSelection {
            primary_style_id: 8000,
            sub_style_id: 8200,
            perk_ids: vec![8008, 9101, 9104, 8017, 8233, 8236, 5005, 5008, 5011],
        }
    }

    #[test]
    fn accepts_valid_page() {
        assert_eq!(validate(&valid(), &catalog()), Ok(()));
    }

    #[test]
    fn rejects_keystone_from_wrong_tree() {
        let mut s = valid();
        s.perk_ids[0] = 8229;
        assert_eq!(
            validate(&s, &catalog()),
            Err(RuneError::InvalidPerk {
                position: 0,
                perk: 8229
            })
        );
    }

    #[test]
    fn rejects_two_secondaries_from_one_row() {
        let mut s = valid();
        s.perk_ids[5] = 8234; // same row as 8233
        assert_eq!(validate(&s, &catalog()), Err(RuneError::SecondarySameRow));
    }

    #[test]
    fn rejects_removed_tree() {
        let mut s = valid();
        s.sub_style_id = 8400;
        assert_eq!(validate(&s, &catalog()), Err(RuneError::UnknownTree(8400)));
    }

    fn page(id: i64, name: &str, current: bool) -> ExistingPage {
        ExistingPage {
            id,
            name: name.into(),
            is_editable: true,
            is_deletable: true,
            current,
        }
    }

    #[test]
    fn reuses_our_own_page_first() {
        let pages = [page(1, "Mine", true), page(2, "LoL Overlay: Ahri", false)];
        assert_eq!(plan_slot(&pages, 2, false), SlotPlan::Replace(2));
    }

    #[test]
    fn overwrites_active_page() {
        assert_eq!(
            plan_slot(&[page(1, "Mine", true)], 2, false),
            SlotPlan::Replace(1)
        );
    }

    #[test]
    fn creates_when_a_slot_is_free_and_none_active() {
        assert_eq!(
            plan_slot(&[page(1, "Mine", false)], 2, false),
            SlotPlan::Create
        );
    }

    #[test]
    fn respects_user_choice_when_full_and_none_active() {
        let pages = [page(1, "Mine", false), page(2, "Other", false)];
        assert_eq!(plan_slot(&pages, 2, false), SlotPlan::NoFreeSlot);
        assert_eq!(plan_slot(&pages, 2, true), SlotPlan::Replace(1));
    }

    #[test]
    fn never_touches_default_pages() {
        let default = ExistingPage {
            id: 9,
            name: "Domination".into(),
            is_editable: false,
            is_deletable: false,
            current: true,
        };
        assert_eq!(plan_slot(&[default], 0, true), SlotPlan::NoFreeSlot);
    }

    proptest! {
        /// Whatever the client state, we only ever delete an editable, deletable page.
        #[test]
        fn only_deletes_deletable_pages(
            flags in proptest::collection::vec((any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()), 0..8),
            owned in 0u32..6,
            replace in any::<bool>(),
        ) {
            let pages: Vec<ExistingPage> = flags.iter().enumerate().map(|(i, (ed, del, cur, ours))| ExistingPage {
                id: i as i64,
                name: if *ours { format!("{PAGE_NAME_PREFIX}X") } else { "Mine".into() },
                is_editable: *ed, is_deletable: *del, current: *cur,
            }).collect();
            if let SlotPlan::Replace(id) = plan_slot(&pages, owned, replace) {
                let p = pages.iter().find(|p| p.id == id).unwrap();
                prop_assert!(p.is_editable && p.is_deletable);
            }
        }
    }
}
