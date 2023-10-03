use std::ffi::c_void;
use std::fs::File;
use std::{cmp, mem};
use std::ops::BitAnd;
use std::panic;
use std::path::PathBuf;

use anyhow::Result;
use simplelog::{Config, LevelFilter, WriteLogger};
use windows::core::PWSTR;
use windows::Win32::Foundation::{HMODULE, MAX_PATH};
use windows::Win32::System::Diagnostics::Debug::{CONTEXT_CONTROL_X86, CONTEXT_DEBUG_REGISTERS_X86, CONTEXT_FLOATING_POINT_X86, CONTEXT_INTEGER_X86, CONTEXT_SEGMENTS_X86, EXCEPTION_POINTERS, AddVectoredExceptionHandler, RemoveVectoredExceptionHandler};
use windows::Win32::System::Memory::{MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_READONLY, PAGE_READWRITE, VirtualQuery};
use windows::Win32::System::ProcessStatus::{EnumProcessModules, GetModuleBaseNameW, GetModuleInformation, MODULEINFO};
use windows::Win32::System::Kernel::ExceptionContinueSearch;
use windows::Win32::System::Threading::GetCurrentProcess;

const STACK_DUMP_WORDS_PER_LINE: usize = 4;
const STACK_DUMP_LINES: usize = 6;
const READABLE_PROTECT: [PAGE_PROTECTION_FLAGS; 4] = [
    PAGE_EXECUTE_READ,
    PAGE_EXECUTE_READWRITE,
    PAGE_READWRITE,
    PAGE_READONLY,
];
const MAX_MODULES: usize = 1000;

unsafe extern "system" fn exception_handler(exc_info: *mut EXCEPTION_POINTERS) -> i32 {
    if let Some(exc_info) = exc_info.as_ref() {
        // exception details
        let mut record_ptr = exc_info.ExceptionRecord;
        while let Some(record) = record_ptr.as_ref() {
            log::error!("Unhandled exception {:08X} at {:08X}. Parameters: {:?}", record.ExceptionCode.0, record.ExceptionAddress as usize, &record.ExceptionInformation[..record.NumberParameters as usize]);
            record_ptr = record.ExceptionRecord;
        }

        // registers
        let mut sp = None;
        if let Some(context) = exc_info.ContextRecord.as_ref() {
            if context.ContextFlags.bitand(CONTEXT_INTEGER_X86) == CONTEXT_INTEGER_X86 {
                log::error!("\tedi = {:08X}\tesi = {:08X}", context.Edi, context.Esi);
                log::error!("\tebx = {:08X}\tedx = {:08X}", context.Ebx, context.Edx);
                log::error!("\tecx = {:08X}\teax = {:08X}", context.Ecx, context.Eax);
            }

            if context.ContextFlags.bitand(CONTEXT_CONTROL_X86) == CONTEXT_CONTROL_X86 {
                log::error!("\tebp = {:08X}\teip = {:08X}", context.Ebp, context.Eip);
                log::error!("\tesp = {:08X}\teflags = {:08X}", context.Esp, context.EFlags);
                log::error!("\tcs = {:04X}\tss = {:04X}", context.SegCs, context.SegSs);
                sp = Some(context.Esp as usize);
            }

            if context.ContextFlags.bitand(CONTEXT_SEGMENTS_X86) == CONTEXT_SEGMENTS_X86 {
                log::error!("\tgs = {:04X}\tfs = {:04X}", context.SegGs, context.SegFs);
                log::error!("\tes = {:04X}\tds = {:04X}", context.SegEs, context.SegDs);
            }

            if context.ContextFlags.bitand(CONTEXT_FLOATING_POINT_X86) == CONTEXT_FLOATING_POINT_X86 {
                log::error!("\tfloat: {:?}", context.FloatSave);
            }

            if context.ContextFlags.bitand(CONTEXT_DEBUG_REGISTERS_X86) == CONTEXT_DEBUG_REGISTERS_X86 {
                log::error!("\tdr0 = {:08X}\tdr1 = {:08X}", context.Dr0, context.Dr1);
                log::error!("\tdr2 = {:08X}\tdr3 = {:08X}", context.Dr2, context.Dr3);
                log::error!("\tdr6 = {:08X}\tdr7 = {:08X}", context.Dr6, context.Dr7);
            }
        }

        // stack dump if it's valid
        if let Some(mut ptr) = sp {
            let mut info = MEMORY_BASIC_INFORMATION::default();
            let info_size = mem::size_of::<MEMORY_BASIC_INFORMATION>();
            let mut region_end = ptr;
            log::error!("Stack dump:");
            for _ in 0..STACK_DUMP_LINES {
                let mut words = [0usize; STACK_DUMP_WORDS_PER_LINE];
                let mut exit = false;
                let line_addr = ptr;
                for word in &mut words {
                    let mut word_buf = [0u8; mem::size_of::<usize>()];
                    let bytes_to_copy = cmp::min(region_end - ptr, word_buf.len());
                    if bytes_to_copy > 0 {
                        (ptr as *const u8).copy_to_nonoverlapping(word_buf.as_mut_ptr(), bytes_to_copy);
                    }
                    ptr += bytes_to_copy;
                    if bytes_to_copy < word_buf.len() {
                        // we reached the end of the region; need to query the next region
                        let bytes_written = VirtualQuery(Some(ptr as *const c_void), &mut info, info_size);
                        if bytes_written < info_size {
                            log::error!("{:08X}: VirtualQuery for stack info failed", ptr as usize);
                            exit = true;
                            break;
                        } else if info.State != MEM_COMMIT || !READABLE_PROTECT.iter().any(|p| info.Protect.bitand(*p) == *p) {
                            log::error!("{:08X}: memory is not readable", ptr as usize);
                            exit = true;
                            break;
                        }

                        region_end = info.AllocationBase as usize + info.RegionSize;
                        let remaining_bytes = word_buf.len() - bytes_to_copy;
                        (ptr as *const u8).copy_to_nonoverlapping(word_buf[bytes_to_copy..].as_mut_ptr(), remaining_bytes);
                        ptr += remaining_bytes;
                    }

                    *word = usize::from_le_bytes(word_buf);
                }

                if exit {
                    break;
                }

                let mut line = format!("\t{:08X}: ", line_addr);
                for word in words {
                    line = format!("{} {:08X}", line, word);
                }
                log::error!("{}", line);
            }
        } else {
            log::error!("Stack dump: stack pointer was not present");
        }

        // module list
        let mut modules = [HMODULE::default(); MAX_MODULES];
        let mut size_needed = 0;
        if !EnumProcessModules(GetCurrentProcess(), modules.as_mut_ptr(), mem::size_of::<[HMODULE; MAX_MODULES]>() as u32, &mut size_needed).is_ok() {
            log::error!("Modules: could not enumerate modules");
        } else {
            log::error!("Modules:");
            let num_modules = size_needed as usize / mem::size_of::<HMODULE>();
            for module in modules.into_iter().take(num_modules) {
                let mut name_buf = [0u16; MAX_PATH as usize];
                let chars_copied = GetModuleBaseNameW(GetCurrentProcess(), module, &mut name_buf);
                let module_name = if chars_copied == 0 || chars_copied >= name_buf.len() as u32 {
                    String::from("<unknown>")
                } else {
                    PWSTR::from_raw(name_buf.as_mut_ptr()).to_string().unwrap_or_else(|_| String::from("<invalid>"))
                };

                let mut mod_info = MODULEINFO::default();
                let address_range = match GetModuleInformation(GetCurrentProcess(), module, &mut mod_info, mem::size_of::<MODULEINFO>() as u32) {
                    Ok(_) => format!("{:08X}-{:08X}", mod_info.lpBaseOfDll as usize, mod_info.lpBaseOfDll as usize + mod_info.SizeOfImage as usize),
                    Err(e) => format!("error: {:?}", e),
                };

                log::error!("\t{}\t{}", module_name, address_range);
            }
        }
    }

    ExceptionContinueSearch.0
}

pub fn open_log(log_level: LevelFilter, log_path: PathBuf) -> Result<()> {
    let log_file = File::create(log_path)?;
    WriteLogger::init(log_level, Config::default(), log_file)?;
    panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<&str>().unwrap_or(&"unknown");
        let (file, line) = info.location().map(|l| (l.file(), l.line())).unwrap_or(("unknown", 0));
        log::error!("Panic in {} on line {}: {}", file, line, msg);
    }));
    unsafe {
        AddVectoredExceptionHandler(0, Some(exception_handler));
    }
    Ok(())
}

pub fn close_log() {
    unsafe {
        RemoveVectoredExceptionHandler(exception_handler as *const c_void);
    }
}