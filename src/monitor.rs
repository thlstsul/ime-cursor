use euro_focus::subscribe_focus_changes;
use rdev::{EventType, Key, listen};

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::cursor::Cursor;
use crate::ime::{IMEControl, InputMode};

struct MayChangeIME;

pub struct Monitor {
    ime: IMEControl,
    cursor: Cursor,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor {
            ime: IMEControl::new(500, true),
            cursor: Cursor::new(),
        }
    }

    pub fn run(&mut self) {
        let (sender, receiver) = channel();

        let _keyboard_handle = Self::listen_keyboard(sender.clone());
        let _window_handle = Self::listen_window(sender);

        let _ = self.cursor.reset_cursor();
        self.set_cursor();

        let (delay_sender, delay_receiver) = channel();
        let _delay_send_handle = Self::delay_send(receiver, delay_sender);

        while let Ok(_) = delay_receiver.recv() {
            self.set_cursor();
        }
    }

    fn set_cursor(&mut self) {
        if let Ok(mode) = self.ime.get_input_mode() {
            if mode.is_cn {
                let _ = self.cursor.set_chinesn_cursor();
            } else {
                let _ = self.cursor.reset_cursor();
            }
        }
    }

    fn delay_send(
        receiver: Receiver<MayChangeIME>,
        delay_sender: Sender<MayChangeIME>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut last_time: Option<(Instant, MayChangeIME)> = None;
            loop {
                if let Some((l, _)) = last_time
                    && l.elapsed().as_millis() > 150
                    && let Some((_, e)) = last_time.take()
                {
                    let _ = delay_sender.send(e);
                }

                if let Ok(e) = receiver.try_recv() {
                    last_time = Some((Instant::now(), e));
                }
            }
        })
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
            let receiver = subscribe_focus_changes().expect("启动窗口监听失败");

            while let Ok(_) = receiver.recv() {
                let _ = sender.send(MayChangeIME);
            }
        })
    }
}
