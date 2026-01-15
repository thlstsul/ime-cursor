#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::monitor::Monitor;

mod channel;
mod cursor;
mod ime;
mod monitor;

fn main() {
    let mut monitor = Monitor::new().expect("创建监听器失败");
    monitor.run();
}
