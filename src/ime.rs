use anyhow::Result;

#[cfg(target_os = "windows")]
mod windows;

#[allow(dead_code)]
pub trait InputMode {
    fn new(check_timeout: u32, base_status: bool) -> Self;
    fn get_input_mode(&self) -> Result<IMEResponse>;
    fn set_input_mode(&self, is_cn: bool) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IMEResponse {
    pub code: isize,
    pub is_cn: bool,
}

#[allow(dead_code)]
pub struct IMEControl {
    check_timeout: u32,
    base_status: bool,
    status_mode: i32,
    conversion_mode: i32,
    even_status_mode: Option<bool>,
    even_conversion_mode: Option<bool>,
}

impl Default for IMEControl {
    fn default() -> Self {
        Self {
            check_timeout: 1000,
            base_status: false,
            status_mode: 0,
            conversion_mode: 0,
            even_status_mode: None,
            even_conversion_mode: None,
        }
    }
}
