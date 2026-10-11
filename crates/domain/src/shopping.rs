//! In-game shopping: which recommended items are still to buy, which one is
//! next, and how far the player is from it (components owned, gold left).

use crate::builds::{BlockKind, ItemBlock};
use crate::static_catalog::StaticCatalog;
use std::collections::{BTreeSet, HashMap};

/// Items in the inventory right now: item id and how many.
pub type Inventory = [(i64, u32)];

/// Recipes deeper than this are not expanded (real ones are 3 levels).
const MAX_RECIPE_DEPTH: usize = 4;

/// Basic Boots; every upgraded pair is built from it.
const BASIC_BOOTS: i64 = 1001;

/// Every item seen in the player's inventory this game.
///
/// "Ever seen", not "held now": a drunk potion or a component merged into an
/// upgrade has still been bought.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Purchases {
    seen: BTreeSet<i64>,
}

impl Purchases {
    /// Records the current inventory. Returns whether anything new was seen.
    pub fn observe(&mut self, inventory: &Inventory) -> bool {
        let before = self.seen.len();
        self.seen
            .extend(inventory.iter().filter(|(_, n)| *n > 0).map(|(id, _)| *id));
        self.seen.len() != before
    }

    pub fn contains(&self, item_id: i64) -> bool {
        self.seen.contains(&item_id)
    }
}

/// Whether `item_id` is built from `part`, at any depth.
fn built_from(item_id: i64, part: i64, catalog: &StaticCatalog, depth: usize) -> bool {
    depth < MAX_RECIPE_DEPTH
        && catalog.items.get(&item_id).is_some_and(|i| {
            i.from
                .iter()
                .any(|c| *c == part || built_from(*c, part, catalog, depth + 1))
        })
}

/// The recommended blocks minus what was bought.
///
/// Starting items are an early-game section: they disappear as a whole once
/// the player has bought anything that costs gold outside them (free items
/// like the trinket do not count), and individually as they are bought.
/// Boots go once any upgraded pair is bought, recommended or not: only one
/// pair can be worn. Empty blocks are dropped.
pub fn remaining_blocks(
    blocks: &[ItemBlock],
    purchases: &Purchases,
    catalog: &StaticCatalog,
) -> Vec<ItemBlock> {
    let starter_items: BTreeSet<i64> = blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Starter)
        .flat_map(|b| b.items.iter().copied())
        .collect();
    let moved_on = purchases.seen.iter().any(|id| {
        !starter_items.contains(id) && catalog.items.get(id).is_some_and(|i| i.total_gold > 0)
    });
    let has_boots = purchases
        .seen
        .iter()
        .any(|id| built_from(*id, BASIC_BOOTS, catalog, 0));

    blocks
        .iter()
        .filter(|b| !(b.kind == BlockKind::Starter && moved_on))
        .filter(|b| !(b.kind == BlockKind::Boots && has_boots))
        .filter_map(|b| {
            let (items, pick_rates): (Vec<i64>, Vec<Option<f64>>) = b
                .items
                .iter()
                .enumerate()
                .filter(|(_, id)| !purchases.contains(**id))
                .map(|(i, id)| (*id, b.pick_rates.get(i).copied()))
                .unzip();
            (!items.is_empty()).then(|| ItemBlock {
                title: b.title.clone(),
                items,
                kind: b.kind,
                pick_rates: if b.pick_rates.is_empty() {
                    Vec::new()
                } else {
                    pick_rates
                        .into_iter()
                        .map(Option::unwrap_or_default)
                        .collect()
                },
            })
        })
        .collect()
}

fn first_of(remaining: &[ItemBlock], kind: BlockKind) -> Option<i64> {
    remaining
        .iter()
        .filter(|b| b.kind == kind)
        .find_map(|b| b.items.first().copied())
}

/// The item to work towards: the first remaining core item, then
/// situational, then anything else. Boots are left out: they have their own
/// slot ([`next_boots`]) because players buy them at different times.
pub fn next_item(remaining: &[ItemBlock]) -> Option<i64> {
    first_of(remaining, BlockKind::Core)
        .or_else(|| first_of(remaining, BlockKind::Situational))
        .or_else(|| first_of(remaining, BlockKind::Other))
}

/// The recommended boots, until bought.
pub fn next_boots(remaining: &[ItemBlock]) -> Option<i64> {
    first_of(remaining, BlockKind::Boots)
}

/// Starting items still to buy, while the game is at its start (see
/// [`remaining_blocks`]); they come before everything else.
pub fn starting_items(remaining: &[ItemBlock]) -> Option<&ItemBlock> {
    remaining.iter().find(|b| b.kind == BlockKind::Starter)
}

/// One item in a recipe tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeNode {
    pub item_id: i64,
    /// Full shop price.
    pub cost: u32,
    /// Held in the inventory (its own components are then not shown).
    pub owned: bool,
    /// Gold still needed for this node, given what is owned beneath it.
    pub remaining_cost: u32,
    /// Not owned, and the player has enough gold for it right now.
    pub affordable: bool,
    pub components: Vec<RecipeNode>,
}

/// Breaks `item_id` into its recipe, matching components the player holds.
/// The root's `remaining_cost` is what the shop will charge for it.
///
/// Held components are matched level by level, so an owned Lost Chapter
/// counts as itself rather than lending its Amplifying Tome elsewhere. Each
/// held item is used once.
pub fn plan_item(
    item_id: i64,
    inventory: &Inventory,
    gold: u32,
    catalog: &StaticCatalog,
) -> RecipeNode {
    let mut available: HashMap<i64, u32> = HashMap::new();
    for (id, n) in inventory {
        *available.entry(*id).or_default() += n;
    }
    expand(item_id, false, &mut available, gold, catalog, 0)
}

fn take(available: &mut HashMap<i64, u32>, id: i64) -> bool {
    match available.get_mut(&id) {
        Some(n) if *n > 0 => {
            *n -= 1;
            true
        }
        _ => false,
    }
}

fn expand(
    item_id: i64,
    owned: bool,
    available: &mut HashMap<i64, u32>,
    gold: u32,
    catalog: &StaticCatalog,
    depth: usize,
) -> RecipeNode {
    let info = catalog.items.get(&item_id);
    let cost = info.map_or(0, |i| i.total_gold);
    if owned {
        return RecipeNode {
            item_id,
            cost,
            owned: true,
            remaining_cost: 0,
            affordable: false,
            components: Vec::new(),
        };
    }
    let from = match info {
        Some(i) if depth < MAX_RECIPE_DEPTH => i.from.as_slice(),
        _ => &[],
    };
    // First claim held components at this level, then expand the rest.
    let held: Vec<bool> = from.iter().map(|id| take(available, *id)).collect();
    let components: Vec<RecipeNode> = from
        .iter()
        .zip(held)
        .map(|(id, is_held)| expand(*id, is_held, available, gold, catalog, depth + 1))
        .collect();
    let covered: u32 = components
        .iter()
        .map(|c| c.cost.saturating_sub(c.remaining_cost))
        .sum();
    let remaining_cost = cost.saturating_sub(covered);
    RecipeNode {
        item_id,
        cost,
        owned: false,
        remaining_cost,
        affordable: gold >= remaining_cost,
        components,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::static_catalog::ItemInfo;

    const LUDENS: i64 = 6655;
    const LOST_CHAPTER: i64 = 3802;
    const ALTERNATOR: i64 = 3145;
    const TOME: i64 = 1052;
    const SAPPHIRE: i64 = 1027;
    const SHOES: i64 = 3020;
    const BOOTS: i64 = 1001;
    const RING: i64 = 1056;
    const POTION: i64 = 2003;
    const TRINKET: i64 = 3340;
    const RYLAIS: i64 = 3116;
    const MERCS: i64 = 3111;
    const MANTLE: i64 = 1033;

    fn item(name: &str, total_gold: u32, from: &[i64]) -> ItemInfo {
        ItemInfo {
            name: name.into(),
            total_gold,
            from: from.to_vec(),
        }
    }

    fn catalog() -> StaticCatalog {
        let mut c = StaticCatalog::default();
        c.items.extend([
            (
                LUDENS,
                item("Luden's Echo", 2750, &[LOST_CHAPTER, ALTERNATOR]),
            ),
            (LOST_CHAPTER, item("Lost Chapter", 1200, &[TOME, SAPPHIRE])),
            (ALTERNATOR, item("Hextech Alternator", 1100, &[TOME, TOME])),
            (TOME, item("Amplifying Tome", 400, &[])),
            (SAPPHIRE, item("Sapphire Crystal", 300, &[])),
            (SHOES, item("Sorcerer's Shoes", 1100, &[BOOTS])),
            (BOOTS, item("Boots", 300, &[])),
            (RING, item("Doran's Ring", 400, &[])),
            (POTION, item("Health Potion", 50, &[])),
            (TRINKET, item("Stealth Ward", 0, &[])),
            (RYLAIS, item("Rylai's Crystal Scepter", 2600, &[])),
            (MERCS, item("Mercury's Treads", 1250, &[BOOTS, MANTLE])),
            (MANTLE, item("Null-Magic Mantle", 400, &[])),
        ]);
        c
    }

    fn block(kind: BlockKind, items: &[i64]) -> ItemBlock {
        ItemBlock {
            title: format!("{kind:?}"),
            items: items.to_vec(),
            kind,
            pick_rates: Vec::new(),
        }
    }

    fn build() -> Vec<ItemBlock> {
        vec![
            block(BlockKind::Starter, &[RING, POTION, POTION]),
            block(BlockKind::Core, &[LUDENS, RYLAIS]),
            block(BlockKind::Boots, &[SHOES]),
            ItemBlock {
                pick_rates: vec![0.35],
                ..block(BlockKind::Situational, &[RYLAIS])
            },
        ]
    }

    fn bought(inventories: &[&Inventory]) -> Purchases {
        let mut p = Purchases::default();
        for inv in inventories {
            p.observe(inv);
        }
        p
    }

    #[test]
    fn everything_is_left_at_the_start_despite_the_free_trinket() {
        let p = bought(&[&[(TRINKET, 1)]]);
        assert_eq!(remaining_blocks(&build(), &p, &catalog()), build());
    }

    #[test]
    fn bought_starting_items_disappear_one_by_one() {
        let p = bought(&[&[(TRINKET, 1), (RING, 1)]]);
        let left = remaining_blocks(&build(), &p, &catalog());
        assert_eq!(left[0].items, [POTION, POTION]);
    }

    #[test]
    fn starting_items_go_once_anything_else_is_bought() {
        let p = bought(&[&[(RING, 1), (POTION, 2)], &[(RING, 1), (BOOTS, 1)]]);
        let left = remaining_blocks(&build(), &p, &catalog());
        assert!(
            left.iter().all(|b| b.kind != BlockKind::Starter),
            "{left:?}"
        );
    }

    #[test]
    fn a_drunk_potion_stays_bought() {
        let p = bought(&[&[(POTION, 1)], &[]]);
        assert!(p.contains(POTION));
    }

    #[test]
    fn bought_items_leave_every_block_and_empty_blocks_vanish() {
        let p = bought(&[&[(RYLAIS, 1), (SHOES, 1)]]);
        let left = remaining_blocks(&build(), &p, &catalog());
        let kinds: Vec<BlockKind> = left.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, [BlockKind::Core]);
        assert_eq!(left[0].items, [LUDENS]);
    }

    #[test]
    fn pick_rates_follow_their_items() {
        let mut blocks = build();
        blocks[3] = ItemBlock {
            pick_rates: vec![0.6, 0.35],
            ..block(BlockKind::Situational, &[SHOES, RYLAIS])
        };
        let p = bought(&[&[(SHOES, 1)]]);
        let left = remaining_blocks(&blocks, &p, &catalog());
        let situational = left
            .iter()
            .find(|b| b.kind == BlockKind::Situational)
            .unwrap();
        assert_eq!(situational.items, [RYLAIS]);
        assert_eq!(situational.pick_rates, [0.35]);
    }

    #[test]
    fn next_item_skips_boots_which_have_their_own_slot() {
        assert_eq!(next_item(&build()), Some(LUDENS));
        assert_eq!(next_boots(&build()), Some(SHOES));
        let p = bought(&[&[(LUDENS, 1), (RYLAIS, 1)]]);
        let left = remaining_blocks(&build(), &p, &catalog());
        assert_eq!(next_item(&left), None, "only boots are left");
        assert_eq!(next_boots(&left), Some(SHOES));
        assert_eq!(next_item(&[block(BlockKind::Starter, &[RING])]), None);
    }

    #[test]
    fn other_upgraded_boots_fill_the_boots_slot() {
        // Mercury's Treads instead of the recommended Sorcerer's Shoes: a
        // champion wears one pair, so the boots are done.
        let p = bought(&[&[(MERCS, 1)]]);
        let left = remaining_blocks(&build(), &p, &catalog());
        assert!(left.iter().all(|b| b.kind != BlockKind::Boots), "{left:?}");
        assert_eq!(next_boots(&left), None);
    }

    #[test]
    fn basic_boots_alone_still_leave_the_upgrade_to_buy() {
        let p = bought(&[&[(BOOTS, 1)]]);
        let left = remaining_blocks(&build(), &p, &catalog());
        assert_eq!(next_boots(&left), Some(SHOES));
    }

    #[test]
    fn starting_items_come_first_until_the_game_moves_on() {
        let start = remaining_blocks(&build(), &Purchases::default(), &catalog());
        assert_eq!(
            starting_items(&start).map(|b| b.items.clone()),
            Some(vec![RING, POTION, POTION])
        );
        let p = bought(&[&[(BOOTS, 1)]]);
        let later = remaining_blocks(&build(), &p, &catalog());
        assert_eq!(starting_items(&later), None);
    }

    #[test]
    fn plan_counts_owned_components_and_gold_left() {
        // Holding a Lost Chapter and one Tome, with 700 gold.
        let plan = plan_item(LUDENS, &[(LOST_CHAPTER, 1), (TOME, 1)], 700, &catalog());
        let root = &plan;
        assert_eq!(root.cost, 2750);
        let [chapter, alternator] = root.components.as_slice() else {
            panic!("{root:?}")
        };
        assert!(chapter.owned && chapter.components.is_empty());
        assert!(!alternator.owned);
        assert!(
            alternator.components[0].owned,
            "the Tome goes to the Alternator"
        );
        assert!(!alternator.components[1].owned);
        assert_eq!(alternator.remaining_cost, 700);
        assert!(alternator.affordable);
        // 2750 - 1200 (Lost Chapter) - 400 (Tome)
        assert_eq!(root.remaining_cost, 1150);
        assert!(!root.affordable);
    }

    #[test]
    fn owned_intermediate_is_matched_before_its_parts() {
        // Two Tomes and a Lost Chapter: the Chapter counts whole, both Tomes
        // go to the Alternator.
        let plan = plan_item(LUDENS, &[(TOME, 2), (LOST_CHAPTER, 1)], 0, &catalog());
        assert_eq!(plan.remaining_cost, 2750 - 1200 - 800);
        assert!(plan
            .components
            .iter()
            .all(|c| c.owned || c.components.iter().all(|g| g.owned)));
    }

    #[test]
    fn nothing_owned_means_full_price() {
        let plan = plan_item(SHOES, &[], 2000, &catalog());
        assert_eq!(plan.remaining_cost, 1100);
        assert!(plan.affordable);
        assert!(plan.components[0].affordable);
    }

    #[test]
    fn unknown_items_do_not_break_the_plan() {
        let plan = plan_item(424_242, &[], 100, &catalog());
        assert_eq!(plan.cost, 0);
        assert!(plan.components.is_empty());
    }

    #[test]
    fn one_held_component_is_not_counted_twice() {
        // One Tome, and the Alternator needs two.
        let plan = plan_item(ALTERNATOR, &[(TOME, 1)], 0, &catalog());
        let owned = plan.components.iter().filter(|c| c.owned).count();
        assert_eq!(owned, 1);
        assert_eq!(plan.remaining_cost, 1100 - 400);
    }

    fn owned_value(node: &RecipeNode) -> u32 {
        if node.owned {
            node.cost
        } else {
            node.components.iter().map(owned_value).sum()
        }
    }

    proptest::proptest! {
        /// Whatever the inventory, the price left is the full price minus
        /// what is held in the recipe, never more than the full price, and
        /// gold decides affordability.
        #[test]
        fn the_price_left_is_the_price_minus_held_parts(
            target in proptest::sample::select(vec![LUDENS, LOST_CHAPTER, ALTERNATOR, SHOES, MERCS]),
            held in proptest::collection::vec(
                (proptest::sample::select(vec![LOST_CHAPTER, ALTERNATOR, TOME, SAPPHIRE, BOOTS, MANTLE]), 1u32..3),
                0..6,
            ),
            gold in 0u32..4000,
        ) {
            let plan = plan_item(target, &held, gold, &catalog());
            proptest::prop_assert!(!plan.owned);
            proptest::prop_assert!(plan.remaining_cost <= plan.cost);
            proptest::prop_assert_eq!(plan.remaining_cost, plan.cost - owned_value(&plan));
            proptest::prop_assert_eq!(plan.affordable, gold >= plan.remaining_cost);
            // Each held item is matched at most as many times as it is held.
            let mut used = std::collections::HashMap::<i64, u32>::new();
            fn count(n: &RecipeNode, used: &mut std::collections::HashMap<i64, u32>) {
                if n.owned {
                    *used.entry(n.item_id).or_default() += 1;
                }
                n.components.iter().for_each(|c| count(c, used));
            }
            count(&plan, &mut used);
            for (id, n) in used {
                let have: u32 = held.iter().filter(|(h, _)| *h == id).map(|(_, c)| c).sum();
                proptest::prop_assert!(n <= have, "{id} used {n} times, held {have}");
            }
        }
    }
}
