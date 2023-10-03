use binrw::binrw;

pub const BAG_SIZE: usize = 6;
const SLOT_TWO: i32 = 180;
const TWO_SLOT_ITEMS: [i32; 9] = [
    5,   // hunting gun
    6,   // shotgun
    7,   // grenade launcher (grenade rounds)
    8,   // grenade launcher (flame rounds)
    9,   // grenade launcher (acid rounds)
    11,  // sub-machine gun
    12,  // invalid weapon with no name, icon, or model
    23,  // rocket launcher
    104, // hookshot
];

#[binrw]
#[derive(Debug, Default, Clone, PartialEq)]
#[repr(C)]
pub struct Item {
    id: i32,
    count: i32,
}

impl Item {
    pub const fn empty() -> Self {
        Self { id: 0, count: 0 }
    }

    pub const fn is_empty(&self) -> bool {
        self.id == 0
    }

    pub const fn is_slot_two(&self) -> bool {
        self.id == SLOT_TWO
    }

    pub fn is_two_slot_item(&self) -> bool {
        TWO_SLOT_ITEMS.iter().any(|i| *i == self.id)
    }
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct Bag {
    unknown00: i32,
    items: [Item; BAG_SIZE],
    personal_item: Item,
    equipped_item_index: i32,
}

impl Bag {
    pub const fn empty() -> Self {
        Self {
            unknown00: 0,
            items: [
                Item::empty(),
                Item::empty(),
                Item::empty(),
                Item::empty(),
                Item::empty(),
                Item::empty(),
            ],
            personal_item: Item::empty(),
            equipped_item_index: -1,
        }
    }

    pub fn is_organized(&self) -> bool {
        // if the second half of a two-slot item is in an even-numbered slot, we're not organized
        !(self.items.iter().step_by(2).any(Item::is_slot_two)
        // if the first half of a two-slot item is in an odd-numbered slot, we're not organized
        || self.items.iter().skip(1).step_by(2).any(Item::is_two_slot_item)
        // if there's an empty slot followed by a non-empty slot, we're not organized
        || self.items.iter().skip_while(|i| !i.is_empty()).any(|i| !i.is_empty()))
    }

    pub fn is_broken(&self) -> bool {
        for (i, item) in self.items.iter().enumerate() {
            // if there's a two-slot item not followed by SLOT_TWO, or a SLOT_TWO preceded by an
            // item that's not two slots, the view is in a broken state
            let is_two_slot_item = item.is_two_slot_item();
            let slot_two_follows = self.items.get(i + 1).map_or(false, Item::is_slot_two);
            if is_two_slot_item != slot_two_follows {
                log::trace!(
                    "View is broken at {}: i is_two_slot_item = {}, i + 1 is_slot_two = {}",
                    i,
                    is_two_slot_item,
                    slot_two_follows
                );
                return true;
            }
        }
        false
    }

    pub fn is_valid(&self) -> bool {
        self.is_organized() && !self.is_broken()
    }

    pub fn can_exchange_double(&self, index: usize) -> bool {
        let num_empty: usize = self
            .items
            .iter()
            .map(|i| if i.is_empty() { 1 } else { 0 })
            .sum();
        // we can exchange a two-slot item if we have at least two free slots, or we have one free
        // slot and the player is exchanging with another item, or the player is exchanging with
        // a two-slot item
        num_empty >= 2
            || (num_empty == 1 && !self.items[index].is_empty())
            || self.items.get(index + 1).map_or(false, Item::is_slot_two)
    }

    pub fn is_slot_two(&self, index: usize) -> bool {
        self.items.get(index).map_or(false, Item::is_slot_two)
    }
}

#[derive(Debug)]
pub struct ItemBox {
    is_open: bool,
    items: Vec<Item>,
    index: usize,
    view: Bag,
}

impl ItemBox {
    pub const fn new() -> Self {
        Self {
            is_open: false,
            items: Vec::new(),
            index: 0,
            view: Bag::empty(),
        }
    }

    fn fix_misaligned(&mut self, mut check_start: usize) {
        // if any items are misaligned, we need to find the range of two-slot items at odd indexes,
        // then move them back one slot and move the item that was before them to the end.
        loop {
            let mut iter = self
                .items
                .iter()
                .enumerate()
                .skip(check_start & !1)
                .step_by(2);
            if let Some((bad_index, _)) = iter.find(|(_, i)| i.is_slot_two()) {
                // the first half of the item is in the previous slot, so back up 2 to find the
                // preceding item
                if bad_index < 2 {
                    log::warn!("Half an item at the beginning of the box. Removing.");
                    self.items.remove(0);
                    continue;
                }

                log::warn!(
                    "Misaligned two-slot item at index {}. Correcting.",
                    bad_index - 1
                );
                let range_start = bad_index - 2;

                let range_end = match iter.find(|(_, i)| !i.is_slot_two()) {
                    Some((i, _)) => i - 1, // back up one because we're iterating by 2
                    None => self.items.len(),
                };

                // this shouldn't happen, but if somehow we ended up with an empty slot in the
                // middle of the box, just delete it
                if self.items[range_start].is_empty() {
                    self.items.remove(range_start);
                } else {
                    self.items[range_start..range_end].rotate_left(1);
                }

                check_start = range_end;
            } else {
                break;
            }
        }
    }

    pub fn organize(&mut self) {
        log::debug!("Organizing box");
        let mut new_items = Vec::with_capacity(self.items.capacity());

        // remove all empty slots and fix any broken two-slot items
        let mut last_item: Option<&Item> = None;
        for (i, item) in self.items.iter().enumerate() {
            let (last_item_id, expect_slot_two) =
                last_item.map_or((0, false), |i| (i.id, i.is_two_slot_item()));
            last_item = Some(item);

            if item.is_slot_two() != expect_slot_two {
                if expect_slot_two {
                    log::warn!("Found two-slot item {} at index {} with no second slot. Inserting slot two.", last_item_id, i - 1);
                    new_items.push(Item {
                        id: SLOT_TWO,
                        count: 1,
                    });
                } else {
                    log::warn!("Found orphaned slot-two at index {}. Removing.", i);
                    continue;
                }
            }

            if item.is_empty() {
                continue;
            }

            new_items.push(item.clone());
        }

        // align any misaligned two-slot items
        self.items = new_items;
        self.fix_misaligned(0);

        log::trace!("Box organized");
    }

    fn update_view(&mut self) {
        let num_items = self.items.len();
        let num_items_ahead = num_items - self.index;
        // make sure we always have at least a view's worth of items
        if num_items_ahead < BAG_SIZE {
            let new_size = num_items + BAG_SIZE - num_items_ahead;
            self.items.resize_with(new_size, Default::default);
        }
        self.view
            .items
            .clone_from_slice(&self.items[self.index..self.index + BAG_SIZE])
    }

    pub fn update_from_view(&mut self) {
        // we want to wait until the game has finished organizing the view before we update
        if !self.view.is_valid() {
            log::debug!("Skipping update_from_view because the view has not yet been organized");
            return;
        }

        let view_end = self.index + BAG_SIZE;
        let view_slice = &mut self.items[self.index..view_end];
        view_slice.clone_from_slice(&self.view.items);
        // re-organize the box to account for any gaps or oddities in the view
        self.organize();
        self.update_view();
        if !self.view.is_valid() {
            log::warn!("View is in an invalid state after updating: {:?}", self);
        }
    }

    pub fn make_room_for_double(&mut self, index: usize) {
        // we only need to do something if we don't already have room
        if !self.view.can_exchange_double(index) {
            let box_index = index + self.index;
            self.items.insert(box_index + 1, Item::empty());
            if index == BAG_SIZE - 1 {
                // if we're trying to exchange with the last slot, shift the previous item outside
                // the view
                self.items.swap(box_index - 1, box_index + 1);
            }
            self.update_view();
            if self.view.is_broken() {
                log::warn!(
                    "View is in a broken state after making room for two-slot item: {:?}",
                    self
                );
            }
        }
    }

    pub fn open(&mut self) {
        if !self.is_open {
            self.is_open = true;
            self.index = 0;
            self.update_view();
            if !self.view.is_valid() {
                log::warn!("View is in an invalid state after opening: {:?}", self);
            }
        }
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn scroll_view(&mut self, offset: isize) -> bool {
        // index must be a multiple of 2; round offset up if it was odd
        let mut new_index = self.index as isize + (offset + 1) & !1;
        if new_index < 0 {
            new_index = 0;
        } else {
            // don't let the index point past the last row (pair of items) in the box
            let last_row_index =
                (self.items.iter().rposition(|i| !i.is_empty()).unwrap_or(0) & !1) as isize;
            if new_index > last_row_index {
                new_index = last_row_index;
            }
        }

        let new_index = new_index as usize;
        if self.index != new_index {
            self.index = new_index;
            self.update_view();
            if !self.view.is_valid() {
                log::warn!("View is in an invalid state after scrolling: {:?}", self);
            }
            true
        } else {
            false
        }
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn view(&mut self) -> &mut Bag {
        &mut self.view
    }

    pub fn get_contents(&self) -> &[Item] {
        &self.items
    }

    pub fn set_contents(&mut self, items: Vec<Item>) {
        // we need to close the box here because it can be left open if the player quits to the
        // title screen with the inventory open
        self.close();
        self.items = items;
        self.index = 0;
        self.update_view();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organize_missing_second_half() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 6, count: 1 }, // shotgun, two-slot item
            Item { id: 55, count: 7 },
            Item { id: 32, count: 15 },
        ]);
        item_box.organize();
        let new_contents = item_box.get_contents();
        let pos = new_contents.iter().position(|i| i.id == 6).unwrap();
        assert_eq!(pos & 1, 0); // must be in an even-numbered (left-hand) slot
        assert!(new_contents[pos + 1].is_slot_two()); // must be followed by SLOT_TWO
        assert_eq!(new_contents.iter().filter(|i| !i.is_empty()).count(), 4); // must have 4 items, counting the newly inserted SLOT_TWO as an item
    }

    #[test]
    fn organize_missing_first_half() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 55, count: 7 },
            Item {
                id: SLOT_TWO,
                count: 1,
            }, // SLOT_TWO but the preceding item is not a two-slot item
            Item { id: 32, count: 15 },
            Item { id: 3, count: 5 },
        ]);
        item_box.organize();
        let new_contents = item_box.get_contents();
        assert!(!new_contents.iter().any(Item::is_slot_two));
        assert_eq!(new_contents.iter().filter(|i| !i.is_empty()).count(), 3);
    }

    #[test]
    fn organize_misaligned() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 55, count: 7 },
            Item { id: 5, count: 1 }, // two-slot item (hunting gun) at an odd-numbered index
            Item {
                id: SLOT_TWO,
                count: 1,
            },
            Item { id: 32, count: 15 },
            Item { id: 3, count: 5 },
        ]);
        item_box.organize();
        let new_contents = item_box.get_contents();
        let pos = new_contents.iter().position(|i| i.id == 5).unwrap();
        assert_eq!(pos & 1, 0); // must be in an even-numbered (left-hand) slot
        assert!(new_contents[pos + 1].is_slot_two()); // must be followed by SLOT_TWO
        assert_eq!(new_contents.iter().filter(|i| !i.is_empty()).count(), 5); // must have the same number of items, counting SLOT_TWO as an item
    }

    #[test]
    fn organize_gaps() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 55, count: 7 },
            Item { id: 0, count: 0 }, // empty slot between non-empty slots
            Item { id: 32, count: 15 },
            Item { id: 3, count: 5 },
        ]);
        item_box.organize();
        let new_contents = item_box.get_contents();
        assert_eq!(new_contents.iter().take_while(|i| !i.is_empty()).count(), 3);
        // empty slot should be gone, any empty slots at the end notwithstanding
    }

    #[test]
    fn make_room_for_double() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 6, count: 1 },
            Item {
                id: SLOT_TWO,
                count: 1,
            },
            Item { id: 55, count: 7 },
            Item { id: 32, count: 15 },
            Item { id: 14, count: 3 },
        ]);
        item_box.open();
        assert!(item_box.view.is_valid());
        // there are five items in the box (counting the SLOT_TWO item) so there should be five occupied
        // slots in the view and one empty slot at the end
        let view = item_box.view();
        assert_eq!(view.items.iter().filter(|i| i.is_empty()).count(), 1);
        assert!(view.items[BAG_SIZE - 1].is_empty());
        item_box.make_room_for_double(BAG_SIZE - 1);
        // there should now be two empty slots at the end of the view to make room for the two-slot item
        let view = item_box.view();
        assert!(view.items[BAG_SIZE - 2].is_empty());
        assert!(view.items[BAG_SIZE - 1].is_empty());
    }

    #[test]
    fn scroll() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 6, count: 1 },
            Item {
                id: SLOT_TWO,
                count: 1,
            },
            Item { id: 104, count: 1 },
            Item {
                id: SLOT_TWO,
                count: 1,
            },
            Item { id: 55, count: 7 },
            Item { id: 32, count: 15 },
            Item { id: 14, count: 3 },
            Item { id: 4, count: 7 },
        ]);
        item_box.open();
        assert!(item_box.view().is_valid());
        assert_eq!(item_box.view().items[0].id, 6);
        // not allowed to scroll before the beginning
        item_box.scroll_view(-2);
        assert_eq!(item_box.view().items[0].id, 6);
        item_box.scroll_view(2);
        assert_eq!(item_box.view().items[0].id, 104);
        // we always scroll in increments of 2, so odd numbers should be rounded
        item_box.scroll_view(1);
        assert_eq!(item_box.view().items[0].id, 55);
        // not allowed to scroll past the end
        item_box.scroll_view(1000);
        assert_eq!(item_box.view().items[0].id, 14);
        // negative
        item_box.scroll_view(-2);
        assert_eq!(item_box.view().items[0].id, 55);
    }

    #[test]
    fn update_from_view() {
        let mut item_box = ItemBox::new();
        item_box.set_contents(vec![
            Item { id: 6, count: 1 },
            Item {
                id: SLOT_TWO,
                count: 1,
            },
            Item { id: 104, count: 1 },
            Item {
                id: SLOT_TWO,
                count: 1,
            },
            Item { id: 3, count: 7 },
            Item { id: 32, count: 15 },
            Item { id: 14, count: 3 },
        ]);
        item_box.open();
        assert!(item_box.view().is_valid());
        item_box.view().items[4] = Item { id: 4, count: 9 };
        item_box.update_from_view();
        let contents = item_box.get_contents();
        assert!(!contents.iter().any(|i| i.id == 3));
        assert!(contents.iter().any(|i| i.id == 4));

        item_box.view().items[BAG_SIZE - 1] = Item::empty();
        item_box.update_from_view();
        // 7 items - the 1 we removed == 6
        assert_eq!(
            item_box
                .get_contents()
                .iter()
                .take_while(|i| !i.is_empty())
                .count(),
            6
        );
        let view = item_box.view();
        // there should be no more empty slots in the view because we still have enough items to fill it
        assert!(view.items.iter().all(|i| !i.is_empty()));
        // the last item in the view should be the item that was shifted up from the end, 14
        assert_eq!(view.items[BAG_SIZE - 1].id, 14);
    }

    #[test]
    fn open_and_close() {
        let mut item_box = ItemBox::new();
        assert!(!item_box.is_open()); // box should not start open
        item_box.open();
        assert!(item_box.is_open()); // box should now be open
        item_box.close();
        assert!(!item_box.is_open()); // box should no longer be open
    }
}
