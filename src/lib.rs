#![cfg(windows)]

use std::ffi::c_void;
use std::path::Path;
use std::str;

use anyhow::Result;
use configparser::ini::Ini;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

mod patch;
use patch::*;

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
static mut SCROLL_UP_TRAMPOLINE: [u8; 20] = [
    0x60, // pushad
    0x6A, 0xFE, // push -2
    0x57, // push edi
    0xE8, 0, 0, 0, 0, // call <fn>
    0x83, 0xC4, 0x08, // add esp,8
    0x61, // popad
    0xBE, 0x9E, 0x4D, 0x5E, 0, // mov esi, 0x5e4d9e
    0xFF, 0xE6, // jmp esi
];

static mut SCROLL_DOWN_TRAMPOLINE: [u8; 20] = [
    0x60, // pushad
    0x6A, 0x02, // push 2
    0x57, // push edi
    0xE8, 0, 0, 0, 0, // call <fn>
    0x83, 0xC4, 0x08, // add esp,8
    0x61, // popad
    0xBE, 0x9E, 0x4D, 0x5E, 0, // mov esi, 0x5e4d9e
    0xFF, 0xE6, // jmp esi
];

static mut SCROLL_LEFT_TRAMPOLINE: [u8; 22] = [
    0x79, 0x0D, // jns done
    0x51, // push ecx
    0x52, // push edx
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x5A, // pop edx
    0x59, // pop ecx
    0xBA, 0xF9, 0x39, 0x5E, 0x00, // done: mov edx,0x5e39f9
    0xFF, 0xE2, // jmp edx
];

static mut SCROLL_RIGHT_TRAMPOLINE: [u8; 25] = [
    0x83, 0xF8, 0x06, // cmp eax,6
    0x7C, 0x0D, // jl done
    0x51, // push ecx
    0x52, // push edx
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x5A, // pop edx
    0x59, // pop ecx
    0xBA, 0xF9, 0x39, 0x5E, 0x00, // done: mov edx,0x5e39f9
    0xFF, 0xE2, // jmp edx
];

static mut PARTNER_BAG_ORG_TRAMPOLINE: [u8; 26] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0xB9, 0x41, 0xC6, 0x4D, 0x00, // mov ecx,0x4dc641
    0x85, 0xC0, // test eax,eax
    0x74, 0x0A, // jz do_jmp
    0x8D, 0x4C, 0x24, 0x08, // lea ecx,[esp+8]
    0x51, // push ecx
    0xB9, 0x3A, 0xC6, 0x4D, 0x00, // mov ecx,0x4dc63a
    0xFF, 0xE1, // do_jmp: jmp ecx
];

static mut ORGANIZE_TRAMPOLINE: [u8; 13] = [
    0x60, // pushad
    0xE8, 0, 0, 0, 0,    // call <fn>
    0x61, // popad
    0x83, 0xC4, 0x3C, // add esp, 0x3c
    0xC2, 0x04, 0x00, // retn 4
];

static mut HAS_INK_RIBBON_TRAMPOLINE: [u8; 26] = [
    0x60, // pushad
    0x6A, 0x01, // push 1
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xC7, 0x47, 0x10, 0x00, 0x00, 0x00, 0x40, // mov dword ptr [edi+0x10], 0x40000000
    0xBF, 0x20, 0xAD, 0x57, 0x00, // mov edi,0x57ad20
    0xFF, 0xE7, // jmp edi
];

static mut NO_INK_RIBBON_TRAMPOLINE: [u8; 26] = [
    0x60, // pushad
    0x6A, 0x00, // push 0
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xC7, 0x47, 0x10, 0x00, 0x00, 0x00, 0x40, // mov dword ptr [edi+0x10], 0x40000000
    0xBF, 0x5B, 0xAD, 0x57, 0x00, // mov edi,0x57ad5b
    0xFF, 0xE7, // jmp edi
];

static mut TYPEWRITER_CHOICE_TRAMPOLINE: [u8; 24] = [
    0x8B, 0x74, 0x24, 0x60, // mov esi,[esp+0x60]
    0x60, // pushad
    0x56, // push esi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0xBE, 0xAC, 0xAD, 0x57, 0x00, // mov esi,0x57adac
    0xFF, 0xE6, // jmp esi
];

static mut OPEN_BOX_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0x74, 0x03, // jz phase
    0xFF, 0x0C, 0x24, // dec dword ptr [esp]
    0x68, 0xEB, 0xAD, 0x57, 0x00, // phase: push 0x57adeb
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp SetRoomPhase
];

static mut INVENTORY_CLOSE_TRAMPOLINE: [u8; 19] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x61, // popad
    0x8B, 0x46, 0x60, // mov eax,[esi+0x60]
    0x6A, 0x00, // push 0
    0xB9, 0x88, 0x89, 0x5D, 0x00, // mov ecx,0x5d8988
    0xFF, 0xE1, // jmp ecx
];

static mut INVENTORY_START_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0xFF, 0x87, 0x94, 0x02, 0x00, 0x00, // inc dword ptr [edi+0x294]
    0xBE, 0x8C, 0x1B, 0x5E, 0x00, // mov esi,0x5e1b8c
    0xFF, 0xE6, // jmp esi
];

static mut CHANGE_CHARACTER_TRAMPOLINE: [u8; 25] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0x80, 0xBF, 0xCA, 0x02, 0x00, 0x00, 0x01, // cmp byte ptr [edi+0x2ca],1
    0xB9, 0xD1, 0x2B, 0x5E, 0x00, // mov ecx,0x5e2bd1
    0xFF, 0xE1, // jmp ecx
];

static mut OPEN_ANIMATION_TRAMPOLINE: [u8; 28] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0x74, 0x07, // jz do_call
    0xC7, 0x04, 0x24, 0x01, 0x00, 0x00, 0x00, // mov dword ptr [esp],1
    0x68, 0x54, 0x1B, 0x5E, 0x00, // do_call: push 0x5e1b54
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp PlayMenuAnimation
];

static mut SIZE_CHECK_TRAMPOLINE: [u8; 16] = [
    0xFF, 0x74, 0x24, 0x20, // push [esp+0x20]
    0x51, // push ecx
    0x57, // push edi
    0x68, 0x99, 0x3E, 0x5E, 0x00, // push 0x5e3e99
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp <fn>
];

static mut LOAD_SLOT_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0x56, // push esi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0x69, 0xF6, 0x50, 0xC8, 0x01, 0x00, // imul esi,0x1c850
    0xB8, 0xF7, 0x25, 0x61, 0x00, // mov eax,0x6125f7
    0xFF, 0xE0, // jmp eax
];

static mut LOAD_TRAMPOLINE: [u8; 25] = [
    0x55, // push ebp
    0x53, // push ebx
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    // we intentionally don't clean the stack because we're passing the same arguments to the next function
    0x89, 0x44, 0x24, 0x04, // mov [esp+4],eax
    0x8D, 0x4C, 0x24, 0x34, // lea ecx,[esp+0x34]
    0x68, 0x80, 0x59, 0x8B, 0x00, // push 0x8b980
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp sub_6FC610
];

static mut SAVE_SLOT_TRAMPOLINE: [u8; 24] = [
    0x60, // pushad
    0x57, // push edi
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x83, 0xC4, 0x04, // add esp,4
    0x61, // popad
    0x69, 0xFF, 0x50, 0xC8, 0x01, 0x00, // imul edi,0x1c850
    0xB8, 0xEF, 0x34, 0x61, 0x00, // mov eax,0x6134ef
    0xFF, 0xE0, // jmp eax
];

static mut SAVE_TRAMPOLINE: [u8; 11] = [
    0x50, // push eax
    0x68, 0xC8, 0x5C, 0x8B, 0x00, // push 0x8b5cc8
    0xE9, 0x00, 0x00, 0x00, 0x00, // jmp <fn>
];

static mut MSG_TRAMPOLINE1: [u8; 20] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x89, 0x04, 0x24, // mov [esp],eax
    0x68, 0x00, 0x5E, 0xCB, 0x00, // push 0xcb5e00
    0xB8, 0x53, 0x86, 0x40, 0x00, // mov eax,0x408653
    0xFF, 0xE0, // jmp eax
];

static mut MSG_TRAMPOLINE2: [u8; 20] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x89, 0x04, 0x24, // mov [esp],eax
    0x68, 0x00, 0x5E, 0xCB, 0x00, // push 0xcb5e00
    0xB8, 0x76, 0x64, 0x5D, 0x00, // mov eax,0x5d6476
    0xFF, 0xE0, // jmp eax
];

static mut MSG_TRAMPOLINE3: [u8; 20] = [
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x89, 0x04, 0x24, // mov [esp],eax
    0x68, 0x00, 0x5E, 0xCB, 0x00, // push 0xcb5e00
    0xB8, 0xE6, 0x67, 0x5D, 0x00, // mov eax,0x5d67e6
    0xFF, 0xE0, // jmp eax
];

static mut SHAFT_CHECK_TRAMPOLINE: [u8; 31] = [
    0x60, // pushad
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x85, 0xC0, // test eax,eax
    0x61, // popad
    0x68, 0x78, 0x3D, 0x5E, 0x00, // push 0x5e3d78
    0xB8, 0xC0, 0x63, 0x52, 0x00, // mov eax,0x5263c0
    0x74, 0x08, // jz do_jmp
    0x83, 0xC4, 0x04, // add esp,4
    0xB8, 0x3D, 0x3E, 0x5E, 0x00, // mov eax,0x5e3e3d
    0xFF, 0xE0, // do_jmp: jmp eax
];

static mut NEW_GAME_TRAMPOLINE: [u8; 14] = [
    0x51, // push ecx
    0xE8, 0x00, 0x00, 0x00, 0x00, // call <fn>
    0x59, // pop ecx
    0xB8, 0x40, 0x13, 0x41, 0x00, // mov eax,0x411340
    0xFF, 0xE0, // jmp eax
];

static mut BOX: ItemBox = ItemBox::new();
static mut GAME: Game = Game::new();

unsafe extern "C" fn new_game() {
    // reset the box when starting a new game
    BOX.set_contents(vec![]);
}

unsafe extern "fastcall" fn should_skip_shaft_check(partner: *const c_void) -> bool {
    // partner should never be null at this point of the code unless the box is open, but we'll
    // check both anyway just to be safe
    BOX.is_open() || partner.is_null()
}

unsafe extern "C" fn load_msg_file(lang: *const u8) -> *const u8 {
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
    GAME.save_to_slot(BOX.get_contents(), index);
}

unsafe extern "stdcall" fn save_data(filename: *const u8, buf: *const u8, size: usize) -> bool {
    GAME.save(std::slice::from_raw_parts(buf, size), filename)
        .is_ok()
}

unsafe extern "C" fn load_slot(index: usize) {
    BOX.set_contents(GAME.load_from_slot(index));
}

unsafe extern "C" fn load_data(buf: *const u8, size: usize) -> usize {
    GAME.load(std::slice::from_raw_parts(buf, size)).unwrap();
    UNMODDED_SAVE_SIZE
}

// we use stdcall here because we're returning directly to the game, so we need to clean the stack
unsafe extern "stdcall" fn make_room_for_double(
    menu: *const c_void,
    unknown: *const c_void,
    item_size: usize,
) -> i32 {
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
    if BOX.is_open() {
        // flag that the partner inventory is displayed
        *(menu.offset(0x2ca) as *mut bool) = true;
    }

    BOX.is_open()
}

unsafe fn close_box() {
    BOX.close();
}

unsafe extern "C" fn change_character(menu: *mut c_void) {
    if BOX.is_open() {
        GAME.update_exchange_state(menu);
    }
}

unsafe extern "C" fn menu_setup(menu: *mut c_void) {
    if BOX.is_open() {
        GAME.init_menu(menu);
    }
}

unsafe fn open_box() -> bool {
    if GAME.should_open_box {
        GAME.prepare_inventory();
        BOX.open();
    }

    GAME.should_open_box
}

unsafe extern "C" fn check_typewriter_choice(choice: i32) -> bool {
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
    GAME.user_had_ink_ribbon = had_ink_ribbon;
}

unsafe extern "C" fn scroll_left(unknown: *const c_void) -> i32 {
    if BOX.scroll_view(-2) {
        GAME.draw_bags(unknown);
        if BOX.view().is_slot_two(1) {
            0
        } else {
            1
        }
    } else {
        5 // we're already at the top, so wrap around to the last cell in the view
    }
}

unsafe extern "C" fn scroll_right(unknown: *const c_void) -> i32 {
    if BOX.scroll_view(2) {
        GAME.draw_bags(unknown);
        4
    } else {
        0 // we're already at the bottom, so wrap around to the first cell in the view
    }
}

unsafe extern "C" fn scroll(unknown: *const c_void, offset: isize) {
    if BOX.scroll_view(offset) {
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

unsafe fn update_box() {
    if BOX.is_open() {
        BOX.update_from_view();
    }
}

unsafe extern "C" fn get_box_if_open() -> *mut Bag {
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

fn main(reason: u32) -> Result<()> {
    if reason == DLL_PROCESS_ATTACH {
        let config_path = unsafe { Game::get_game_dir() }.join("re0box.ini");
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

        unsafe {
            GAME.init(is_enabled);

            if is_enabled {
                // when the game tries to display the partner's inventory, show the box instead if it's open
                let bag_jump = jmp(GET_PARTNER_BAG, get_partner_bag as usize);
                patch(GET_PARTNER_BAG, &bag_jump)?;
                let org_jump = jmp(
                    GET_PARTNER_BAG_ORG,
                    PARTNER_BAG_ORG_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(&mut PARTNER_BAG_ORG_TRAMPOLINE, 0, get_box_if_open as usize)?;
                patch(GET_PARTNER_BAG_ORG, &org_jump)?;

                // override the msg file the game looks for so we don't have to replace the originals
                let msg_jump1 = jmp(MSG_LOAD1, MSG_TRAMPOLINE1.as_ptr() as usize);
                set_trampoline(&mut MSG_TRAMPOLINE1, 0, load_msg_file as usize)?;
                patch(MSG_LOAD1, &msg_jump1)?;

                let msg_jump2 = jmp(MSG_LOAD2, MSG_TRAMPOLINE2.as_ptr() as usize);
                set_trampoline(&mut MSG_TRAMPOLINE2, 0, load_msg_file as usize)?;
                patch(MSG_LOAD2, &msg_jump2)?;

                let msg_jump3 = jmp(MSG_LOAD3, MSG_TRAMPOLINE3.as_ptr() as usize);
                set_trampoline(&mut MSG_TRAMPOLINE3, 0, load_msg_file as usize)?;
                patch(MSG_LOAD3, &msg_jump3)?;

                // when trying to scroll up past the top inventory row, scroll the box view
                let scroll_up_jump = jl(SCROLL_UP_CHECK, SCROLL_UP_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut SCROLL_UP_TRAMPOLINE, 4, scroll as usize)?;
                patch(SCROLL_UP_CHECK, &scroll_up_jump)?;

                // when trying to scroll down past the last inventory row, scroll the box view
                let scroll_down_jump =
                    jge(SCROLL_DOWN_CHECK, SCROLL_DOWN_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut SCROLL_DOWN_TRAMPOLINE, 4, scroll as usize)?;
                patch(SCROLL_DOWN_CHECK, &scroll_down_jump)?;

                // when trying to scroll left from the first inventory cell, scroll the box view
                let scroll_left_jump =
                    jmp(SCROLL_LEFT_CHECK, SCROLL_LEFT_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut SCROLL_LEFT_TRAMPOLINE, 5, scroll_left as usize)?;
                patch(SCROLL_LEFT_CHECK, &scroll_left_jump)?;

                // when trying to scroll right from the last inventory cell, scroll the box view
                let scroll_right_jump = jmp(
                    SCROLL_RIGHT_CHECK,
                    SCROLL_RIGHT_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(&mut SCROLL_RIGHT_TRAMPOLINE, 8, scroll_right as usize)?;
                patch(SCROLL_RIGHT_CHECK, &scroll_right_jump)?;

                // after the view is organized, copy its contents back into the box
                let organize_jump1 = jmp(ORGANIZE_END1, ORGANIZE_TRAMPOLINE.as_ptr() as usize);
                let organize_jump2 = jmp(ORGANIZE_END2, ORGANIZE_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut ORGANIZE_TRAMPOLINE, 1, update_box as usize)?;
                patch(ORGANIZE_END1, &organize_jump1)?;
                patch(ORGANIZE_END2, &organize_jump2)?;

                if !is_leave_allowed {
                    // disable leaving items since that would be OP when combined with the item box
                    patch(LEAVE_SOUND_ARG, &FAIL_SOUND.to_le_bytes())?;
                    patch(LEAVE_MENU_STATE, &[0xEB, 0x08])?; // short jump to skip the code that switches to the "leaving item" menu state
                }

                // handle the extra options when activating the typewriter
                let has_ink_jump = jmp(HAS_INK_RIBBON, HAS_INK_RIBBON_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(
                    &mut HAS_INK_RIBBON_TRAMPOLINE,
                    3,
                    track_typewriter_message as usize,
                )?;
                patch(HAS_INK_RIBBON, &has_ink_jump)?;

                let no_ink_jump = jmp(NO_INK_RIBBON, NO_INK_RIBBON_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(
                    &mut NO_INK_RIBBON_TRAMPOLINE,
                    3,
                    track_typewriter_message as usize,
                )?;
                patch(NO_INK_RIBBON, &no_ink_jump)?;

                let choice_jump = jmp(
                    TYPEWRITER_CHOICE_CHECK,
                    TYPEWRITER_CHOICE_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(
                    &mut TYPEWRITER_CHOICE_TRAMPOLINE,
                    6,
                    check_typewriter_choice as usize,
                )?;
                patch(TYPEWRITER_CHOICE_CHECK, &choice_jump)?;

                let box_jump = jmp(TYPEWRITER_PHASE_SET, OPEN_BOX_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut OPEN_BOX_TRAMPOLINE, 1, open_box as usize)?;
                set_trampoline(&mut OPEN_BOX_TRAMPOLINE, 19, SET_ROOM_PHASE)?;
                patch(TYPEWRITER_PHASE_SET, &box_jump)?;

                // make the menu show the box to start with instead of the partner control panel
                let view_jump = jmp(
                    INVENTORY_OPEN_ANIMATION,
                    OPEN_ANIMATION_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(
                    &mut OPEN_ANIMATION_TRAMPOLINE,
                    1,
                    show_partner_inventory as usize,
                )?;
                set_trampoline(&mut OPEN_ANIMATION_TRAMPOLINE, 23, PLAY_MENU_ANIMATION)?;
                patch(INVENTORY_OPEN_ANIMATION, &view_jump)?;

                // always enable exchanging when a character first opens the box
                let init_jump = jmp(
                    INVENTORY_MENU_START,
                    INVENTORY_START_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(&mut INVENTORY_START_TRAMPOLINE, 2, menu_setup as usize)?;
                patch(INVENTORY_MENU_START, &init_jump)?;

                // handle enabling and disabling exchanging when the character changes
                let character_jump = jmp(
                    INVENTORY_CHANGE_CHARACTER,
                    CHANGE_CHARACTER_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(
                    &mut CHANGE_CHARACTER_TRAMPOLINE,
                    2,
                    change_character as usize,
                )?;
                patch(INVENTORY_CHANGE_CHARACTER, &character_jump)?;

                // close the box after closing the inventory
                let close_jump = jmp(
                    INVENTORY_MENU_CLOSE,
                    INVENTORY_CLOSE_TRAMPOLINE.as_ptr() as usize,
                );
                set_trampoline(&mut INVENTORY_CLOSE_TRAMPOLINE, 1, close_box as usize)?;
                patch(INVENTORY_MENU_CLOSE, &close_jump)?;

                // make room in the box if the player tries to swap a two-slot item into a full view
                let double_jump = jmp(EXCHANGE_SIZE_CHECK, SIZE_CHECK_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(
                    &mut SIZE_CHECK_TRAMPOLINE,
                    11,
                    make_room_for_double as usize,
                )?;
                patch(EXCHANGE_SIZE_CHECK, &double_jump)?;

                // skip the check preventing giving both shaft keys to the same character when the box
                // is open. aside from being undesirable, it also crashes the game when using the box
                // without having a partner character.
                let shaft_jump = jmp(SHAFT_CHECK, SHAFT_CHECK_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(
                    &mut SHAFT_CHECK_TRAMPOLINE,
                    1,
                    should_skip_shaft_check as usize,
                )?;
                patch(SHAFT_CHECK, &shaft_jump)?;

                // reset the box when starting a new game
                let new_game_call = call(NEW_GAME, NEW_GAME_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut NEW_GAME_TRAMPOLINE, 1, new_game as usize)?;
                patch(NEW_GAME, &new_game_call)?;
            }

            // even if the mod is disabled, we still install our load and save handlers to prevent
            // the game from blowing away saved boxes, and also so we can clear the box on any save
            // slots that are saved to while the mod is inactive

            // load data
            let load_jump = jmp(POST_LOAD, LOAD_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut LOAD_TRAMPOLINE, 2, load_data as usize)?;
            set_trampoline(&mut LOAD_TRAMPOLINE, 20, SUB_6FC610)?;
            patch(POST_LOAD, &load_jump)?;

            // load slot
            let ls_jump = jmp(LOAD_SLOT, LOAD_SLOT_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut LOAD_SLOT_TRAMPOLINE, 2, load_slot as usize)?;
            patch(LOAD_SLOT, &ls_jump)?;

            // save data
            let save_jump = jmp(STEAM_SAVE, SAVE_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut SAVE_TRAMPOLINE, 6, save_data as usize)?;
            patch(STEAM_SAVE, &save_jump)?;

            // save slot
            let ss_jump = jmp(SAVE_SLOT, SAVE_SLOT_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut SAVE_SLOT_TRAMPOLINE, 2, save_slot as usize)?;
            patch(SAVE_SLOT, &ss_jump)?;
        }
    }

    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(_dll_module: HMODULE, reason: u32, _reserved: *const c_void) -> BOOL {
    main(reason).is_ok().into()
}
