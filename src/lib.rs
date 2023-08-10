#![cfg(windows)]

use std::arch::asm;
use std::ffi::c_void;
use std::fs::File;
use std::io::prelude::*;

use anyhow::Result;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::System::Memory::{PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, VirtualProtect};
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

mod inventory;
use inventory::*;

// I tried the naked-function crate, but it failed to compile for me, complaining about "unknown
// directive" .pushsection. maybe it has something to do with the fact that I'm cross-compiling.
static mut SCROLL_UP_TRAMPOLINE: [u8; 14] = [
    0x60, // pushad
    0xE8, 0, 0, 0, 0, // call <fn>
    0x61, // popad
    0xBE, 0x9E, 0x4D, 0x5E, 0, // mov esi, 0x5e4d9e
    0xFF, 0xE6, // jmp esi
];

static mut SCROLL_DOWN_TRAMPOLINE: [u8; 14] = [
    0x60, // pushad
    0xE8, 0, 0, 0, 0, // call <fn>
    0x61, // popad
    0xBE, 0x9E, 0x4D, 0x5E, 0, // mov esi, 0x5e4d9e
    0xFF, 0xE6, // jmp esi
];

static mut BOX: ItemBox = ItemBox::new();

const GET_CHARACTER_BAG: usize = 0x0050DA80;
const GET_PARTNER_BAG: usize = 0x004DC8B0;
const GET_PARTNER_BAG_ORG: usize = 0x004DC635;
const ORGANIZE_BAG: usize = 0x004DA880;
const SCROLL_UP_CHECK: usize = 0x005E386A;
const SCROLL_DOWN_CHECK: usize = 0x005E3935;
const SUB_66DEC0: usize = 0x0066DEC0;
const SUB_522A20: usize = 0x00522A20;
const PTR_DCDF3C: usize = 0x00DCDF3C;

const fn addr_offset(from: usize, to: usize, inst_size: usize) -> [u8; std::mem::size_of::<usize>()] {
    to.overflowing_sub(from + inst_size).0.to_le_bytes()
}

fn call(from: usize, to: usize) -> [u8; 5] {
    let bytes = addr_offset(from, to, 5);
    [0xE8, bytes[0], bytes[1], bytes[2], bytes[3]]
}

fn jmp(from: usize, to: usize) -> [u8; 5] {
    let bytes = addr_offset(from, to, 5);
    [0xE9, bytes[0], bytes[1], bytes[2], bytes[3]]
}

fn jl(from: usize, to: usize) -> [u8; 6] {
    let bytes = addr_offset(from, to, 6);
    [0x0F, 0x8C, bytes[0], bytes[1], bytes[2], bytes[3]]
}

fn jge(from: usize, to: usize) -> [u8; 6] {
    let bytes = addr_offset(from, to, 6);
    [0x0F, 0x8D, bytes[0], bytes[1], bytes[2], bytes[3]]
}

unsafe fn set_trampoline(trampoline: &mut [u8], call_offset: usize, to: usize) -> Result<()> {
    let ptr = trampoline.as_ptr();
    let asm = call(ptr.offset(call_offset as isize) as usize, to);
    trampoline[call_offset..call_offset+asm.len()].copy_from_slice(&asm);

    let mut old_protect = PAGE_PROTECTION_FLAGS::default();
    VirtualProtect(ptr as *const c_void, trampoline.len(), PAGE_EXECUTE_READWRITE,
                   &mut old_protect).ok()?;

    Ok(())
}

unsafe fn patch(addr: usize, bytes: &[u8]) -> Result<()> {
    let mut old_protect = PAGE_PROTECTION_FLAGS::default();
    VirtualProtect(addr as *const c_void, bytes.len(), PAGE_EXECUTE_READWRITE,
                   &mut old_protect).ok()?;

    let addr = addr as *mut u8;
    addr.copy_from(bytes.as_ptr(), bytes.len());

    Ok(())
}

unsafe fn scroll_up() {
    BOX.scroll_view(-2);
    organize_box();
}

unsafe fn scroll_down() {
    BOX.scroll_view(2);
    organize_box();
}

unsafe fn organize_box() {
    if BOX.is_open() {
        // stable Rust doesn't support the thiscall calling convention, so we have to use assembly
        let mut buffer = [0i32; 6];
        let bag = BOX.view();
        asm!(
            "push {buf}",
            "call {addr}",
            in("ecx") bag,
            buf = in(reg) buffer.as_mut_ptr(),
            addr = in(reg) ORGANIZE_BAG,
        );
    }
}

unsafe extern "fastcall" fn get_box_if_open(character: *const c_void) -> *mut Bag {
    if BOX.is_open() {
        BOX.view()
    } else {
        let get_character_bag: unsafe extern "fastcall" fn(*const c_void) -> *mut Bag = std::mem::transmute(GET_CHARACTER_BAG);
        get_character_bag(character)
    }
}

unsafe extern "fastcall" fn get_partner_bag(unknown: *mut c_void) -> *mut Bag {
    if BOX.is_open() {
        return BOX.view();
    }

    // reimplementation of the original function
    let v2 = PTR_DCDF3C as *const *const c_void;
    let sub_66dec0: unsafe extern "fastcall" fn(*const c_void) -> *const c_void = std::mem::transmute(SUB_66DEC0);
    let sub_522a20: unsafe extern "fastcall" fn(*const c_void) -> i32 = std::mem::transmute(SUB_522A20);

    let v2 = *v2;
    if v2.is_null() {
        panic!("Pointer not initialized");
    }

    let v3 = sub_66dec0(v2);
    if !v3.is_null() {
        let v4 = sub_522a20(v3);
        match v4 {
            1 | 2 | 3 => unknown.offset(32) as *mut Bag,
            5 | 7 => unknown.offset(96) as *mut Bag,
            _ => std::ptr::null_mut(),
        }
    } else {
        std::ptr::null_mut()
    }
}

fn main(reason: u32) -> Result<()> {
    match reason {
        DLL_PROCESS_ATTACH => {
            let mut file = File::create("test.txt")?;
            file.write_all(b"DLL attached\n")?;

            // when the game tries to display the partner's inventory, show the box instead if it's open
            let bag_jump = jmp(GET_PARTNER_BAG, get_partner_bag as usize);
            let bag_call = call(GET_PARTNER_BAG_ORG, get_box_if_open as usize);
            unsafe {
                patch(GET_PARTNER_BAG, &bag_jump)?;
                patch(GET_PARTNER_BAG_ORG, &bag_call)?;

                // when trying to scroll up past the top inventory row, scroll the box view
                let scroll_up_jump = jl(SCROLL_UP_CHECK, SCROLL_UP_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut SCROLL_UP_TRAMPOLINE, 1, scroll_up as usize)?;
                patch(SCROLL_UP_CHECK, &scroll_up_jump)?;

                // when trying to scroll down past the last inventory row, scroll the box view
                let scroll_down_jump = jge(SCROLL_DOWN_CHECK, SCROLL_DOWN_TRAMPOLINE.as_ptr() as usize);
                set_trampoline(&mut SCROLL_DOWN_TRAMPOLINE, 1, scroll_down as usize)?;
                patch(SCROLL_DOWN_CHECK, &scroll_down_jump)?;

                BOX.open();
            }
        }
        DLL_PROCESS_DETACH => {
            let mut file = File::options().append(true).open("test.txt")?;
            file.write_all(b"DLL detached\n")?;

        }
        _ => ()
    }

    Ok(())
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(_dll_module: HMODULE, reason: u32, _reserved: *const c_void) -> BOOL {
    main(reason).is_ok().into()
}