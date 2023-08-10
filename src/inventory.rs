const BAG_SIZE: usize = 6;

#[derive(Debug, Default, Clone)]
#[repr(C)]
struct Item {
    id: i32,
    count: i32,
}

impl Item {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            count: 0,
        }
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
            items: [Item::empty(), Item::empty(), Item::empty(), Item::empty(), Item::empty(), Item::empty()],
            personal_item: Item::empty(),
            equipped_item_index: -1,
        }
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
        if num_items_ahead < BAG_SIZE {
            let new_size = num_items + BAG_SIZE - num_items_ahead;
            self.items.resize_with(new_size, Default::default);
        }
        self.view.items.clone_from_slice(&self.items[self.index..self.index+BAG_SIZE])
    }

    fn update_from_view(&mut self) {
        self.items[self.index..self.index+BAG_SIZE].clone_from_slice(&self.view.items);
    }

    pub fn open(&mut self) {
        if !self.is_open {
            self.is_open = true;
            self.index = 0;
            self.update_view();
        }
    }

    pub fn close(&mut self) {
        if self.is_open {
            self.is_open = false;
            self.update_from_view();
        }
    }

    pub fn scroll_view(&mut self, offset: isize) {
        // before we change the view, copy whatever is in it back to the box
        self.update_from_view();

        // index must be a multiple of 2; round offset up if it was odd
        let mut new_index = self.index as isize + (offset + 1) & !1;
        if new_index < 0 {
            new_index = 0;
        } else {
            // don't let the index point past the last row (pair of items) in the box
            let last_row_index = (self.items.iter().rposition(|i| !i.is_empty()).unwrap_or(0) & !1) as isize;
            if new_index > last_row_index {
                new_index = last_row_index;
            }
        }

        self.index = new_index as usize;
        self.update_view();
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn view(&mut self) -> &mut Bag {
        &mut self.view
    }
}