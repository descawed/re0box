use std::ffi::c_void;
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};

use anyhow::Result;

pub const fn addr_offset(
    from: usize,
    to: usize,
    inst_size: usize,
) -> [u8; std::mem::size_of::<usize>()] {
    to.overflowing_sub(from + inst_size).0.to_le_bytes()
}

pub fn jmp(from: usize, to: usize) -> [u8; 5] {
    let bytes = addr_offset(from, to, 5);
    [0xE9, bytes[0], bytes[1], bytes[2], bytes[3]]
}

pub fn jl(from: usize, to: usize) -> [u8; 6] {
    let bytes = addr_offset(from, to, 6);
    [0x0F, 0x8C, bytes[0], bytes[1], bytes[2], bytes[3]]
}

pub fn jge(from: usize, to: usize) -> [u8; 6] {
    let bytes = addr_offset(from, to, 6);
    [0x0F, 0x8D, bytes[0], bytes[1], bytes[2], bytes[3]]
}

pub unsafe fn unprotect(ptr: *const c_void, size: usize) -> Result<()> {
    let mut old_protect = PAGE_PROTECTION_FLAGS::default();
    VirtualProtect(ptr, size, PAGE_EXECUTE_READWRITE, &mut old_protect)?;

    Ok(())
}

// supports either call or jmp
pub unsafe fn set_trampoline(trampoline: &mut [u8], call_offset: usize, to: usize) -> Result<()> {
    let ptr = trampoline.as_ptr();
    let asm = addr_offset(ptr.offset(call_offset as isize) as usize, to, 5);
    trampoline[call_offset + 1..call_offset + 1 + asm.len()].copy_from_slice(&asm);
    unprotect(ptr as *const c_void, trampoline.len())
}

pub unsafe fn patch(addr: usize, bytes: &[u8]) -> Result<()> {
    unprotect(addr as *const c_void, bytes.len())?;

    let addr = addr as *mut u8;
    addr.copy_from(bytes.as_ptr(), bytes.len());

    Ok(())
}
