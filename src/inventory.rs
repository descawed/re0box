use binrw::binrw;

const BAG_SIZE: usize = 6;
const SLOT_TWO: i32 = 180;

#[binrw]
#[derive(Debug, Default, Clone)]
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
        !(self.items.iter().step_by(2).any(|i| i.id == SLOT_TWO)
        // if there's an empty slot followed by a non-empty slot, we're not organized
        || self.items.iter().skip_while(|i| !i.is_empty()).any(|i| !i.is_empty()))
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
            || self
                .items
                .get(index + 1)
                .map(|i| i.id == SLOT_TWO)
                .unwrap_or(false)
    }

    pub fn is_slot_two(&self, index: usize) -> bool {
        self.items[index].id == SLOT_TWO
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
        if !self.view.is_organized() {
            return;
        }

        let view_end = self.index + BAG_SIZE;
        let view_slice = &mut self.items[self.index..view_end];
        view_slice.clone_from_slice(&self.view.items);
        // if any items were removed from the view, we should shift up the contents of the box to
        // to fill the empty space. the game organizes the view for us when things are moved around,
        // so any empty spaces should always be at the end.
        let num_empty: usize = view_slice
            .iter()
            .map(|i| if i.is_empty() { 1 } else { 0 })
            .sum();
        if num_empty > 0 && !self.items.get(view_end).map_or(true, Item::is_empty) {
            let remove_start = view_end - num_empty;
            self.items.drain(remove_start..view_end);
            // now we need to check and see if any two-slot items have ended up at an odd index.
            // if they have, we need to find the range of two-slot items at odd indexes, then move
            // them back one slot and move the item that was before them to the end.
            let mut check_start = remove_start;
            loop {
                let mut iter = self
                    .items
                    .iter()
                    .enumerate()
                    .skip(check_start & !1)
                    .step_by(2);
                if let Some((bad_index, _)) = iter.find(|(_, i)| i.id == SLOT_TWO) {
                    // the first half of the item is in the previous slot, so back up 2 to find the
                    // preceding item
                    if bad_index < 2 {
                        panic!(
                            "Box contents are screwed up: half an item at the beginning of the box"
                        );
                    }
                    let range_start = bad_index - 2;

                    let range_end = match iter.find(|(_, i)| i.id != SLOT_TWO) {
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
            self.update_view();
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
        }
    }

    pub fn open(&mut self) {
        if !self.is_open {
            self.is_open = true;
            self.index = 0;
            self.update_view();
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
