use windows::Win32::{
    Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, TRUE, WPARAM},
    System::SystemServices::{
        DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetClassNameW, SetWindowsHookExW, CWPRETSTRUCT, CWPSTRUCT, HHOOK, MSG,
        WH_CALLWNDPROC, WH_CALLWNDPROCRET, WH_GETMESSAGE,
    },
};

static mut WIN_PROC_HOOK: Option<HHOOK> = None;
static mut WIN_PROC_RET_HOOK: Option<HHOOK> = None;
static mut GET_MESSAGE_HOOK: Option<HHOOK> = None;
static mut DLL_HANDLE: Option<HINSTANCE> = None;

fn get_class(hwnd: HWND) -> String {
    let mut text: [u16; 512] = [0; 512];
    let len = unsafe { GetClassNameW(hwnd, &mut text) };
    let length = usize::try_from(len).unwrap_or(0);
    String::from_utf16_lossy(&text[..length])
}

/// # Safety
#[no_mangle]
pub unsafe extern "system" fn win_proc_hook(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let data = (l_param.0 as *const CWPSTRUCT).as_ref();
    if let Some(data) = data {
        println!(
            "win_proc_hook Window: {:08X} Class: {}",
            data.hwnd.0 as usize,
            get_class(data.hwnd),
        );
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

/// # Safety
#[no_mangle]
pub unsafe extern "system" fn win_proc_ret_hook(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let data = (l_param.0 as *const CWPRETSTRUCT).as_ref();
    if let Some(data) = data {
        println!(
            "win_proc_ret_hook Window: {:08X} Class: {}",
            data.hwnd.0 as usize,
            get_class(data.hwnd),
        );
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

/// # Safety
#[no_mangle]
pub unsafe extern "system" fn get_message_hook(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let msg = (l_param.0 as *const MSG).as_ref().unwrap();
    println!(
        "get_message_hook Window: {:08X} Class: {}",
        msg.hwnd.0 as usize,
        get_class(msg.hwnd)
    );
    CallNextHookEx(None, n_code, w_param, l_param)
}

/// # Safety
#[no_mangle]
pub unsafe extern "system" fn install_hook(thread_id: u32) -> bool {
    println!("Installing hook");

    WIN_PROC_HOOK =
        match SetWindowsHookExW(WH_CALLWNDPROC, Some(win_proc_hook), DLL_HANDLE, thread_id) {
            Ok(hook) => Some(hook),
            Err(err) => {
                println!("Failed to install hook: {}", err);
                return false;
            }
        };

    WIN_PROC_RET_HOOK = match SetWindowsHookExW(
        WH_CALLWNDPROCRET,
        Some(win_proc_ret_hook),
        DLL_HANDLE,
        thread_id,
    ) {
        Ok(hook) => Some(hook),
        Err(err) => {
            println!("Failed to install hook: {}", err);
            return false;
        }
    };

    GET_MESSAGE_HOOK =
        match SetWindowsHookExW(WH_GETMESSAGE, Some(get_message_hook), DLL_HANDLE, thread_id) {
            Ok(hook) => Some(hook),
            Err(err) => {
                println!("Failed to install hook: {}", err);
                return false;
            }
        };

    true
}

/// # Safety
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    dll_handle: HINSTANCE,
    reason: u32,
    _reserved: isize,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            println!("DllMain: DLL_PROCESS_ATTACH");
            DLL_HANDLE = Some(dll_handle);
        }
        DLL_PROCESS_DETACH => {
            println!("DllMain: DLL_PROCESS_DETACH");
        }
        DLL_THREAD_ATTACH => {
            // println!("DllMain: DLL_THREAD_ATTACH");
        }
        DLL_THREAD_DETACH => {
            // println!("DllMain: DLL_THREAD_DETACH");
        }
        _ => {
            println!("DllMain: Unknown reason");
        }
    }
    TRUE
}
