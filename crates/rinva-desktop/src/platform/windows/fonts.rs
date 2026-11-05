//! Private, process-local fonts: nothing is installed into the user's Windows profile.
use super::ui::wide;
use crate::i18n::Language;
use std::{
    path::Path,
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
};
use windows_sys::Win32::Graphics::Gdi::{AddFontResourceExW, RemoveFontResourceExW, FR_PRIVATE};
static BUNDLED: AtomicBool = AtomicBool::new(false);
pub struct Fonts(Option<Vec<u16>>);
impl Fonts {
    pub unsafe fn load(executable: &Path) -> Result<Self, String> {
        let path = executable
            .parent()
            .ok_or("Executable folder missing")?
            .join("fonts/NotoSansCJK-Regular.ttc");
        if !path.exists() {
            return Ok(Self(None));
        }
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if AddFontResourceExW(path.as_ptr(), FR_PRIVATE, null_mut()) == 0 {
            return Err("Could not load bundled Noto Sans CJK font".into());
        }
        BUNDLED.store(true, Ordering::Relaxed);
        Ok(Self(Some(path)))
    }
}
impl Drop for Fonts {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            unsafe {
                RemoveFontResourceExW(path.as_ptr(), FR_PRIVATE, null_mut());
            }
            BUNDLED.store(false, Ordering::Relaxed);
        }
    }
}
pub fn family(language: Language) -> &'static str {
    if !BUNDLED.load(Ordering::Relaxed) {
        return language.font();
    }
    match language {
        Language::Korean => "Noto Sans CJK KR",
        Language::Chinese => "Noto Sans CJK SC",
        _ => "Noto Sans CJK JP",
    }
}
pub unsafe fn verify_glyphs(font: windows_sys::Win32::Graphics::Gdi::HFONT) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::*;
    let dc = CreateCompatibleDC(null_mut());
    let old = SelectObject(dc, font);
    let sample = wide("Русский English 한국어 日本語 中文");
    let mut glyphs = vec![0u16; sample.len() - 1];
    let result = GetGlyphIndicesW(
        dc,
        sample.as_ptr(),
        (sample.len() - 1) as i32,
        glyphs.as_mut_ptr(),
        GGI_MARK_NONEXISTING_GLYPHS,
    );
    SelectObject(dc, old);
    DeleteDC(dc);
    if result == GDI_ERROR as u32 || glyphs.contains(&0xffff) {
        Err("Font is missing Russian, Latin or CJK glyphs".into())
    } else {
        Ok(())
    }
}
