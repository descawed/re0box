// I don't want to get warnings in here about unused instruction functions that I've used in the
// past and could want again in the future
#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::c_void;

use anyhow::{anyhow, Result};
use memchr::memmem;
use windows::core::PWSTR;
use windows::Win32::Foundation::{HMODULE, MAX_PATH};
use windows::Win32::System::Memory::{VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READWRITE, PAGE_WRITECOPY};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModules, GetModuleBaseNameW, GetModuleInformation, MODULEINFO,
};
use windows::Win32::System::Threading::GetCurrentProcess;

pub const NOP: u8 = 0x90;

pub const unsafe fn get_absolute_target(ptr: *const c_void, instruction_size: isize) -> *const c_void {
    let original_jump_offset = std::ptr::read_unaligned(ptr.offset(instruction_size - 4) as *const isize) + instruction_size;
    ptr.offset(original_jump_offset)
}

// works with both call and non-short jump
pub const unsafe fn get_call_target(ptr: *const c_void) -> *const c_void {
    get_absolute_target(ptr, 5)
}

pub const unsafe fn get_conditional_jump_target(ptr: *const c_void) -> *const c_void {
    get_absolute_target(ptr, 6)
}

pub const fn addr_offset(
    from: usize,
    to: usize,
    inst_size: usize,
) -> [u8; size_of::<usize>()] {
    to.overflowing_sub(from + inst_size).0.to_le_bytes()
}

pub fn call(from: usize, to: usize) -> [u8; 5] {
    let bytes = addr_offset(from, to, 5);
    [0xE8, bytes[0], bytes[1], bytes[2], bytes[3]]
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

pub unsafe fn set_conditional_trampoline(trampoline: &mut [u8], call_offset: usize, to: usize) -> Result<()> {
    let ptr = trampoline.as_ptr();
    let asm = addr_offset(ptr.offset(call_offset as isize) as usize, to, 6);
    trampoline[call_offset + 2..call_offset + 2 + asm.len()].copy_from_slice(&asm);
    unprotect(ptr as *const c_void, trampoline.len())
}

pub unsafe fn patch(addr: usize, bytes: &[u8]) -> Result<()> {
    unprotect(addr as *const c_void, bytes.len())?;

    let addr = addr as *mut u8;
    addr.copy_from(bytes.as_ptr(), bytes.len());

    Ok(())
}

pub unsafe fn assert_byte<T>(addr: *const T, expected: u8) -> Result<()> {
    let actual = *(addr as *const u8);
    if actual != expected {
        let uaddr = addr as usize;
        return Err(anyhow!(
            "Expected {expected:#02X} at {uaddr:#08X} but found {actual:#02X}"
        ));
    }

    Ok(())
}

#[derive(Debug)]
pub struct ByteSearcher {
    modules: HashMap<String, (*const c_void, *const c_void)>,
}

impl ByteSearcher {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    fn search_in_ranges<'a, T: Default + Copy, const N: usize>(
        protection: Option<PAGE_PROTECTION_FLAGS>,
        ranges: impl Iterator<Item = &'a (*const c_void, *const c_void)>,
        search_func: impl Fn(*const u8, usize, &mut [T]) -> bool,
    ) -> Result<[T; N]> {
        let mut results = [Default::default(); N];
        for &(start, end) in ranges {
            let mut addr = start;
            while addr < end {
                let mut memory_info = MEMORY_BASIC_INFORMATION::default();
                log::debug!("Querying address {:#08X}", addr as usize);
                let result = unsafe {
                    VirtualQuery(Some(addr), &mut memory_info, size_of_val(&memory_info))
                };
                if result == 0 {
                    break;
                }

                let search_base = addr as *const u8;
                addr = unsafe { memory_info.BaseAddress.add(memory_info.RegionSize) };

                if memory_info.State != MEM_COMMIT
                    || protection.is_some_and(|p| !p.contains(memory_info.Protect))
                {
                    log::trace!("Skipping address {:#08X} due to state {:#08X} or protection (actual {:#08X}, expected {:#08X})", search_base as usize, memory_info.State.0, memory_info.Protect.0, protection.unwrap_or(PAGE_NOACCESS).0);
                    continue;
                }

                log::debug!("Searching address {:#08X}", search_base as usize);
                if search_func(search_base, memory_info.RegionSize, &mut results) {
                    // if search_func returns true, we've found everything we were looking for
                    break;
                }
            }
        }

        Ok(results)
    }

    pub fn find_bytes_in_ranges<'a, const N: usize>(
        patterns: &[&[u8]; N],
        protection: Option<PAGE_PROTECTION_FLAGS>,
        ranges: impl Iterator<Item = &'a (*const c_void, *const c_void)>,
    ) -> Result<[Option<*const c_void>; N]> {
        Self::search_in_ranges(protection, ranges, |search_base, region_size, addresses: &mut [Option<*const c_void>]| {
            let search_region =
                unsafe { std::slice::from_raw_parts(search_base, region_size) };
            for (&pattern, address) in patterns
                .iter()
                .zip(addresses.iter_mut())
                .filter(|(_, a)| a.is_none())
            {
                if let Some(offset) = memmem::find(search_region, pattern) {
                    let found_address = unsafe { search_base.add(offset) } as *const c_void;
                    log::debug!("Found address {:#08X}", found_address as usize);
                    *address = Some(found_address);
                }
            }

            addresses.iter().all(Option::is_some)
        })
    }

    pub fn find_addresses_in_ranges<'a, const N: usize>(
        addresses: &[usize; N],
        protection: Option<PAGE_PROTECTION_FLAGS>,
        ranges: impl Iterator<Item = &'a (*const c_void, *const c_void)>,
    ) -> Result<[bool; N]> {
        Self::search_in_ranges(protection, ranges, |search_base, region_size, flags: &mut [bool]| {
            for (&address, flag) in addresses
                .iter()
                .zip(flags.iter_mut())
                .filter(|(_, f)| !**f)
            {
                let start = search_base as usize;
                let end = start + region_size;
                if address >= start && address < end {
                    log::debug!("Found address {:#08X}", address);
                    *flag = true;
                }
            }

            flags.iter().all(|&f| f)
        })
    }

    pub fn discover_modules(&mut self) -> Result<()> {
        let mut modules = [HMODULE::default(); 1024];
        let mut bytes_needed = 0;
        let hproc = unsafe { GetCurrentProcess() };
        log::debug!("Enumerating process modules");
        unsafe {
            EnumProcessModules(
                hproc,
                modules.as_mut_ptr(),
                size_of_val(&modules) as u32,
                &mut bytes_needed,
            )
        }?;

        let num_modules =
            std::cmp::min(bytes_needed as usize / size_of::<HMODULE>(), modules.len());
        log::debug!("Found {} modules", num_modules);
        for &module in &modules[..num_modules] {
            let mut name_utf16 = [0; MAX_PATH as usize];
            let module_name = unsafe {
                let num_chars = GetModuleBaseNameW(hproc, Some(module), &mut name_utf16) as usize;
                if num_chars == 0 || num_chars >= name_utf16.len() {
                    continue;
                }

                match PWSTR::from_raw(name_utf16.as_mut_ptr()).to_string() {
                    Ok(name) => name,
                    Err(_) => continue,
                }
            }
                .to_lowercase();

            log::debug!("Module name: {}", module_name);
            let mut module_info = MODULEINFO::default();
            unsafe {
                GetModuleInformation(
                    hproc,
                    module,
                    &mut module_info,
                    size_of_val(&module_info) as u32,
                )?;
                let base = module_info.lpBaseOfDll as *const c_void;
                self.modules.insert(
                    module_name,
                    (base, base.add(module_info.SizeOfImage as usize)),
                );
            }
        }

        Ok(())
    }

    fn get_module_ranges<'b, 'a: 'b, 'c: 'b>(
        &'a self,
        modules: &'b [&'c str],
    ) -> impl Iterator<Item = &'a (*const c_void, *const c_void)> + 'b {
        modules
            .iter()
            .filter_map(|&module_name| self.modules.get(&module_name.to_lowercase()))
    }

    pub fn find_bytes<const N: usize, const M: usize>(
        &self,
        patterns: &[&[u8]; N],
        protection: Option<PAGE_PROTECTION_FLAGS>,
        modules: &[&str; M],
    ) -> Result<[Option<*const c_void>; N]> {
        if M > 0 {
            Self::find_bytes_in_ranges(patterns, protection, self.get_module_ranges(modules))
        } else {
            // we'll use the standard page size as the minimum address
            Self::find_bytes_in_ranges(
                patterns,
                protection,
                [&(0x1000 as *const c_void, usize::MAX as *const c_void)].into_iter(),
            )
        }
    }

    pub fn find_addresses<const N: usize, const M: usize>(
        &self,
        addresses: &[usize; N],
        protection: Option<PAGE_PROTECTION_FLAGS>,
        modules: &[&str; M],
    ) -> Result<[bool; N]> {
        if M > 0 {
            Self::find_addresses_in_ranges(addresses, protection, self.get_module_ranges(modules))
        } else {
            // we'll use the standard page size as the minimum address
            Self::find_addresses_in_ranges(
                addresses,
                protection,
                [&(0x1000 as *const c_void, usize::MAX as *const c_void)].into_iter(),
            )
        }
    }

    pub fn find_addresses_write<const N: usize, const M: usize>(
        &self,
        addresses: &[usize; N],
        modules: &[&str; M],
    ) -> Result<[bool; N]> {
        self.find_addresses(addresses, Some(PAGE_READWRITE | PAGE_WRITECOPY), modules)
    }

    pub fn find_addresses_exec<const N: usize, const M: usize>(
        &self,
        addresses: &[usize; N],
        modules: &[&str; M],
    ) -> Result<[bool; N]> {
        self.find_addresses(addresses, Some(PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE), modules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_addr_offset() {
        assert_eq!(addr_offset(0x80000000, 0x80000010, 3), [13, 0, 0, 0]);
        assert_eq!(
            addr_offset(0x80000000, 0x7FFFFF10, 4),
            [12, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn call_bytes() {
        assert_eq!(call(0x80000000, 0x80000010), [0xE8, 11, 0, 0, 0]);
    }

    #[test]
    fn jmp_bytes() {
        assert_eq!(jmp(0x80000000, 0x80000010), [0xE9, 11, 0, 0, 0]);
    }

    #[test]
    fn jl_bytes() {
        assert_eq!(jl(0x80000000, 0x800000F0), [0x0F, 0x8C, 0xEA, 0, 0, 0]);
    }

    #[test]
    fn jge_bytes() {
        assert_eq!(jge(0x80000000, 0x800000E0), [0x0F, 0x8D, 0xDA, 0, 0, 0]);
    }
}
