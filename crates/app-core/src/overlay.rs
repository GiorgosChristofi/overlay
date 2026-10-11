//! The in-game column's view: the build's items minus what was bought, and
//! the next item broken into its recipe with the player's gold.

use domain::builds::{BlockKind, Build, ItemBlock};
use domain::ports::LiveSnapshot;
use domain::shopping::{
    next_boots, next_item, plan_item, remaining_blocks, starting_items, Purchases, RecipeNode,
};
use domain::static_catalog::StaticCatalog;
use ipc_types::{
    OverlayBlockDto, OverlayBlockKind, OverlayCommand, OverlayItemDto, OverlayViewDto,
    RecipeNodeDto, StartingItemsDto,
};
use std::collections::BTreeSet;

/// What the player changed in the column this game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OverlayChoices {
    /// Item to build next instead of the automatic choice.
    pub next: Option<i64>,
    /// Items taken off the list.
    pub removed: BTreeSet<i64>,
}

impl OverlayChoices {
    pub fn apply(&mut self, command: OverlayCommand) {
        match command {
            OverlayCommand::SetNext { item_id } => self.next = Some(i64::from(item_id)),
            OverlayCommand::ClearNext => self.next = None,
            OverlayCommand::Remove { item_id } => {
                let id = i64::from(item_id);
                self.removed.insert(id);
                if self.next == Some(id) {
                    self.next = None;
                }
            }
            OverlayCommand::RestoreRemoved => self.removed.clear(),
        }
    }
}

/// Everything the view is computed from.
#[derive(Debug, Clone, Copy)]
pub struct OverlayInputs<'a> {
    pub build: &'a Build,
    pub champion_name: &'a str,
    pub role: &'a str,
    pub catalog: &'a StaticCatalog,
    pub purchases: &'a Purchases,
    pub choices: &'a OverlayChoices,
    /// `None` outside a running game.
    pub live: Option<&'a LiveSnapshot>,
}

fn kind_dto(kind: BlockKind) -> OverlayBlockKind {
    match kind {
        BlockKind::Starter => OverlayBlockKind::Starter,
        BlockKind::Core => OverlayBlockKind::Core,
        BlockKind::Boots => OverlayBlockKind::Boots,
        BlockKind::Situational => OverlayBlockKind::Situational,
        BlockKind::Other => OverlayBlockKind::Other,
    }
}

fn item_dto(
    id: i64,
    count: u32,
    pick_rate: Option<f64>,
    catalog: &StaticCatalog,
) -> OverlayItemDto {
    #[allow(clippy::cast_possible_truncation)] // a rate in 0..=1
    OverlayItemDto {
        id: i32::try_from(id).unwrap_or(0),
        name: catalog.item_name(id),
        icon_url: catalog.item_icon_url(id),
        cost: catalog.items.get(&id).map_or(0, |i| i.total_gold),
        count,
        pick_rate: pick_rate.map(|r| r as f32),
    }
}

/// One entry per item, repeats folded into a count (2 potions), first
/// pick rate kept.
fn block_dto(block: &ItemBlock, catalog: &StaticCatalog) -> OverlayBlockDto {
    let mut items: Vec<OverlayItemDto> = Vec::new();
    for (i, id) in block.items.iter().enumerate() {
        match items.iter_mut().find(|it| i64::from(it.id) == *id) {
            Some(seen) => seen.count += 1,
            None => items.push(item_dto(*id, 1, block.pick_rates.get(i).copied(), catalog)),
        }
    }
    OverlayBlockDto {
        kind: kind_dto(block.kind),
        title: block.title.clone(),
        items,
    }
}

fn recipe_dto(node: &RecipeNode, catalog: &StaticCatalog) -> RecipeNodeDto {
    RecipeNodeDto {
        item: item_dto(node.item_id, 1, None, catalog),
        owned: node.owned,
        remaining_cost: node.remaining_cost,
        affordable: node.affordable,
        components: node
            .components
            .iter()
            .map(|c| recipe_dto(c, catalog))
            .collect(),
    }
}

pub fn overlay_view(inputs: OverlayInputs<'_>) -> OverlayViewDto {
    let catalog = inputs.catalog;
    let choices = inputs.choices;
    // Items removed this patch cannot be bought; leave them out, like the item
    // set. Items the player removed go too.
    let known: Vec<ItemBlock> = inputs
        .build
        .item_blocks
        .iter()
        .map(|b| {
            let keep: Vec<usize> = (0..b.items.len())
                .filter(|i| {
                    let id = b.items[*i];
                    catalog.items.contains_key(&id) && !choices.removed.contains(&id)
                })
                .collect();
            ItemBlock {
                title: b.title.clone(),
                items: keep.iter().map(|i| b.items[*i]).collect(),
                kind: b.kind,
                pick_rates: keep
                    .iter()
                    .filter_map(|i| b.pick_rates.get(*i).copied())
                    .collect(),
            }
        })
        .collect();
    let remaining = remaining_blocks(&known, inputs.purchases, catalog);
    let (inventory, gold): (&[(i64, u32)], u32) = inputs
        .live
        .map_or((&[], 0), |l| (l.inventory.as_slice(), l.gold));
    // The player's choice wins while it is still on the list (not bought).
    let pinned = choices
        .next
        .filter(|id| remaining.iter().any(|b| b.items.contains(id)));
    let next_id = pinned.or_else(|| next_item(&remaining));
    // Boots keep their own slot, unless the player picked them as next.
    let boots_id = next_boots(&remaining).filter(|id| Some(*id) != next_id);
    let plan = |id: i64| recipe_dto(&plan_item(id, inventory, gold, catalog), catalog);
    let (next, boots) = (next_id.map(plan), boots_id.map(plan));
    let starting = starting_items(&remaining).map(|b| {
        let block = block_dto(b, catalog);
        let cost = block.items.iter().map(|i| i.cost * i.count).sum();
        StartingItemsDto {
            affordable: inputs.live.is_some() && gold >= cost,
            cost,
            items: block.items,
        }
    });
    let removed_count = u32::try_from(choices.removed.len()).unwrap_or(u32::MAX);

    OverlayViewDto {
        champion_name: inputs.champion_name.to_owned(),
        role: inputs.role.to_owned(),
        source: inputs.build.source.clone(),
        gold: inputs.live.map(|l| l.gold),
        starting,
        // Without live data nothing is known to be affordable.
        next: next.map(|n| known_affordable(n, inputs.live.is_some())),
        boots: boots.map(|n| known_affordable(n, inputs.live.is_some())),
        next_pinned: pinned.is_some(),
        removed_count,
        // Starting items are shown in their own slot.
        blocks: remaining
            .iter()
            .filter(|b| b.kind != BlockKind::Starter)
            .map(|b| block_dto(b, catalog))
            .collect(),
    }
}

fn known_affordable(mut node: RecipeNodeDto, live: bool) -> RecipeNodeDto {
    if !live {
        clear_affordable(&mut node);
    }
    node
}

fn clear_affordable(node: &mut RecipeNodeDto) {
    node.affordable = false;
    node.components.iter_mut().for_each(clear_affordable);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use domain::champ_select::Role;
    use domain::static_catalog::ItemInfo;

    fn catalog() -> StaticCatalog {
        let mut c = StaticCatalog {
            image_base_url: "https://cdn.test/img".into(),
            ..StaticCatalog::default()
        };
        let item = |name: &str, total_gold: u32, from: &[i64]| ItemInfo {
            name: name.into(),
            total_gold,
            from: from.to_vec(),
        };
        c.items.extend([
            (1056, item("Doran's Ring", 400, &[])),
            (2003, item("Health Potion", 50, &[])),
            (3020, item("Sorcerer's Shoes", 1100, &[1001])),
            (1001, item("Boots", 300, &[])),
            (3116, item("Rylai's Crystal Scepter", 2600, &[])),
        ]);
        c
    }

    fn build() -> Build {
        let block = |kind, items: &[i64], pick_rates: &[f64]| ItemBlock {
            title: format!("{kind:?}"),
            items: items.to_vec(),
            kind,
            pick_rates: pick_rates.to_vec(),
        };
        Build {
            champion_id: 99,
            role: Role::Middle,
            rune_pages: vec![],
            summoner_spells: None,
            item_blocks: vec![
                block(BlockKind::Starter, &[1056, 2003, 2003], &[]),
                block(BlockKind::Boots, &[3020], &[]),
                block(BlockKind::Situational, &[999_999, 3116], &[0.9, 0.31]),
            ],
            source: "OP.GG".into(),
        }
    }

    fn view(purchases: &Purchases, live: Option<&LiveSnapshot>) -> OverlayViewDto {
        view_with(purchases, &OverlayChoices::default(), live)
    }

    fn view_with(
        purchases: &Purchases,
        choices: &OverlayChoices,
        live: Option<&LiveSnapshot>,
    ) -> OverlayViewDto {
        let (b, c) = (build(), catalog());
        overlay_view(OverlayInputs {
            build: &b,
            champion_name: "Lux",
            role: "Middle",
            catalog: &c,
            purchases,
            choices,
            live,
        })
    }

    fn ids(v: &OverlayViewDto) -> Vec<i32> {
        v.blocks
            .iter()
            .flat_map(|b| b.items.iter().map(|i| i.id))
            .collect()
    }

    #[test]
    fn a_chosen_next_item_replaces_the_automatic_one_until_bought() {
        let mut choices = OverlayChoices::default();
        choices.apply(OverlayCommand::SetNext { item_id: 3116 });
        let v = view_with(&Purchases::default(), &choices, None);
        assert_eq!(v.next.unwrap().item.id, 3116);
        assert!(v.next_pinned);

        let mut bought = Purchases::default();
        bought.observe(&[(3116, 1)]);
        let v = view_with(&bought, &choices, None);
        assert_eq!(v.next, None, "back to automatic: only boots are left");
        assert!(!v.next_pinned);
    }

    #[test]
    fn removed_items_leave_the_list_and_can_come_back() {
        let mut choices = OverlayChoices::default();
        choices.apply(OverlayCommand::SetNext { item_id: 3020 });
        choices.apply(OverlayCommand::Remove { item_id: 3020 });
        assert_eq!(choices.next, None, "removing the chosen item unpins it");
        let v = view_with(&Purchases::default(), &choices, None);
        assert!(!ids(&v).contains(&3020));
        assert_eq!(v.removed_count, 1);
        assert_eq!(v.next.unwrap().item.id, 3116);

        choices.apply(OverlayCommand::RestoreRemoved);
        let v = view_with(&Purchases::default(), &choices, None);
        assert!(ids(&v).contains(&3020));
        assert_eq!(v.removed_count, 0);
    }

    #[test]
    fn before_the_game_shows_everything_with_icons_counts_and_rates() {
        let v = view(&Purchases::default(), None);
        assert_eq!(v.gold, None);
        let starting = v.starting.as_ref().unwrap();
        assert_eq!(starting.items.len(), 2, "two potions are one entry");
        assert_eq!(starting.items[1].count, 2);
        assert_eq!(starting.cost, 400 + 2 * 50);
        assert!(
            !starting.affordable,
            "nothing is affordable without live data"
        );
        assert_eq!(
            starting.items[0].icon_url.as_deref(),
            Some("https://cdn.test/img/item/1056.png")
        );
        assert!(
            v.blocks.iter().all(|b| b.kind != OverlayBlockKind::Starter),
            "starting items are not repeated in the list"
        );
        let situational = &v.blocks[1];
        assert_eq!(situational.items.len(), 1, "unknown items are left out");
        assert_eq!(situational.items[0].pick_rate, Some(0.31));
        assert_eq!(v.next.unwrap().item.id, 3116, "boots are not the next item");
        let boots = v.boots.unwrap();
        assert_eq!(boots.item.name, "Sorcerer's Shoes");
        assert!(!boots.affordable);
    }

    #[test]
    fn starting_items_are_affordable_with_enough_gold() {
        let live = LiveSnapshot {
            gold: 500,
            inventory: vec![(3340, 1)],
        };
        let v = view(&Purchases::default(), Some(&live));
        assert!(v.starting.unwrap().affordable);
    }

    #[test]
    fn in_game_drops_bought_items_and_prices_the_next_one() {
        let live = LiveSnapshot {
            gold: 900,
            inventory: vec![(1056, 1), (1001, 1)],
        };
        let mut purchases = Purchases::default();
        purchases.observe(&live.inventory);
        let v = view(&purchases, Some(&live));
        assert_eq!(v.gold, Some(900));
        assert_eq!(v.starting, None, "boots bought, so the start is over");
        let boots = v.boots.unwrap();
        assert_eq!(boots.remaining_cost, 800);
        assert!(boots.affordable);
        assert!(boots.components[0].owned);
        assert_eq!(v.next.unwrap().item.id, 3116);
    }
}
