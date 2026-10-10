use anyhow::Result;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        Input::KeyboardAndMouse::{
            VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RSHIFT, VK_RWIN,
        },
        WindowsAndMessaging::{
            CallNextHookEx, EVENT_SYSTEM_FOREGROUND, GetMessageW, KBDLLHOOKSTRUCT, MSG,
            SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WINEVENT_OUTOFCONTEXT,
            WM_KEYUP, WM_SYSKEYUP,
        },
    },
};

use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::channel::{Sender, channel};
use crate::cursor::Cursor;
use crate::ime::{IMEControl, InputMode};

struct MayChangeIME;

// FFI 回调不能捕获环境，通过静态槽把 sender 传给回调
static WINDOW_SENDER: Mutex<Option<Sender<MayChangeIME>>> = Mutex::new(None);
static KEYBOARD_SENDER: Mutex<Option<Sender<MayChangeIME>>> = Mutex::new(None);

// 低级键盘钩子回调：修饰键（Ctrl/Shift/Win）抬起时通知，绝不 panic
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && (wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize) {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = info.vkCode;
        const MODIFIER_KEYS: [u16; 6] = [
            VK_LCONTROL.0,
            VK_RCONTROL.0,
            VK_LSHIFT.0,
            VK_RSHIFT.0,
            VK_LWIN.0,
            VK_RWIN.0,
        ];
        if u16::try_from(vk).is_ok_and(|vk| MODIFIER_KEYS.contains(&vk))
            && let Ok(guard) = KEYBOARD_SENDER.lock()
            && let Some(sender) = guard.as_ref()
        {
            let _ = sender.send(MayChangeIME);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub struct Monitor {
    ime: IMEControl,
    cursor: Cursor,
}

impl Monitor {
    pub fn new() -> Result<Self> {
        Ok(Monitor {
            ime: IMEControl::new(500, true),
            cursor: Cursor::new()?,
        })
    }

    pub fn run(&mut self) {
        let (sender, receiver) = channel(Duration::from_millis(150));

        let _keyboard_handle = Self::listen_keyboard(sender.clone());
        let _window_handle = Self::listen_window(sender);

        while receiver.recv().is_ok() {
            let _ = self.set_cursor();
        }
    }

    fn set_cursor(&mut self) -> Result<()> {
        // IME 切换是异步的：事件到达时输入法往往尚未完成切换。
        // 以当前光标状态为基线轮询，读到与基线不同的状态才说明切换已完成；
        // 若整个等待窗口内 IME 始终没变，则无需移动光标，直接返回。
        let baseline = self.cursor.is_cn();
        let deadline = Instant::now() + Duration::from_millis(300);

        let mut mode = None;
        while Instant::now() < deadline {
            if let Ok(current) = self.ime.get_input_mode()
                && current.is_cn != baseline
            {
                mode = Some(current);
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        if let Some(mode) = mode {
            if mode.is_cn {
                self.cursor.set_chinese_cursor()?;
            } else {
                self.cursor.set_default_cursor()?;
            }
        }

        Ok(())
    }

    fn listen_keyboard(sender: Sender<MayChangeIME>) -> JoinHandle<()> {
        thread::spawn(move || {
            // 注意：guard 必须在安装钩子前释放，否则回调在同线程重入加锁会死锁
            {
                let Ok(mut slot) = KEYBOARD_SENDER.lock() else {
                    eprintln!("KEYBOARD_SENDER 锁中毒，键盘监听未启动");
                    return;
                };
                *slot = Some(sender);
            }

            let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) };
            let Ok(hook) = hook else {
                eprintln!("SetWindowsHookExW 失败，键盘监听未启动");
                // 释放槽内的 sender，让通道能正常关闭
                *KEYBOARD_SENDER.lock().unwrap() = None;
                return;
            };

            // 低级钩子的回调经由本线程的消息队列分发
            let mut msg = MSG::default();
            // GetMessageW 返回 0 表示 WM_QUIT，-1 表示错误，两者都应退出循环
            loop {
                let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                if ret.0 <= 0 {
                    break;
                }
            }

            let _ = unsafe { UnhookWindowsHookEx(hook) };
            *KEYBOARD_SENDER.lock().unwrap() = None;
        })
    }

    fn listen_window(sender: Sender<MayChangeIME>) -> JoinHandle<()> {
        thread::spawn(move || {
            // 注意：guard 必须在安装钩子前释放，否则回调在同线程重入加锁会死锁
            {
                let Ok(mut slot) = WINDOW_SENDER.lock() else {
                    eprintln!("WINDOW_SENDER 锁中毒，窗口监听未启动");
                    return;
                };
                *slot = Some(sender);
            }

            // 仅关心前台窗口变化事件；事件范围 min..=max 必须连续
            let hook = unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(on_foreground_changed),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                )
            };
            if hook.is_invalid() {
                eprintln!("SetWinEventHook 失败，窗口监听未启动");
                // 释放槽内的 sender，让通道能正常关闭
                *WINDOW_SENDER.lock().unwrap() = None;
                return;
            }

            // 消息循环：WinEventHook 的回调经由本线程的消息队列分发
            let mut msg = MSG::default();
            // GetMessageW 返回 0 表示 WM_QUIT，-1 表示错误，两者都应退出循环
            loop {
                let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                if ret.0 <= 0 {
                    break;
                }
            }

            let _ = unsafe { UnhookWinEvent(hook) };
            *WINDOW_SENDER.lock().unwrap() = None;

            #[allow(non_snake_case)]
            unsafe extern "system" fn on_foreground_changed(
                _hook: HWINEVENTHOOK,
                _event: u32,
                _hwnd: HWND,
                _id_object: i32,
                _id_child: i32,
                _thread: u32,
                _time: u32,
            ) {
                // 注意：这是 FFI 回调，绝不能 panic（会变成 0xC0000409 abort）
                if let Ok(guard) = WINDOW_SENDER.lock()
                    && let Some(sender) = guard.as_ref()
                {
                    let _ = sender.send(MayChangeIME);
                }
            }
        })
    }
}
