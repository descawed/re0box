#![allow(static_mut_refs)]
#![cfg(windows)]

use std::ffi::c_void;
use std::panic;
use std::path::{Path, PathBuf};
use std::str;

use anyhow::Result;
use configparser::ini::Ini;
use simplelog::LevelFilter;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

mod patch;
use patch::*;

mod error;
use error::*;

mod game;
use game::*;

mod inventory;
use inventory::*;

const MSG_DIR: &[u8] = br"nativePC\arc\message\msg_";
// we need static strings that always exist so we can give pointers to the game
const MSG_FILES: [&[u8; 8]; 8] = [
    b"chS_box\0",
    b"chT_box\0",
    b"eng_box\0",
    b"fre_box\0",
    b"ger_box\0",
    b"ita_box\0",
    b"jpn_box\0",
    b"spa_box\0",
];

// I tried the naked-function crate, but it failed to compile for me, complaining about "unknown
// directive" .pushsection. maybe it has something to do with the fact that I'm cross-compiling.
static mut SCROLL_UP_TRAMPOLINE: [u8; 16] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0, 0, 0, 0, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut SCROLL_DOWN_TRAMPOLINE: [u8; 24] = [
    0x50, // push eax
    0x57, // push edi
    0xE8, 0, 0, 0, 0, // call <fn>
    0x83, 0xC4, 0x08, // add esp,8
    0x83, 0xF8, 0x06, // cmp eax,6
    0x0F, 0x8D, 0, 0, 0, 0, // jge <jmp_return>
    0xE9, 0, 0, 0, 0, // jmp <no_jmp_return>
];

static mut SCROLL_LEFT_TRAMPOLINE: [u8; 20] = [
    0x79, 0x0D, // jns done
    0x51, // push ecx
    0x52, // push edx
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x5A, // pop edx
    0x59, // pop ecx
    0xE9, 0, 0, 0, 0, // done: jmp <return>
];

static mut SCROLL_RIGHT_TRAMPOLINE: [u8; 20] = [
    0x83, 0xF8, 0x05, // cmp eax,5
    0x7C, 0x0A, // jl done
    0x50, // push eax
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x08, // add esp,8
    0xE9, 0, 0, 0, 0, // done: jmp <return>
];

static mut SCROLL_RIGHT_TWO_TRAMPOLINE: [u8; 26] = [
    0xFF, 0xB7, 0xBC, 0x02, 0x00, 0x00, // push dword ptr [edi+0x2bc]
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x08, // add esp,8
    0x89, 0x87, 0xBC, 0x02, 0x00, 0x00, // mov dword ptr [edi+0x2bc],eax
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut ORGANIZE_TRAMPOLINE: [u8; 13] = [
    0x60, // pushad
    0xE8, 0, 0, 0, 0,    // call <fn>
    0x61, // popad
    0x83, 0xC4, 0x3C, // add esp, 0x3c
    0xC2, 0x04, 0x00, // retn 4
];

static mut HAS_INK_RIBBON_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0x6A, 0x01, // push 1
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xC7, 0x47, 0x10, 0x00, 0x00, 0x00, 0x40, // mov dword ptr [edi+0x10], 0x40000000
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut NO_INK_RIBBON_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0x6A, 0x00, // push 0
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xC7, 0x47, 0x10, 0x00, 0x00, 0x00, 0x40, // mov dword ptr [edi+0x10], 0x40000000
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut TYPEWRITER_CHOICE_TRAMPOLINE: [u8; 18] = [
    0x8B, 0x74, 0x24, 0x64, // mov esi,[esp+0x64]
    0x60, // pushad
    0x56, // push esi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0xC3, // ret
];

static mut OPEN_BOX_TRAMPOLINE: [u8; 20] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0x74, 0x04, // jz phase
    0xFF, 0x4C, 0x24, 0x04, // dec dword ptr [esp+4]
    0xE9, 0x00, 0x00, 0x00, 0x00, // phase: jmp SetRoomPhase
];

static mut INVENTORY_CLOSE_TRAMPOLINE: [u8; 17] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x61, // popad
    0x8B, 0x46, 0x60, // mov eax,[esi+0x60]
    0x6A, 0x00, // push 0
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut INVENTORY_START_TRAMPOLINE: [u8; 22] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xFF, 0x87, 0x94, 0x02, 0x00, 0x00, // inc dword ptr [edi+0x294]
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut CHANGE_CHARACTER_TRAMPOLINE: [u8; 23] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0x80, 0xBF, 0xCA, 0x02, 0x00, 0x00, 0x01, // cmp byte ptr [edi+0x2ca],1
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut OPEN_ANIMATION_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0x74, 0x08, // jz do_call
    0xC7, 0x44, 0x24, 0x04, 0x01, 0x00, 0x00, 0x00, // mov dword ptr [esp+4],1
    0xE9, 0x00, 0x00, 0x00, 0x00, // do_call: jmp PlayMenuAnimation
];

static mut SIZE_CHECK_TRAMPOLINE: [u8; 16] = [
    0xFF, 0x74, 0x24, 0x20, // push [esp+0x20]
    0x51, // push ecx
    0x57, // push edi
    0x68, 0, 0, 0, 0, // push <return>
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp <fn>
];

static mut LOAD_SLOT_TRAMPOLINE: [u8; 22] = [
    0x60, // pushad
    0x56, // push esi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0x69, 0xF6, 0x50, 0xC8, 0x01, 0x00, // imul esi,0x1c850
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut LOAD_TRAMPOLINE: [u8; 25] = [
    0x55, // push ebp
    0x53, // push ebx
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    // we intentionally don't clean the stack because we're passing the same arguments to the next function
    0x89, 0x44, 0x24, 0x04, // mov [esp+4],eax
    0x8D, 0x4C, 0x24, 0x34, // lea ecx,[esp+0x34]
    0x68, 0, 0, 0, 0, // push <return>
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp sub_6FC610
];

static mut SAVE_SLOT_TRAMPOLINE: [u8; 22] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0x69, 0xFF, 0x50, 0xC8, 0x01, 0x00, // imul edi,0x1c850
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut SAVE_TRAMPOLINE: [u8; 11] = [
    0x50, // push eax
    0x68, 0, 0, 0, 0, // push <return>
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp <fn>
];

static mut MSG_TRAMPOLINE1: [u8; 18] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x89, 0x04, 0x24, // mov [esp],eax
    0x68, 0, 0, 0, 0, // push <msg format>
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut MSG_TRAMPOLINE2: [u8; 18] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x89, 0x04, 0x24, // mov [esp],eax
    0x68, 0, 0, 0, 0, // push <msg format>
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut MSG_TRAMPOLINE3: [u8; 18] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x89, 0x04, 0x24, // mov [esp],eax
    0x68, 0, 0, 0, 0, // push <msg format>
    0xE9, 0, 0, 0, 0, // jmp <return>
];

static mut SHAFT_CHECK_TRAMPOLINE: [u8; 31] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0x68, 0, 0, 0, 0, // push <return>
    0xB8, 0, 0, 0, 0, // mov eax,<original_fn>
    0x74, 0x08, // jz do_jmp
    0x83, 0xC4, 0x04, // add esp,4
    0xB8, 0, 0, 0, 0, // mov eax,<skip_check>
    0xFF, 0xE0, // do_jmp: jmp eax
];

static mut NEW_GAME_TRAMPOLINE: [u8; 12] = [
    0x51, // push ecx
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x59, // pop ecx
    0xE9, 0, 0, 0, 0, // call <original_fn>
];

static mut BOX: ItemBox = ItemBox::new();
static mut GAME: Game = Game::new();

unsafe extern "C" fn new_game() {
    log::debug!("new_game");
    // reset the box when starting a new game
    BOX.set_contents(vec![]);
}

unsafe extern "fastcall" fn should_skip_shaft_check(partner: *const c_void) -> bool {
    log::trace!("should_skip_shaft_check");
    // partner should never be null at this point of the code unless the box is open, but we'll
    // check both anyway just to be safe
    BOX.is_open() || partner.is_null()
}

unsafe extern "C" fn load_msg_file(lang: *const u8) -> *const u8 {
    log::trace!("load_msg_file");
    let lang_slice = std::slice::from_raw_parts(lang, 3);
    if let Some(override_file) = MSG_FILES.iter().find(|f| f.starts_with(lang_slice)) {
        // make sure the file actually exists before we tell the game to load it
        let mut raw_path = [0u8; 36];
        raw_path[..MSG_DIR.len()].copy_from_slice(MSG_DIR);
        let end = MSG_DIR.len() + override_file.len() - 1;
        raw_path[MSG_DIR.len()..end].copy_from_slice(&override_file[..override_file.len() - 1]); // -1 to skip null
        raw_path[end..].copy_from_slice(b".arc");

        let path = Path::new(str::from_utf8_unchecked(&raw_path));
        if path.exists() {
            return (*override_file).as_ptr();
        }
    }

    lang
}

unsafe extern "C" fn save_slot(index: usize) {
    log::debug!("save_slot {}", index);
    GAME.save_to_slot(BOX.get_contents(), index);
}

unsafe extern "stdcall" fn save_data(filename: *const u8, buf: *const u8, size: usize) -> bool {
    log::trace!("save_data");
    if let Err(e) = GAME.save(std::slice::from_raw_parts(buf, size), filename) {
        log::error!("Failed to save: {:?}", e);
        false
    } else {
        true
    }
}

unsafe extern "C" fn load_slot(index: usize) {
    log::debug!("load_slot {}", index);
    BOX.set_contents(GAME.load_from_slot(index));
    // fix the box if we somehow saved it in an invalid state
    BOX.organize();
}

unsafe extern "C" fn load_data(buf: *const u8, size: usize) -> usize {
    log::trace!("load_data");
    if let Err(e) = GAME.load(std::slice::from_raw_parts(buf, size)) {
        log::error!("Failed to load save data: {:?}", e);
        panic!("Failed to load save data");
    }
    UNMODDED_SAVE_SIZE
}

// we use stdcall here because we're returning directly to the game, so we need to clean the stack
unsafe extern "stdcall" fn make_room_for_double(
    menu: *const c_void,
    unknown: *const c_void,
    item_size: usize,
) -> i32 {
    log::trace!("make_room_for_double");
    if BOX.is_open() {
        if item_size > 1 {
            let index = *(menu.offset(0x2bc) as *const usize);
            BOX.make_room_for_double(index);
        }

        // we just always say we have enough space
        2
    } else {
        // if the box isn't open, just forward the call to the original function
        GAME.sub_4db330(unknown)
    }
}

unsafe extern "fastcall" fn show_partner_inventory(menu: *mut c_void) -> bool {
    log::trace!("show_partner_inventory");
    if BOX.is_open() {
        // flag that the partner inventory is displayed
        *(menu.offset(0x2ca) as *mut bool) = true;
    }

    BOX.is_open()
}

unsafe fn close_box() {
    log::debug!("close_box");
    BOX.close();
    // fix the box if it somehow got into an invalid state
    BOX.organize();
}

unsafe extern "C" fn change_character(menu: *mut c_void) {
    log::trace!("change_character");
    if BOX.is_open() {
        GAME.update_exchange_state(menu);
    }
}

unsafe extern "C" fn menu_setup(menu: *mut c_void) {
    log::trace!("menu_setup");
    if BOX.is_open() {
        GAME.init_menu(menu);
    }
}

unsafe fn open_box() -> bool {
    log::debug!("open_box");
    if GAME.should_open_box {
        log::debug!("Opening item box");
        GAME.prepare_inventory();
        BOX.open();
    }

    GAME.should_open_box
}

unsafe extern "C" fn check_typewriter_choice(choice: i32) -> bool {
    log::trace!("check_typewriter_choice");
    // "no" is option 2 for both messages
    if choice == 2 {
        true
    } else {
        // otherwise, record whether we need to open the box
        // there's only a choice 3 if the user had an ink ribbon, in which case it's "Use". if they
        // didn't have an ink ribbon, the only other option is "Yes".
        GAME.should_open_box = choice == 3 || !GAME.user_had_ink_ribbon;
        false
    }
}

unsafe extern "C" fn track_typewriter_message(had_ink_ribbon: bool) {
    log::trace!("track_typewriter_message {}", had_ink_ribbon);
    GAME.user_had_ink_ribbon = had_ink_ribbon;
}

unsafe extern "C" fn scroll_left(unknown: *const c_void) -> i32 {
    log::trace!("scroll_left");
    if BOX.is_open() && BOX.scroll_view(-2) {
        GAME.draw_bags(unknown);
        if BOX.view().is_slot_two(1) {
            0
        } else {
            1
        }
    } else {
        (BAG_SIZE - 1) as i32 // we're already at the top, so wrap around to the last cell in the view
    }
}

unsafe extern "C" fn scroll_right(unknown: *const c_void, new_index: i32) -> i32 {
    log::trace!("scroll_right {}", new_index);
    let bag_size = BAG_SIZE as i32;
    if BOX.is_open()
        && (new_index == bag_size
            || (new_index == bag_size - 1 && BOX.view().is_slot_two(new_index as usize)))
        && BOX.scroll_view(2)
    {
        GAME.draw_bags(unknown);
        bag_size - 2
    } else {
        new_index % bag_size
    }
}

unsafe extern "C" fn scroll_up(unknown: *const c_void) {
    log::trace!("scroll_up");
    if BOX.is_open() && BOX.scroll_view(-2) {
        // by default the inventory display doesn't update at this point, so we have to do it ourselves
        GAME.draw_bags(unknown);
        // if we've ended up on the second slot of a two-slot item, back up one
        let selection_index = unknown.offset(0x2bc) as *mut usize;
        if BOX.view().is_slot_two(*selection_index) {
            *selection_index -= 1;
        }
        // the sound doesn't normally play when moving the cursor past the edges of the inventory,
        // so we have to do that, too
        GAME.play_sound(MOVE_SELECTION_SOUND);
    }
}

unsafe extern "C" fn scroll_down(unknown: *const c_void, mut new_index: i32) -> i32 {
    log::trace!("scroll_down {}", new_index);
    if BOX.is_open() {
        if new_index >= BAG_SIZE as i32 && BOX.scroll_view(2) {
            // by default the inventory display doesn't update at this point, so we have to do it ourselves
            GAME.draw_bags(unknown);
            // the sound doesn't normally play when moving the cursor past the edges of the inventory,
            // so we have to do that, too
            GAME.play_sound(MOVE_SELECTION_SOUND);
            new_index -= 2;
        }
        // if we've ended up on the second slot of a two-slot item, back up one
        if BOX.view().is_slot_two(new_index as usize) {
            return new_index - 1;
        }
    }

    new_index
}

unsafe fn update_box() {
    log::trace!("update_box");
    if BOX.is_open() {
        BOX.update_from_view();
    }
}

unsafe extern "C" fn get_box_if_open() -> *mut Bag {
    log::trace!("get_box_if_open");
    if BOX.is_open() {
        BOX.view()
    } else {
        let character = GAME.get_partner_character();
        if character.is_null() {
            std::ptr::null_mut()
        } else {
            GAME.get_character_bag(character)
        }
    }
}

unsafe extern "fastcall" fn get_partner_bag(unknown: *mut c_void) -> *mut Bag {
    // this function is called a lot, even outside the inventory menu, so logging it just floods
    // the log with useless info
    // log::trace!("get_partner_bag");
    if BOX.is_open() {
        return BOX.view();
    }

    // reimplementation of the original function
    let partner = GAME.get_partner_character();
    if !partner.is_null() {
        match GAME.sub_522a20(partner) {
            1 | 2 | 3 => unknown.offset(32) as *mut Bag,
            5 | 7 => unknown.offset(96) as *mut Bag,
            _ => std::ptr::null_mut(),
        }
    } else {
        std::ptr::null_mut()
    }
}

unsafe fn initialize(is_enabled: bool, is_leave_allowed: bool) -> Result<()> {
    log::info!("Initializing item box mod");

    GAME.init(is_enabled)?;

    let version = GAME.version();
    if is_enabled {
        log::info!("Item box mod is enabled; installing all hooks");
        // when the game tries to display the partner's inventory, show the box instead if it's open
        let bag_jump = jmp(version.get_partner_bag, get_partner_bag as usize);
        patch(version.get_partner_bag, &bag_jump)?;

        let partner_bag_org_call = call(version.get_partner_bag_org, get_box_if_open as usize);
        patch(version.get_partner_bag_org, &partner_bag_org_call)?;
        // nop out call to GetCharacterBag which is now handled by get_box_if_open
        patch(version.get_partner_bag_org + 0x10, &[NOP; 5])?;

        // override the msg file the game looks for so we don't have to replace the originals
        // grab the address of the format string
        let msg_format = std::ptr::read_unaligned((version.msg_load1 + 1) as *const usize);

        let msg_jmp1 = jmp(version.msg_load1, MSG_TRAMPOLINE1.as_ptr() as usize);
        set_trampoline(&mut MSG_TRAMPOLINE1, 0, load_msg_file as usize)?;
        MSG_TRAMPOLINE1[9..13].copy_from_slice(&msg_format.to_le_bytes());
        set_trampoline(&mut MSG_TRAMPOLINE1, 13, version.msg_load1 + 5)?;
        patch(version.msg_load1, &msg_jmp1)?;

        let msg_jmp2 = jmp(version.msg_load2, MSG_TRAMPOLINE2.as_ptr() as usize);
        set_trampoline(&mut MSG_TRAMPOLINE2, 0, load_msg_file as usize)?;
        MSG_TRAMPOLINE2[9..13].copy_from_slice(&msg_format.to_le_bytes());
        set_trampoline(&mut MSG_TRAMPOLINE2, 13, version.msg_load2 + 5)?;
        patch(version.msg_load2, &msg_jmp2)?;

        let msg_jmp3 = jmp(version.msg_load3, MSG_TRAMPOLINE3.as_ptr() as usize);
        set_trampoline(&mut MSG_TRAMPOLINE3, 0, load_msg_file as usize)?;
        MSG_TRAMPOLINE3[9..13].copy_from_slice(&msg_format.to_le_bytes());
        set_trampoline(&mut MSG_TRAMPOLINE3, 13, version.msg_load3 + 5)?;
        patch(version.msg_load3, &msg_jmp3)?;

        // when trying to scroll up past the top inventory row, scroll the box view
        let scroll_up_return = get_conditional_jump_target(version.scroll_up_check as *const c_void);
        let scroll_up_jump = jl(version.scroll_up_check, SCROLL_UP_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(&mut SCROLL_UP_TRAMPOLINE, 2, scroll_up as usize)?;
        set_trampoline(&mut SCROLL_UP_TRAMPOLINE, 11, scroll_up_return as usize)?;
        patch(version.scroll_up_check, &scroll_up_jump)?;

        // when trying to scroll down past the last inventory row, scroll the box view
        let scroll_down_return = get_conditional_jump_target(version.scroll_down_check as *const c_void);
        let scroll_down_jump = jmp(version.scroll_down_check, SCROLL_DOWN_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(&mut SCROLL_DOWN_TRAMPOLINE, 2, scroll_down as usize)?;
        set_conditional_trampoline(&mut SCROLL_DOWN_TRAMPOLINE, 13, scroll_down_return as usize)?;
        set_trampoline(&mut SCROLL_DOWN_TRAMPOLINE, 19, version.scroll_down_check + 6)?;
        patch(version.scroll_down_check, &scroll_down_jump)?;

        // when trying to scroll left from the first inventory cell, scroll the box view
        let scroll_left_jump = jmp(version.scroll_left_check, SCROLL_LEFT_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(&mut SCROLL_LEFT_TRAMPOLINE, 5, scroll_left as usize)?;
        set_trampoline(&mut SCROLL_LEFT_TRAMPOLINE, 15, version.scroll_left_check + 8)?;
        patch(version.scroll_left_check, &scroll_left_jump)?;

        // when trying to scroll right from the last inventory cell, scroll the box view
        let scroll_right_jump = jmp(
            version.scroll_right_check,
            SCROLL_RIGHT_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(&mut SCROLL_RIGHT_TRAMPOLINE, 7, scroll_right as usize)?;
        set_trampoline(&mut SCROLL_RIGHT_TRAMPOLINE, 15, version.scroll_right_check + 6)?;
        patch(version.scroll_right_check, &scroll_right_jump)?;

        let scroll_right_two_jump = jmp(
            version.scroll_right_two_check,
            SCROLL_RIGHT_TWO_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(&mut SCROLL_RIGHT_TWO_TRAMPOLINE, 7, scroll_right as usize)?;
        set_trampoline(&mut SCROLL_RIGHT_TWO_TRAMPOLINE, 21, version.scroll_right_two_check + 10)?;
        patch(version.scroll_right_two_check, &scroll_right_two_jump)?;

        // after the view is organized, copy its contents back into the box
        let organize_jump1 = jmp(version.organize_end1, ORGANIZE_TRAMPOLINE.as_ptr() as usize);
        let organize_jump2 = jmp(version.organize_end2, ORGANIZE_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(&mut ORGANIZE_TRAMPOLINE, 1, update_box as usize)?;
        patch(version.organize_end1, &organize_jump1)?;
        patch(version.organize_end2, &organize_jump2)?;

        if !is_leave_allowed {
            log::info!("Disabling leave option");
            // disable leaving items since that would be OP when combined with the item box
            patch(version.leave_sound_arg, &FAIL_SOUND.to_le_bytes())?;
            patch(version.leave_menu_state, &[0xEB, 0x08])?; // short jump to skip the code that switches to the "leaving item" menu state
        }

        // handle the extra options when activating the typewriter
        let has_ink_jump = jmp(version.has_ink_ribbon, HAS_INK_RIBBON_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(
            &mut HAS_INK_RIBBON_TRAMPOLINE,
            3,
            track_typewriter_message as usize,
        )?;
        set_trampoline(&mut HAS_INK_RIBBON_TRAMPOLINE, 19, version.has_ink_ribbon + 7)?;
        patch(version.has_ink_ribbon, &has_ink_jump)?;

        let no_ink_jump = jmp(version.no_ink_ribbon, NO_INK_RIBBON_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(
            &mut NO_INK_RIBBON_TRAMPOLINE,
            3,
            track_typewriter_message as usize,
        )?;
        set_trampoline(&mut NO_INK_RIBBON_TRAMPOLINE, 19, version.no_ink_ribbon + 7)?;
        patch(version.no_ink_ribbon, &no_ink_jump)?;

        let choice_call = call(
            version.typewriter_choice_check,
            TYPEWRITER_CHOICE_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(
            &mut TYPEWRITER_CHOICE_TRAMPOLINE,
            6,
            check_typewriter_choice as usize,
        )?;
        patch(version.typewriter_choice_check, &choice_call)?;

        let box_call = call(version.typewriter_phase_set, OPEN_BOX_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(&mut OPEN_BOX_TRAMPOLINE, 1, open_box as usize)?;
        set_trampoline(&mut OPEN_BOX_TRAMPOLINE, 15, version.set_room_phase)?;
        patch(version.typewriter_phase_set, &box_call)?;

        // make the menu show the box to start with instead of the partner control panel
        let view_call = call(
            version.inventory_open_animation,
            OPEN_ANIMATION_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(
            &mut OPEN_ANIMATION_TRAMPOLINE,
            1,
            show_partner_inventory as usize,
        )?;
        set_trampoline(&mut OPEN_ANIMATION_TRAMPOLINE, 19, version.play_menu_animation)?;
        patch(version.inventory_open_animation, &view_call)?;

        // always enable exchanging when a character first opens the box
        let init_jump = jmp(
            version.inventory_menu_start,
            INVENTORY_START_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(&mut INVENTORY_START_TRAMPOLINE, 2, menu_setup as usize)?;
        set_trampoline(&mut INVENTORY_START_TRAMPOLINE, 17, version.inventory_menu_start + 6)?;
        patch(version.inventory_menu_start, &init_jump)?;

        // handle enabling and disabling exchanging when the character changes
        let character_jump = jmp(
            version.inventory_change_character,
            CHANGE_CHARACTER_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(
            &mut CHANGE_CHARACTER_TRAMPOLINE,
            2,
            change_character as usize,
        )?;
        set_trampoline(&mut CHANGE_CHARACTER_TRAMPOLINE, 18, version.inventory_change_character + 7)?;
        patch(version.inventory_change_character, &character_jump)?;

        // close the box after closing the inventory
        let close_call = jmp(
            version.inventory_menu_close,
            INVENTORY_CLOSE_TRAMPOLINE.as_ptr() as usize,
        );
        set_trampoline(&mut INVENTORY_CLOSE_TRAMPOLINE, 1, close_box as usize)?;
        set_trampoline(&mut INVENTORY_CLOSE_TRAMPOLINE, 12, version.inventory_menu_close + 5)?;
        patch(version.inventory_menu_close, &close_call)?;

        // make room in the box if the player tries to swap a two-slot item into a full view
        let double_jump = jmp(version.exchange_size_check, SIZE_CHECK_TRAMPOLINE.as_ptr() as usize);
        SIZE_CHECK_TRAMPOLINE[7..11].copy_from_slice(&(version.exchange_size_check + 5).to_le_bytes());
        set_trampoline(
            &mut SIZE_CHECK_TRAMPOLINE,
            11,
            make_room_for_double as usize,
        )?;
        patch(version.exchange_size_check, &double_jump)?;

        // skip the check preventing giving both shaft keys to the same character when the box
        // is open. aside from being undesirable, it also crashes the game when using the box
        // without having a partner character.
        let original_function = get_call_target(version.shaft_check as *const c_void);
        let shaft_jump = jmp(version.shaft_check, SHAFT_CHECK_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(
            &mut SHAFT_CHECK_TRAMPOLINE,
            1,
            should_skip_shaft_check as usize,
        )?;
        SHAFT_CHECK_TRAMPOLINE[10..14].copy_from_slice(&(version.shaft_check + 5).to_le_bytes());
        SHAFT_CHECK_TRAMPOLINE[15..19].copy_from_slice(&(original_function as usize).to_le_bytes());
        SHAFT_CHECK_TRAMPOLINE[25..29].copy_from_slice(&(version.shaft_check + 0xCA).to_le_bytes());
        patch(version.shaft_check, &shaft_jump)?;

        // reset the box when starting a new game
        let original_function = get_call_target(version.new_game as *const c_void);
        let new_game_call = call(version.new_game, NEW_GAME_TRAMPOLINE.as_ptr() as usize);
        set_trampoline(&mut NEW_GAME_TRAMPOLINE, 1, new_game as usize)?;
        set_trampoline(&mut NEW_GAME_TRAMPOLINE, 7, original_function as usize)?;
        patch(version.new_game, &new_game_call)?;
    } else {
        log::info!("Item box mod is disabled; installing only save/load hooks");
    }

    // even if the mod is disabled, we still install our load and save handlers to prevent
    // the game from blowing away saved boxes, and also so we can clear the box on any save
    // slots that are saved to while the mod is inactive

    // load data
    let load_jump = jmp(version.post_load, LOAD_TRAMPOLINE.as_ptr() as usize);
    set_trampoline(&mut LOAD_TRAMPOLINE, 2, load_data as usize)?;
    set_trampoline(&mut LOAD_TRAMPOLINE, 20, version.sub_6fc610)?;
    LOAD_TRAMPOLINE[16..20].copy_from_slice(&(version.post_load + 11).to_le_bytes());
    patch(version.post_load, &load_jump)?;

    // load slot
    let ls_jump = jmp(version.load_slot, LOAD_SLOT_TRAMPOLINE.as_ptr() as usize);
    set_trampoline(&mut LOAD_SLOT_TRAMPOLINE, 2, load_slot as usize)?;
    set_trampoline(&mut LOAD_SLOT_TRAMPOLINE, 17, version.load_slot + 6)?;
    patch(version.load_slot, &ls_jump)?;

    // save data
    let save_jump = jmp(version.steam_save, SAVE_TRAMPOLINE.as_ptr() as usize);
    SAVE_TRAMPOLINE[2..6].copy_from_slice(&(version.steam_save + 7).to_le_bytes());
    set_trampoline(&mut SAVE_TRAMPOLINE, 6, save_data as usize)?;
    patch(version.steam_save, &save_jump)?;

    // save slot
    let ss_jump = jmp(version.save_slot, SAVE_SLOT_TRAMPOLINE.as_ptr() as usize);
    set_trampoline(&mut SAVE_SLOT_TRAMPOLINE, 2, save_slot as usize)?;
    set_trampoline(&mut SAVE_SLOT_TRAMPOLINE, 17, version.save_slot + 6)?;
    patch(version.save_slot, &ss_jump)?;

    log::info!("Patching complete");

    Ok(())
}

fn main(reason: u32) -> Result<()> {
    if reason == DLL_PROCESS_ATTACH {
        let game_dir = unsafe { Game::get_game_dir() };

        let config_path = game_dir.join("re0box.ini");
        let mut config = Ini::new();
        // we don't care if the config fails to load, we'll just use the defaults
        let _ = config.load(config_path);
        let is_enabled = config
            .getboolcoerce("Enable", "Mod")
            .ok()
            .flatten()
            .unwrap_or(true);
        let is_leave_allowed = config
            .getboolcoerce("Enable", "Leave")
            .ok()
            .flatten()
            .unwrap_or(false);
        let log_level = config
            .get("Log", "Level")
            .map(|s| {
                let s = s.to_lowercase();
                LevelFilter::iter().find(|l| l.as_str().to_lowercase() == s)
            })
            .flatten()
            .unwrap_or(LevelFilter::Info);
        let mut log_file_path = config
            .get("Log", "Path")
            .map_or_else(|| PathBuf::from("re0box.log"), PathBuf::from);

        if !log_file_path.is_absolute() {
            log_file_path = game_dir.join(log_file_path);
        }

        // ignore the result because there's nothing we can do if opening the log file fails (except
        // crash, which we don't want to do)
        let _ = open_log(log_level, log_file_path);
        if let Err(e) = unsafe { initialize(is_enabled, is_leave_allowed) } {
            log::error!("Initialization failed: {:?}", e);
            return Err(e);
        }
    } else if reason == DLL_PROCESS_DETACH {
        close_log();
    }

    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(_dll_module: HMODULE, reason: u32, _reserved: *const c_void) -> BOOL {
    main(reason).is_ok().into()
}
