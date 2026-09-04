use anyhow::Result;
use rdev::{EventType, Key, listen};
use wintrack::WindowEventKind;

use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::channel::{Sender, channel};
use crate::cursor::Cursor;
use crate::ime::{IMEControl, InputMode};

struct MayChangeIME;

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
        if let Ok(mode) = self.ime.get_input_mode() {
            if mode.is_cn {
                self.cursor.set_chinesn_cursor()?;
            } else {
                self.cursor.set_default_cursor()?;
            }
        }

        Ok(())
    }

    fn listen_keyboard(sender: Sender<MayChangeIME>) -> JoinHandle<()> {
        thread::spawn(move || {
            listen(move |event| {
                if let EventType::KeyRelease(key) = event.event_type
                    && matches!(
                        key,
                        Key::ControlLeft
                            | Key::ControlRight
                            | Key::ShiftLeft
                            | Key::ShiftRight
                            | Key::MetaLeft
                            | Key::MetaRight
                    )
                {
                    let _ = sender.send(MayChangeIME);
                }
            })
            .expect("启动键盘监听失败");
        })
    }

    fn listen_window(sender: Sender<MayChangeIME>) -> JoinHandle<()> {
        thread::spawn(move || {
            wintrack::try_hook().expect("启动窗口监听失败");

            let (tx, rx) = std::sync::mpsc::channel();
            wintrack::set_callback(Box::new(move |event| {
                if event.kind == WindowEventKind::ForegroundWindowChanged {
                    let _ = tx.send(());
                }
            }));

            while rx.recv().is_ok() {
                let _ = sender.send(MayChangeIME);
            }
        })
    }
}
