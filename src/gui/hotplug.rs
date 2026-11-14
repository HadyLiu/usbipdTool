//! 监听 Windows USB 插拔（`WM_DEVICECHANGE`），去抖后通知 UI 刷新设备列表。
//!
//! 使用隐藏的 message-only 窗口，并对 `GUID_DEVINTERFACE_USB_DEVICE` 调用
//! `RegisterDeviceNotification`，以便收到真正的 USB 到达/移除，而不仅是卷设备事件。

/// 插拔会在短时间内连发多条消息，等安静这么久再触发一次扫描。
#[cfg(windows)]
const DEBOUNCE_MS: u32 = 600;
#[cfg(windows)]
const TIMER_ID: usize = 1;
#[cfg(windows)]
const CLASS_NAME: &[u16] = &[
    b'U' as u16, b's' as u16, b'b' as u16, b'i' as u16, b'p' as u16, b'd' as u16,
    b'T' as u16, b'o' as u16, b'o' as u16, b'l' as u16, b'.' as u16, b'H' as u16,
    b'o' as u16, b't' as u16, b'p' as u16, b'l' as u16, b'u' as u16, b'g' as u16, 0,
];

/// 后台监听句柄：丢弃时关闭隐藏窗口并结束消息循环。
pub struct UsbWatcher {
    /// HWND 以 usize 保存，以便跨线程传递（原始指针不是 `Send`）。
    #[cfg(windows)]
    hwnd: usize,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl UsbWatcher {
    /// 启动监听。失败（无法创建窗口或注册通知）时返回 `None`，不影响手动扫描。
    pub fn start(on_change: impl Fn() + Send + 'static) -> Option<Self> {
        start_impl(Box::new(on_change))
    }
}

impl Drop for UsbWatcher {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if self.hwnd != 0 {
                unsafe {
                    let _ = windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                        self.hwnd as windows_sys::Win32::Foundation::HWND,
                        windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                        0,
                        0,
                    );
                }
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(not(windows))]
fn start_impl(_on_change: Box<dyn Fn() + Send>) -> Option<UsbWatcher> {
    None
}

#[cfg(windows)]
fn start_impl(on_change: Box<dyn Fn() + Send>) -> Option<UsbWatcher> {
    let (tx, rx) = std::sync::mpsc::channel::<usize>();
    let thread = std::thread::Builder::new()
        .name("usb-hotplug".into())
        .spawn(move || watcher_thread(on_change, tx))
        .ok()?;
    let hwnd = rx.recv_timeout(std::time::Duration::from_secs(5)).ok()?;
    if hwnd == 0 {
        let _ = thread.join();
        return None;
    }
    Some(UsbWatcher { hwnd, thread: Some(thread) })
}

#[cfg(windows)]
struct WndState {
    notify: windows_sys::Win32::UI::WindowsAndMessaging::HDEVNOTIFY,
    on_change: Box<dyn Fn() + Send>,
}

#[cfg(windows)]
fn watcher_thread(
    on_change: Box<dyn Fn() + Send>,
    ready: std::sync::mpsc::Sender<usize>,
) {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Devices::Usb::GUID_DEVINTERFACE_USB_DEVICE;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            match msg {
                WM_DEVICECHANGE => {
                    let ev = wparam as u32;
                    if ev == DBT_DEVICEARRIVAL || ev == DBT_DEVICEREMOVECOMPLETE || ev == DBT_DEVNODES_CHANGED {
                        // 重置去抖定时器：连续事件会被合并成一次刷新
                        let _ = SetTimer(hwnd, TIMER_ID, DEBOUNCE_MS, None);
                    }
                    0
                }
                WM_TIMER if wparam == TIMER_ID => {
                    let _ = KillTimer(hwnd, TIMER_ID);
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WndState;
                    if !ptr.is_null() {
                        ((*ptr).on_change)();
                    }
                    0
                }
                WM_CLOSE => {
                    let _ = DestroyWindow(hwnd);
                    0
                }
                WM_DESTROY => {
                    let _ = KillTimer(hwnd, TIMER_ID);
                    let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut WndState;
                    if !ptr.is_null() {
                        let state = Box::from_raw(ptr);
                        if !state.notify.is_null() {
                            let _ = UnregisterDeviceNotification(state.notify);
                        }
                    }
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }

    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        if hinstance.is_null() {
            let _ = ready.send(0);
            return;
        }

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: CLASS_NAME.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            let _ = ready.send(0);
            return;
        }

        let hwnd = CreateWindowExW(
            0,
            CLASS_NAME.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            let _ = UnregisterClassW(CLASS_NAME.as_ptr(), hinstance);
            let _ = ready.send(0);
            return;
        }

        let mut filter: DEV_BROADCAST_DEVICEINTERFACE_W = zeroed();
        filter.dbcc_size = size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32;
        filter.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
        filter.dbcc_classguid = GUID_DEVINTERFACE_USB_DEVICE;
        let notify = RegisterDeviceNotificationW(
            hwnd,
            &filter as *const _ as *const _,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        );

        let state = Box::new(WndState { notify, on_change });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        let _ = ready.send(hwnd as usize);

        let mut msg: MSG = zeroed();
        loop {
            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r == 0 || r == -1 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnregisterClassW(CLASS_NAME.as_ptr(), hinstance);
    }
}
