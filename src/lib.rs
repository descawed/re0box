#![cfg(windows)]

use std::ffi::c_void;
use std::fs::File;
use std::io::prelude::*;

use anyhow::Result;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

mod patch;
use patch::*;

mod game;
use game::*;

mod inventory;
use inventory::*;

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

static mut BOX: ItemBox = ItemBox::new();
static mut GAME: Game = Game::new();

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

unsafe extern "C" fn scroll(unknown: *const c_void, offset: isize) {
    BOX.scroll_view(offset);
    // by default the inventory display doesn't update at this point, so we have to do it ourselves
    GAME.draw_bags(unknown);
}

unsafe fn update_box() {
    if BOX.is_open() {
        BOX.update_from_view();
    }
}

unsafe extern "fastcall" fn get_box_if_open(character: *const c_void) -> *mut Bag {
    if BOX.is_open() {
        BOX.view()
    } else {
        GAME.get_character_bag(character)
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
        let mut file = File::create("test.txt")?;
        file.write_all(b"DLL attached\n")?;

        // when the game tries to display the partner's inventory, show the box instead if it's open
        let bag_jump = jmp(GET_PARTNER_BAG, get_partner_bag as usize);
        let bag_call = call(GET_PARTNER_BAG_ORG, get_box_if_open as usize);
        unsafe {
            GAME.init();

            patch(GET_PARTNER_BAG, &bag_jump)?;
            patch(GET_PARTNER_BAG_ORG, &bag_call)?;

            // when trying to scroll up past the top inventory row, scroll the box view
            let scroll_up_jump = jl(SCROLL_UP_CHECK, SCROLL_UP_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut SCROLL_UP_TRAMPOLINE, 4, scroll as usize)?;
            patch(SCROLL_UP_CHECK, &scroll_up_jump)?;

            // when trying to scroll down past the last inventory row, scroll the box view
            let scroll_down_jump = jge(SCROLL_DOWN_CHECK, SCROLL_DOWN_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut SCROLL_DOWN_TRAMPOLINE, 4, scroll as usize)?;
            patch(SCROLL_DOWN_CHECK, &scroll_down_jump)?;

            // after the view is organized, copy its contents back into the box
            let organize_jump1 = jmp(ORGANIZE_END1, ORGANIZE_TRAMPOLINE.as_ptr() as usize);
            let organize_jump2 = jmp(ORGANIZE_END2, ORGANIZE_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut ORGANIZE_TRAMPOLINE, 1, update_box as usize)?;
            patch(ORGANIZE_END1, &organize_jump1)?;
            patch(ORGANIZE_END2, &organize_jump2)?;

            // disable leaving items since that would be OP when combined with the item box
            patch(LEAVE_SOUND_ARG, &FAIL_SOUND.to_le_bytes())?;
            patch(LEAVE_MENU_STATE, &[0xEB, 0x08])?; // short jump to skip the code that switches to the "leaving item" menu state

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
            let view_jump = jmp(INVENTORY_OPEN_ANIMATION, OPEN_ANIMATION_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut OPEN_ANIMATION_TRAMPOLINE, 1, show_partner_inventory as usize)?;
            set_trampoline(&mut OPEN_ANIMATION_TRAMPOLINE, 23, PLAY_MENU_ANIMATION)?;
            patch(INVENTORY_OPEN_ANIMATION, &view_jump)?;

            // always enable exchanging when a character first opens the box
            let init_jump = jmp(INVENTORY_MENU_START, INVENTORY_START_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut INVENTORY_START_TRAMPOLINE, 2, menu_setup as usize)?;
            patch(INVENTORY_MENU_START, &init_jump)?;

            // handle enabling and disabling exchanging when the character changes
            let character_jump = jmp(INVENTORY_CHANGE_CHARACTER, CHANGE_CHARACTER_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut CHANGE_CHARACTER_TRAMPOLINE, 2, change_character as usize)?;
            patch(INVENTORY_CHANGE_CHARACTER, &character_jump)?;

            // close the box after closing the inventory
            let close_jump = jmp(INVENTORY_MENU_CLOSE, INVENTORY_CLOSE_TRAMPOLINE.as_ptr() as usize);
            set_trampoline(&mut INVENTORY_CLOSE_TRAMPOLINE, 1, close_box as usize)?;
            patch(INVENTORY_MENU_CLOSE, &close_jump)?;

        }
    }

    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(_dll_module: HMODULE, reason: u32, _reserved: *const c_void) -> BOOL {
    main(reason).is_ok().into()
}
