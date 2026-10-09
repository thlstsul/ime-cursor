use windows::{
    Win32::UI::WindowsAndMessaging::{
        LoadCursorFromFileW, OCR_HAND, OCR_IBEAM, OCR_NORMAL, SYSTEM_CURSOR_ID, SetSystemCursor,
    },
    core::{PCWSTR, Result, w},
};

const CURSOR_ICONS: [CursorIcon; 3] = [
    CursorIcon {
        id: OCR_NORMAL,
        origin: w!("C:\\Windows\\Cursors\\aero_arrow.cur"),
        chinese: w!(".\\assets\\arrow_chi.cur"),
    },
    CursorIcon {
        id: OCR_IBEAM,
        origin: w!("C:\\Windows\\Cursors\\beam_i.cur"),
        chinese: w!(".\\assets\\ibeam_chi.cur"),
    },
    CursorIcon {
        id: OCR_HAND,
        origin: w!("C:\\Windows\\Cursors\\aero_link.cur"),
        chinese: w!(".\\assets\\link_chi.cur"),
    },
];

struct CursorIcon {
    id: SYSTEM_CURSOR_ID,
    origin: PCWSTR,
    chinese: PCWSTR,
}

pub struct Cursor {
    is_cn: bool,
}

impl Cursor {
    /// 当前光标对应的输入法状态（true = 中文光标）
    pub fn is_cn(&self) -> bool {
        self.is_cn
    }

    pub fn new() -> Result<Self> {
        Self::reset_cursor()?;

        Ok(Cursor { is_cn: false })
    }

    pub fn set_chinese_cursor(&mut self) -> Result<()> {
        if self.is_cn {
            return Ok(());
        }

        for icon in CURSOR_ICONS {
            unsafe {
                SetSystemCursor(LoadCursorFromFileW(icon.chinese)?, icon.id)?;
            }
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
        for icon in CURSOR_ICONS {
            unsafe {
                SetSystemCursor(LoadCursorFromFileW(icon.origin)?, icon.id)?;
            }
        }

        Ok(())
    }
}
