use anyhow::{Result, anyhow};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Input::{Ime::ImmGetDefaultIMEWnd, KeyboardAndMouse::GetKeyboardLayout},
        WindowsAndMessaging::{
            GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId, SMTO_ABORTIFHUNG,
            SendMessageTimeoutW, WM_IME_CONTROL,
        },
    },
};

use super::{IMEControl, IMEResponse, InputMode};

impl InputMode for IMEControl {
    fn new(check_timeout: u32, base_status: bool) -> Self {
        Self {
            check_timeout,
            base_status,
            ..Default::default()
        }
    }

    fn get_input_mode(&self) -> Result<IMEResponse> {
        self.get_input_mode(None)
    }

    fn set_input_mode(&self, is_cn: bool) -> Result<()> {
        self.set_input_mode(is_cn, None)
    }
}

#[allow(dead_code)]
impl IMEControl {
    /// 获取当前输入模式
    fn get_input_mode(&self, hwnd: Option<HWND>) -> Result<IMEResponse> {
        let hwnd = Self::ensure_hwnd(hwnd)?;

        if self.status_mode == 0
            && self.even_status_mode.is_none()
            && self.conversion_mode == 0
            && self.even_conversion_mode.is_none()
        {
            if self.get_open_status_internal(hwnd)? == 0 {
                return Ok(IMEResponse {
                    code: 0,
                    is_cn: false,
                });
            }
            let v = self.get_conversion_mode_internal(hwnd)?;
            return Ok(IMEResponse {
                code: v,
                is_cn: (v & 1) != 0,
            });
        }

        let v = self.get_conversion_mode_internal(hwnd)?;
        let flag = (v & 1) != 0;

        // 转换码逻辑
        if self.base_status {
            if let Some(even_cm) = self.even_conversion_mode {
                return Ok(IMEResponse {
                    code: v,
                    is_cn: even_cm ^ flag,
                });
            }
            if self.conversion_mode != 0 {
                let contains = format!(":{}:", v).contains(&format!(":{}:", self.conversion_mode));
                return Ok(IMEResponse {
                    code: v,
                    is_cn: contains,
                });
            }
        } else {
            if let Some(even_cm) = self.even_conversion_mode {
                return Ok(IMEResponse {
                    code: v,
                    is_cn: even_cm && flag,
                });
            }
            if self.conversion_mode != 0 {
                let contains = format!(":{}:", v).contains(&format!(":{}:", self.conversion_mode));
                return Ok(IMEResponse {
                    code: 0,
                    is_cn: !contains,
                });
            }
        }

        // 状态码逻辑
        let v = self.get_open_status_internal(hwnd)?;
        let flag = (v & 1) != 0;

        if self.base_status {
            if let Some(even_sm) = self.even_status_mode {
                return Ok(IMEResponse {
                    code: v,
                    is_cn: even_sm ^ flag,
                });
            }
            if self.status_mode != 0 {
                let contains = format!(":{}:", v).contains(&format!(":{}:", self.status_mode));
                return Ok(IMEResponse {
                    code: 0,
                    is_cn: contains,
                });
            }
        } else {
            if let Some(even_sm) = self.even_status_mode {
                return Ok(IMEResponse {
                    code: v,
                    is_cn: even_sm && flag,
                });
            }
            if self.status_mode != 0 {
                let contains = format!(":{}:", v).contains(&format!(":{}:", self.status_mode));
                return Ok(IMEResponse {
                    code: 0,
                    is_cn: !contains,
                });
            }
        }

        Err(anyhow!("Invalid IME state"))
    }

    /// 设置输入模式
    fn set_input_mode(&self, is_cn: bool, hwnd: Option<HWND>) -> Result<()> {
        let hwnd = Self::ensure_hwnd(hwnd)?;

        if is_cn {
            self.set_open_status_internal(true, hwnd)?;
            match Self::get_keyboard_layout(hwnd)? as u32 {
                0x08040804 => self.set_conversion_mode_internal(1025, hwnd)?,
                0x04110411 => self.set_conversion_mode_internal(9, hwnd)?,
                _ => (),
            }
        } else {
            self.set_open_status_internal(false, hwnd)?;
        }
        Ok(())
    }

    // 内部实现方法
    fn get_open_status_internal(&self, hwnd: HWND) -> Result<isize> {
        let ime_wnd = unsafe { ImmGetDefaultIMEWnd(hwnd) };
        let mut status = 0;

        unsafe {
            SendMessageTimeoutW(
                ime_wnd,
                WM_IME_CONTROL,
                WPARAM(0x5),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                self.check_timeout,
                Some(&mut status as *mut _ as *mut _),
            );
        }

        Ok(status)
    }

    fn set_open_status_internal(&self, status: bool, hwnd: HWND) -> Result<()> {
        let ime_wnd = unsafe { ImmGetDefaultIMEWnd(hwnd) };
        let status = isize::from(status);

        unsafe {
            SendMessageTimeoutW(
                ime_wnd,
                WM_IME_CONTROL,
                WPARAM(0x6),
                LPARAM(status),
                SMTO_ABORTIFHUNG,
                self.check_timeout,
                None,
            );
        }

        Ok(())
    }

    fn get_conversion_mode_internal(&self, hwnd: HWND) -> Result<isize> {
        let ime_wnd = unsafe { ImmGetDefaultIMEWnd(hwnd) };
        let mut mode = 0;

        unsafe {
            SendMessageTimeoutW(
                ime_wnd,
                WM_IME_CONTROL,
                WPARAM(0x1),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                self.check_timeout,
                Some(&mut mode as *mut _ as *mut _),
            );
        }

        Ok(mode)
    }

    fn set_conversion_mode_internal(&self, mode: isize, hwnd: HWND) -> Result<()> {
        let ime_wnd = unsafe { ImmGetDefaultIMEWnd(hwnd) };

        unsafe {
            SendMessageTimeoutW(
                ime_wnd,
                WM_IME_CONTROL,
                WPARAM(0x2),
                LPARAM(mode),
                SMTO_ABORTIFHUNG,
                self.check_timeout,
                None,
            );
        }

        Ok(())
    }

    fn get_keyboard_layout(hwnd: HWND) -> Result<isize> {
        let tid = unsafe { GetWindowThreadProcessId(hwnd, None) };
        Ok(unsafe { GetKeyboardLayout(tid) }.0 as isize)
    }

    fn ensure_hwnd(hwnd: Option<HWND>) -> Result<HWND> {
        hwnd.ok_or_else(Self::get_focused_window)
            .or_else(|_| Self::get_focused_window())
    }

    fn get_focused_window() -> Result<HWND> {
        let mut gui_thread_info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };

        unsafe {
            if GetGUIThreadInfo(0, &mut gui_thread_info).is_ok() {
                return Ok(gui_thread_info.hwndFocus);
            }
        }

        Err(anyhow!("Failed to get focused window"))
    }
}

// 使用示例
#[test]
fn test() -> Result<()> {
    // 初始化配置
    let ctrl = IMEControl::new(500, true);

    // 获取当前输入模式
    let mode1 = ctrl.get_input_mode(None)?;
    println!("Current IME mode: {:?}", mode1);

    // 切换输入法状态
    ctrl.set_input_mode(!mode1.is_cn, None)?;

    let mode2 = ctrl.get_input_mode(None)?;
    println!("Current IME mode: {:?}", mode2);

    assert_ne!(mode1.is_cn, mode2.is_cn);
    Ok(())
}
