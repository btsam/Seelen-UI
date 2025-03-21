use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    Arc,
};
use windows::Win32::{
    Devices::Display::GUID_DEVINTERFACE_MONITOR,
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowExW, GetMessageW,
        PostQuitMessage, RegisterClassW, RegisterDeviceNotificationW, TranslateMessage,
        DBT_DEVTYP_DEVICEINTERFACE, DEVICE_NOTIFY_WINDOW_HANDLE, DEV_BROADCAST_DEVICEINTERFACE_W,
        MSG, WINDOW_STYLE, WM_DESTROY, WNDCLASSW, WS_EX_TOPMOST,
    },
};

use crate::{
    error_handler::{Result, WindowsResultExt},
    log_error, trace_lock,
    utils::spawn_named_thread,
};

use super::{string_utils::WindowsString, WindowsApi};

type Callback = Box<dyn Fn(u32, usize, isize) -> Result<()> + Send + Sync + 'static>;

lazy_static! {
    static ref CALLBACKS: Arc<Mutex<Vec<Callback>>> = Arc::new(Mutex::new(Vec::new()));
}

pub static BACKGROUND_HWND: AtomicIsize = AtomicIsize::new(0);

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return LRESULT(0);
    }

    for callback in CALLBACKS.lock().iter() {
        log_error!(callback(msg, w_param.0, l_param.0));
    }

    DefWindowProcW(hwnd, msg, w_param, l_param)
}

/// will lock until the window is closed
unsafe fn _create_background_window(done: &crossbeam_channel::Sender<()>) -> Result<()> {
    let title = WindowsString::from("Seelen UI Background Window");
    let class = WindowsString::from("SeelenUIShell");

    let h_module = WindowsApi::module_handle_w()?;

    let wnd_class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: h_module.into(),
        lpszClassName: class.as_pcwstr(),
        ..Default::default()
    };

    RegisterClassW(&wnd_class);

    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST,
        class.as_pcwstr(),
        title.as_pcwstr(),
        WINDOW_STYLE::default(),
        0,
        0,
        0,
        0,
        None,
        None,
        Some(wnd_class.hInstance),
        None,
    )?;

    let handle: isize = hwnd.0 as isize;
    BACKGROUND_HWND.store(handle, Ordering::Relaxed);

    // register window to recieve device notifications for monitor changes
    {
        let mut notification_filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
            dbcc_reserved: 0,
            dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
            dbcc_name: [0; 1],
        };
        RegisterDeviceNotificationW(
            hwnd.into(),
            &mut notification_filter as *mut _ as *mut _,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )?;
    }

    done.send(())?;
    let mut msg = MSG::default();
    // GetMessageW will run until PostQuitMessage(0) is called
    while GetMessageW(&mut msg, Some(hwnd), 0, 0).into() {
        TranslateMessage(&msg).ok().filter_fake_error()?;
        DispatchMessageW(&msg);
    }
    Ok(())
}

pub unsafe fn test_dll_hook() -> Result<()> {
    let dll_path = WindowsString::from("hook.dll");
    let dll = LoadLibraryW(dll_path.as_pcwstr())?;

    let install_hook: unsafe extern "system" fn(u32) -> bool =
        std::mem::transmute(GetProcAddress(dll, windows_core::s!("install_hook")));

    let native_shell = get_native_shell_hwnd()?;
    let (process_id, thread_id) = WindowsApi::window_thread_process_id(native_shell);
    log::debug!(
        "Native shell hwnd: {:08X}, thread id: {:08X}, process id: {:08X}",
        native_shell.0 as usize,
        thread_id,
        process_id
    );

    install_hook(thread_id);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).into() {
        TranslateMessage(&msg).ok().filter_fake_error()?;
        DispatchMessageW(&msg);
    }

    Ok(())
}

/// the objective with this window is having a thread that will receive window events
/// and propagate them across the application (common events are keyboard, power, display, etc)
pub fn create_background_window() -> Result<()> {
    spawn_named_thread("DLL", || {
        log_error!(unsafe { test_dll_hook() });
    })?;

    let (tx, rx) = crossbeam_channel::bounded(1);
    spawn_named_thread("Background Window", move || {
        log::trace!("Creating background window...");
        log_error!(unsafe { _create_background_window(&tx) });
    })?;
    rx.recv()?;
    log::trace!("Background window created");
    Ok(())
}

pub fn subscribe_to_background_window<F>(callback: F)
where
    F: Fn(u32, usize, isize) -> Result<()> + Send + Sync + 'static,
{
    trace_lock!(CALLBACKS).push(Box::new(callback));
}

pub fn get_native_shell_hwnd() -> Result<HWND> {
    let hwnd = unsafe {
        let class = WindowsString::from("Shell_TrayWnd");
        FindWindowExW(None, None, class.as_pcwstr(), None)?
    };
    Ok(hwnd)
}
