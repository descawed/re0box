use std::arch::asm;
use std::ffi::c_void;
use std::io::Cursor;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use binrw::{binrw, BinReaderExt, BinWrite};
use windows::core::PWSTR;
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Memory::PAGE_READONLY;
use windows::Win32::System::Threading::{
    GetCurrentProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
};

use super::inventory::{Bag, Item};
use super::patch::ByteSearcher;

#[derive(Debug)]
pub struct GameVersion {
    pub version_string: &'static [u8],
    pub get_character_bag: usize,
    pub get_partner_bag: usize,
    pub draw_bags: usize,
    pub play_sound: usize,
    pub get_partner_bag_org: usize,
    pub organize_end1: usize,
    pub organize_end2: usize,
    pub scroll_up_check: usize,
    pub scroll_down_check: usize,
    pub scroll_left_check: usize,
    pub scroll_right_check: usize,
    pub scroll_right_two_check: usize,
    pub get_partner_character: usize,
    pub sub_522a20: usize,
    pub ptr_dcdf3c: usize,
    pub leave_sound_arg: usize,
    pub leave_menu_state: usize,
    pub no_ink_ribbon: usize,
    pub has_ink_ribbon: usize,
    pub typewriter_choice_check: usize,
    pub typewriter_phase_set: usize,
    pub set_room_phase: usize,
    pub prepare_inventory: usize,
    pub inventory_menu_start: usize,
    pub inventory_menu_close: usize,
    pub inventory_change_character: usize,
    pub inventory_open_animation: usize,
    pub play_menu_animation: usize,
    pub exchange_size_check: usize,
    pub sub_4db330: usize,
    pub ptr_dd0bd0: usize,
    pub steam_remote_storage: usize,
    pub load_slot: usize,
    pub post_load: usize,
    pub sub_6fc610: usize,
    pub save_slot: usize,
    pub steam_save: usize,
    pub msg_load1: usize,
    pub msg_load2: usize,
    pub msg_load3: usize,
    pub shaft_check: usize,
    pub new_game: usize,
}

impl GameVersion {
    pub fn str_version(&self) -> String {
        // remove null byte
        String::from_utf8_lossy(&self.version_string[..self.version_string.len() - 1]).into_owned()
    }
}

pub const SUPPORTED_VERSIONS: [GameVersion; 2] = [
    GameVersion {
        version_string: b"MasterRelease Aug 28 2018 14:42:14\0",
        get_character_bag: 0x0050DA80,
        get_partner_bag: 0x004DC8B0,
        draw_bags: 0x005E6ED0,
        play_sound: 0x005EE920,
        get_partner_bag_org: 0x004DC625,
        organize_end1: 0x004DADC7,
        organize_end2: 0x004DADDA,
        scroll_up_check: 0x005E386A,
        scroll_down_check: 0x005E3935,
        scroll_left_check: 0x005E39F1,
        scroll_right_check: 0x005E3AFD,
        scroll_right_two_check: 0x005E3B5A,
        get_partner_character: 0x0066DEC0,
        sub_522a20: 0x00522A20,
        ptr_dcdf3c: 0x00DCDF3C,
        leave_sound_arg: 0x005E3634,
        leave_menu_state: 0x005E363D,
        no_ink_ribbon: 0x0057AD54,
        has_ink_ribbon: 0x0057AD19,
        typewriter_choice_check: 0x0057ADA7,
        typewriter_phase_set: 0x0057ADE6,
        set_room_phase: 0x00610C20,
        prepare_inventory: 0x005D71D0,
        inventory_menu_start: 0x005E1B86,
        inventory_menu_close: 0x005D8983,
        inventory_change_character: 0x005E2BCA,
        inventory_open_animation: 0x005E1B4F,
        play_menu_animation: 0x005DBDF0,
        exchange_size_check: 0x005E3E94,
        sub_4db330: 0x004DB330,
        ptr_dd0bd0: 0x00DD0BD0,
        steam_remote_storage: 0x00CB1440,
        load_slot: 0x006125F1,
        post_load: 0x008B5975,
        sub_6fc610: 0x006FC610,
        save_slot: 0x006134E9,
        steam_save: 0x008B5CC1,
        msg_load1: 0x0040864E,
        msg_load2: 0x005D6471,
        msg_load3: 0x005D67E1,
        shaft_check: 0x005E3D73,
        new_game: 0x0041249C,
    },
    GameVersion {
        version_string: b"MasterRelease Jan 28 2025 16:45:59\0",
        get_character_bag: 0x0050DC70,
        get_partner_bag: 0x004DCA00,
        draw_bags: 0x005E7240,
        play_sound: 0x005EECC0,
        get_partner_bag_org: 0x004DC775,
        organize_end1: 0x004DAF17,
        organize_end2: 0x004DAF2A,
        scroll_up_check: 0x005E3BDA,
        scroll_down_check: 0x005E3CA5,
        scroll_left_check: 0x005E3D61,
        scroll_right_check: 0x005E3E6D,
        scroll_right_two_check: 0x005E3ECA,
        get_partner_character: 0x0096CD30,
        sub_522a20: 0x00522AF0,
        ptr_dcdf3c: 0x00DCBF3C,
        leave_sound_arg: 0x005E39A4,
        leave_menu_state: 0x005E39AD,
        no_ink_ribbon: 0x0057ADA4,
        has_ink_ribbon: 0x0057AD69,
        typewriter_choice_check: 0x0057ADF7,
        typewriter_phase_set: 0x0057AE36,
        set_room_phase: 0x00610E00,
        prepare_inventory: 0x005D7550,
        inventory_menu_start: 0x005E1EF6,
        inventory_menu_close: 0x005D8D03,
        inventory_change_character: 0x005E2F3A,
        inventory_open_animation: 0x005E1EBF,
        play_menu_animation: 0x005DC170,
        exchange_size_check: 0x005E4204,
        sub_4db330: 0x004DB480,
        steam_remote_storage: 0x00CB1458,
        load_slot: 0x006127E1,
        post_load: 0x008B3755,
        sub_6fc610: 0x006FA100,
        ptr_dd0bd0: 0x00DCEBD0,
        save_slot: 0x006136D9,
        steam_save: 0x008B3AA1,
        msg_load1: 0x0040847E,
        msg_load2: 0x005D67F1,
        msg_load3: 0x005D6B61,
        shaft_check: 0x005E40E3,
        new_game: 0x0041240C,
    },
];

pub const MOVE_SELECTION_SOUND: i32 = 2050;
pub const FAIL_SOUND: i32 = 2053;
pub const NUM_SAVE_SLOTS: usize = 20;
pub const MAGIC: &[u8] = b"IBOX";
pub const UNMODDED_SAVE_SIZE: usize = 2337008; // this is the size of the 20 save slots plus, presumably, a few hundred bytes of header/metadata

#[binrw]
#[derive(Debug, Default)]
struct ItemVec {
    #[bw(calc = items.len() as u32)]
    count: u32,
    #[br(count = count)]
    items: Vec<Item>,
}

impl ItemVec {
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }
}

/// Game API and state information
#[derive(Debug)]
pub struct Game {
    pub user_had_ink_ribbon: bool,
    pub should_open_box: bool,
    is_mod_enabled: bool,
    box_partner: *const c_void,
    original_exchange_state: i8,
    draw_bags: Option<unsafe extern "fastcall" fn(*const c_void) -> *mut Bag>,
    get_character_bag: Option<unsafe extern "fastcall" fn(*const c_void) -> *mut Bag>,
    get_partner_character: Option<unsafe extern "fastcall" fn(*const c_void) -> *const c_void>,
    sub_522a20: Option<unsafe extern "fastcall" fn(*const c_void) -> i32>,
    prepare_inventory: Option<unsafe extern "fastcall" fn(*const c_void) -> bool>,
    sub_4db330: Option<unsafe extern "fastcall" fn(*const c_void) -> i32>,
    play_sound: Option<unsafe extern "C" fn(i32) -> i32>,
    get_remote_storage: *const unsafe extern "C" fn() -> *const *const usize,
    ptr_dcdf3c: *const *const c_void,
    ptr_dd0bd0: *const *const c_void,
    saved_boxes: [ItemVec; NUM_SAVE_SLOTS],
    current_version: Option<&'static GameVersion>,
}

impl Game {
    pub const fn new() -> Self {
        Self {
            user_had_ink_ribbon: false,
            should_open_box: false,
            is_mod_enabled: true,
            box_partner: std::ptr::null(),
            original_exchange_state: 0,
            draw_bags: None,
            get_character_bag: None,
            get_partner_character: None,
            sub_522a20: None,
            prepare_inventory: None,
            sub_4db330: None,
            play_sound: None,
            get_remote_storage: std::ptr::null(),
            ptr_dd0bd0: std::ptr::null(),
            ptr_dcdf3c: std::ptr::null(),
            saved_boxes: [
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
                ItemVec::new(),
            ],
            current_version: None,
        }
    }

    pub unsafe fn init(&mut self, is_mod_enabled: bool) -> Result<()> {
        // identify the current game version
        let mut searcher = ByteSearcher::new();
        searcher.discover_modules()?;
        let searcher = searcher;
        
        for version in &SUPPORTED_VERSIONS {
            if let [Some(_)] = searcher.find_bytes(&[version.version_string], Some(PAGE_READONLY), &["re0hd.exe"])? {
                log::info!("Found game version: {}", version.str_version());
                self.current_version = Some(version);
                break;
            }
        }
        
        let Some(version) = self.current_version else {
            bail!("Unsupported or unknown game version");
        };
        
        self.is_mod_enabled = is_mod_enabled;
        self.draw_bags = Some(std::mem::transmute(version.draw_bags));
        self.get_character_bag = Some(std::mem::transmute(version.get_character_bag));
        self.get_partner_character = Some(std::mem::transmute(version.get_partner_character));
        self.sub_522a20 = Some(std::mem::transmute(version.sub_522a20));
        self.prepare_inventory = Some(std::mem::transmute(version.prepare_inventory));
        self.sub_4db330 = Some(std::mem::transmute(version.sub_4db330));
        self.play_sound = Some(std::mem::transmute(version.play_sound));
        self.get_remote_storage =
            version.steam_remote_storage as *const unsafe extern "C" fn() -> *const *const usize;
        self.ptr_dd0bd0 = version.ptr_dd0bd0 as *const *const c_void;
        self.ptr_dcdf3c = version.ptr_dcdf3c as *const *const c_void;
        
        Ok(())
    }
    
    pub fn version(&self) -> &'static GameVersion {
        self.current_version.unwrap()
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

    pub unsafe fn sub_4db330(&self, unknown: *const c_void) -> i32 {
        self.sub_4db330.unwrap()(unknown)
    }

    pub unsafe fn play_sound(&self, sound_id: i32) -> i32 {
        self.play_sound.unwrap()(sound_id)
    }

    pub unsafe fn get_remote_storage(&self) -> *const *const usize {
        (*self.get_remote_storage)()
    }

    pub fn save_to_slot(&mut self, items: &[Item], index: usize) {
        self.saved_boxes[index].items = Vec::from(if self.is_mod_enabled {
            items
        } else {
            // if the mod is disabled, clear the box in this slot
            &[]
        });
    }

    pub fn save(&self, game_buf: &[u8], filename: *const u8) -> Result<()> {
        let buf = Vec::with_capacity(
            game_buf.len()
                + MAGIC.len()
                + NUM_SAVE_SLOTS * size_of::<u32>()
                + self
                    .saved_boxes
                    .iter()
                    .fold(0, |a, b| a + b.items.len() * size_of::<Item>()),
        );
        let mut writer = Cursor::new(buf);
        game_buf.write(&mut writer)?;
        MAGIC.write(&mut writer)?;
        self.saved_boxes.write_le(&mut writer)?;
        let buf = writer.get_ref();

        // pass our buffer to Steam with thiscall calling convention
        let result: u8;
        unsafe {
            let remote_storage = self.get_remote_storage();

            asm!(
                "push {size}",
                "push {buf}",
                "push {name}",
                "call {func}",
                in("ecx") remote_storage,
                func = in(reg) **remote_storage,
                name = in(reg) filename,
                buf = in(reg) buf.as_ptr(),
                size = in(reg) buf.len(),
                lateout("al") result,
            );
        }

        match result {
            0 => Err(anyhow!("Failed to save file")),
            _ => Ok(()),
        }
    }

    pub fn load_from_slot(&self, index: usize) -> Vec<Item> {
        self.saved_boxes[index].items.clone()
    }

    pub fn clear_save(&mut self) {
        for slot in &mut self.saved_boxes {
            slot.items.clear();
        }
    }

    pub fn load(&mut self, buf: &[u8]) -> Result<()> {
        if buf.len() <= UNMODDED_SAVE_SIZE {
            // this is the first time the mod has been used. clear out the boxes.
            self.clear_save();
            return Ok(());
        }

        let mut reader = Cursor::new(&buf[UNMODDED_SAVE_SIZE..]);
        if reader.read_le::<[u8; 4]>()? != MAGIC {
            // something weird has happened
            return Err(anyhow!(
                "Save file appears to be modded but box data was not correct"
            ));
        }
        self.saved_boxes = reader.read_le()?;

        Ok(())
    }

    pub unsafe fn get_game_dir() -> PathBuf {
        let mut path_buf = [0u16; MAX_PATH as usize];
        let wstr = PWSTR::from_raw(path_buf.as_mut_ptr());
        let mut size = MAX_PATH;
        QueryFullProcessImageNameW(
            GetCurrentProcess(),
            PROCESS_NAME_FORMAT::default(),
            wstr,
            &mut size,
        )
            .ok()
            .and_then(|_| wstr.to_string().ok())
            .and_then(|s| PathBuf::from(s).parent().map(PathBuf::from)).unwrap_or_else(|| PathBuf::from("../"))
    }
}
