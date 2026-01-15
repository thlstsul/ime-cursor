use windows::{
    Win32::UI::WindowsAndMessaging::{LoadCursorFromFileW, OCR_IBEAM, OCR_NORMAL, SetSystemCursor},
    core::{PCWSTR, Result, w},
};

const CHI_ARROW_CURSOR_PATH: PCWSTR = w!(".\\assets\\arrow_chi.cur");
const CHI_IBEAM_CURSOT_PATH: PCWSTR = w!(".\\assets\\ibeam_chi.cur");
const ORI_ARROW_CURSOR_PATH: PCWSTR = w!("C:\\Windows\\Cursors\\aero_arrow.cur");
const ORI_IBEAM_CURSOR_PATH: PCWSTR = w!("C:\\Windows\\Cursors\\beam_i.cur");

pub struct Cursor {
    is_cn: bool,
}

impl Cursor {
    pub fn new() -> Result<Self> {
        Self::reset_cursor()?;

        Ok(Cursor { is_cn: false })
    }

    pub fn set_chinesn_cursor(&mut self) -> Result<()> {
        if self.is_cn {
            return Ok(());
        }

        unsafe {
            SetSystemCursor(LoadCursorFromFileW(CHI_ARROW_CURSOR_PATH)?, OCR_NORMAL)?;
            SetSystemCursor(LoadCursorFromFileW(CHI_IBEAM_CURSOT_PATH)?, OCR_IBEAM)?;
        }
        self.is_cn = true;

        Ok(())
    }

    pub fn set_default_cursor(&mut self) -> Result<()> {
        if !self.is_cn {
            return Ok(());
        }

        Self::reset_cursor()?;
        self.is_cn = false;

        Ok(())
    }

    fn reset_cursor() -> Result<()> {
        unsafe {
            SetSystemCursor(LoadCursorFromFileW(ORI_ARROW_CURSOR_PATH)?, OCR_NORMAL)?;
            SetSystemCursor(LoadCursorFromFileW(ORI_IBEAM_CURSOR_PATH)?, OCR_IBEAM)?;
        }

        Ok(())
    }
}
