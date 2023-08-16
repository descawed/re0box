use std::ffi::c_void;

use windows::core::{IUnknown, GUID, HRESULT, PCSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

const LIBRARY_PATH: PCSTR = PCSTR::from_raw(b"C:\\Windows\\System32\\dinput8\0".as_ptr());
const DIRECT_INPUT8_CREATE: PCSTR = PCSTR::from_raw(b"DirectInput8Create\0".as_ptr());
const DLL_CAN_UNLOAD_NOW: PCSTR = PCSTR::from_raw(b"DllCanUnloadNow\0".as_ptr());
const DLL_GET_CLASS_OBJECT: PCSTR = PCSTR::from_raw(b"DllGetClassObject\0".as_ptr());
const DLL_REGISTER_SERVER: PCSTR = PCSTR::from_raw(b"DllRegisterServer\0".as_ptr());
const DLL_UNREGISTER_SERVER: PCSTR = PCSTR::from_raw(b"DllUnregisterServer\0".as_ptr());
const GET_DF_DI_JOYSTICK: PCSTR = PCSTR::from_raw(b"GetdfDIJoystick\0".as_ptr());

#[derive(Debug)]
pub struct DInput8 {
    is_initialized: bool,
    library: HMODULE,
    direct_input8_create: Option<
        unsafe extern "system" fn(
            HMODULE,
            u32,
            *const GUID,
            *mut *const c_void,
            *const IUnknown,
        ) -> HRESULT,
    >,
    dll_can_unload_now: Option<unsafe extern "system" fn() -> HRESULT>,
    dll_get_class_object:
        Option<unsafe extern "system" fn(*const GUID, *const GUID, *mut *const c_void) -> HRESULT>,
    dll_register_server: Option<unsafe extern "system" fn() -> HRESULT>,
    dll_unregister_server: Option<unsafe extern "system" fn() -> HRESULT>,
    get_df_di_joystick: Option<unsafe extern "system" fn() -> *const c_void>,
}

impl DInput8 {
    pub const fn new() -> Self {
        Self {
            is_initialized: false,
            library: HMODULE(0),
            direct_input8_create: None,
            dll_can_unload_now: None,
            dll_get_class_object: None,
            dll_register_server: None,
            dll_unregister_server: None,
            get_df_di_joystick: None,
        }
    }

    pub unsafe fn init(&mut self) {
        if !self.is_initialized {
            self.is_initialized = true;
            // load the real dinput8 library
            self.library = match LoadLibraryA(LIBRARY_PATH) {
                Ok(m) => m,
                Err(e) => panic!("Could not load dinput8.dll: {:?}", e),
            };
            // I'm pretty sure this would've returned an Err result, but we'll check just to be safe
            if self.library.is_invalid() {
                panic!("Could not load dinput8.dll: invalid HMODULE");
            }

            self.direct_input8_create =
                GetProcAddress(self.library, DIRECT_INPUT8_CREATE).map(|f| std::mem::transmute(f));
            self.dll_can_unload_now =
                GetProcAddress(self.library, DLL_CAN_UNLOAD_NOW).map(|f| std::mem::transmute(f));
            self.dll_get_class_object =
                GetProcAddress(self.library, DLL_GET_CLASS_OBJECT).map(|f| std::mem::transmute(f));
            self.dll_register_server =
                GetProcAddress(self.library, DLL_REGISTER_SERVER).map(|f| std::mem::transmute(f));
            self.dll_unregister_server =
                GetProcAddress(self.library, DLL_UNREGISTER_SERVER).map(|f| std::mem::transmute(f));
            self.get_df_di_joystick =
                GetProcAddress(self.library, GET_DF_DI_JOYSTICK).map(|f| std::mem::transmute(f));
        }
    }

    pub unsafe fn direct_input8_create(
        &mut self,
        hinst: HMODULE,
        version: u32,
        riidltf: *const GUID,
        ppv_out: *mut *const c_void,
        punk_outer: *const IUnknown,
    ) -> HRESULT {
        self.init();
        self.direct_input8_create.unwrap()(hinst, version, riidltf, ppv_out, punk_outer)
    }

    pub unsafe fn dll_can_unload_now(&mut self) -> HRESULT {
        self.init();
        self.dll_can_unload_now.unwrap()()
    }

    pub unsafe fn dll_get_class_object(
        &mut self,
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *const c_void,
    ) -> HRESULT {
        self.init();
        self.dll_get_class_object.unwrap()(rclsid, riid, ppv)
    }

    pub unsafe fn dll_register_server(&mut self) -> HRESULT {
        self.init();
        self.dll_register_server.unwrap()()
    }

    pub unsafe fn dll_unregister_server(&mut self) -> HRESULT {
        self.init();
        self.dll_unregister_server.unwrap()()
    }

    pub unsafe fn get_df_di_joystick(&mut self) -> *const c_void {
        self.init();
        self.get_df_di_joystick.unwrap()()
    }
}
