#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::monitor::Monitor;

mod cursor;
mod ime;
mod monitor;

fn main() {
    let mut monitor = Monitor::new();
    monitor.run();
}
