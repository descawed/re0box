use std::ffi::c_void;

use super::inventory::Bag;

pub const GET_CHARACTER_BAG: usize = 0x0050DA80;
pub const GET_PARTNER_BAG: usize = 0x004DC8B0;
pub const DRAW_BAGS: usize = 0x005E6ED0;
pub const GET_PARTNER_BAG_ORG: usize = 0x004DC635;
pub const ORGANIZE_END1: usize = 0x004DADC7;
pub const ORGANIZE_END2: usize = 0x004DADDA;
pub const SCROLL_UP_CHECK: usize = 0x005E386A;
pub const SCROLL_DOWN_CHECK: usize = 0x005E3935;
pub const GET_PARTNER_CHARACTER: usize = 0x0066DEC0;
pub const SUB_522A20: usize = 0x00522A20;
pub const PTR_DCDF3C: usize = 0x00DCDF3C;
pub const LEAVE_SOUND_ARG: usize = 0x005E3634;
pub const LEAVE_MENU_STATE: usize = 0x005E363D;
pub const NO_INK_RIBBON: usize = 0x0057AD54;
pub const HAS_INK_RIBBON: usize = 0x0057AD19;
pub const TYPEWRITER_CHOICE_CHECK: usize = 0x0057ADA7;
pub const TYPEWRITER_PHASE_SET: usize = 0x0057ADE6;
pub const SET_ROOM_PHASE: usize = 0x00610C20;
pub const PREPARE_INVENTORY: usize = 0x005D71D0;
pub const INVENTORY_MENU_START: usize = 0x005E1B86;
pub const INVENTORY_MENU_CLOSE: usize = 0x005D8983;
pub const INVENTORY_CHANGE_CHARACTER: usize = 0x005E2BCA;
pub const INVENTORY_OPEN_ANIMATION: usize = 0x005E1B4F;
pub const PLAY_MENU_ANIMATION: usize = 0x005DBDF0;
pub const PTR_DD0BD0: usize = 0x00DD0BD0;
pub const FAIL_SOUND: i32 = 2053;

/// Game API and state information
#[derive(Debug)]
pub struct Game {
    pub user_had_ink_ribbon: bool,
    pub should_open_box: bool,
    box_partner: *const c_void,
    original_exchange_state: i8,
    draw_bags: Option<unsafe extern "fastcall" fn(*const c_void) -> *mut Bag>,
    get_character_bag: Option<unsafe extern "fastcall" fn(*const c_void) -> *mut Bag>,
    get_partner_character: Option<unsafe extern "fastcall" fn(*const c_void) -> *const c_void>,
    sub_522a20: Option<unsafe extern "fastcall" fn(*const c_void) -> i32>,
    prepare_inventory: Option<unsafe extern "fastcall" fn(*const c_void) -> bool>,
    ptr_dcdf3c: *const *const c_void,
    ptr_dd0bd0: *const *const c_void,
}

impl Game {
    pub const fn new() -> Self {
        Self {
            user_had_ink_ribbon: false,
            should_open_box: false,
            box_partner: std::ptr::null(),
            original_exchange_state: 0,
            draw_bags: None,
            get_character_bag: None,
            get_partner_character: None,
            sub_522a20: None,
            prepare_inventory: None,
            ptr_dd0bd0: std::ptr::null(),
            ptr_dcdf3c: std::ptr::null(),
        }
    }

    pub unsafe fn init(&mut self) {
        self.draw_bags = Some(std::mem::transmute(DRAW_BAGS));
        self.get_character_bag = Some(std::mem::transmute(GET_CHARACTER_BAG));
        self.get_partner_character = Some(std::mem::transmute(GET_PARTNER_CHARACTER));
        self.sub_522a20 = Some(std::mem::transmute(SUB_522A20));
        self.prepare_inventory = Some(std::mem::transmute(PREPARE_INVENTORY));
        self.ptr_dd0bd0 = PTR_DD0BD0 as *const *const c_void;
        self.ptr_dcdf3c = PTR_DCDF3C as *const *const c_void;
    }

    pub unsafe fn init_menu(&mut self, menu: *mut c_void) {
        // always allow exchange on first open because the active character will be the one who
        // opened the box
        let exchange_state = menu.offset(0x28b) as *mut i8;
        self.original_exchange_state = *exchange_state;
        *exchange_state = 0;
        self.box_partner = self.get_partner_character();
    }

    pub unsafe fn update_exchange_state(&mut self, menu: *mut c_void) {
        // if the current character is not the one who opened the box, restore the original exchange
        // state
        *(menu.offset(0x28b) as *mut i8) = if self.get_partner_character() != self.box_partner {
            self.original_exchange_state
        } else {
            0
        };
    }

    pub unsafe fn draw_bags(&self, unknown: *const c_void) -> *mut Bag {
        self.draw_bags.unwrap()(unknown)
    }

    pub unsafe fn get_character_bag(&self, character: *const c_void) -> *mut Bag {
        self.get_character_bag.unwrap()(character)
    }

    pub unsafe fn get_partner_character(&self) -> *const c_void {
        self.get_partner_character.unwrap()(*self.ptr_dcdf3c)
    }

    pub unsafe fn sub_522a20(&self, unknown: *const c_void) -> i32 {
        self.sub_522a20.unwrap()(unknown)
    }

    pub unsafe fn prepare_inventory(&self) -> bool {
        self.prepare_inventory.unwrap()(*self.ptr_dd0bd0)
    }
}
