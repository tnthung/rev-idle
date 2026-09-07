use windows::{
    core::Error as WinError,
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL},
        System::{
            Console::GetConsoleWindow,
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, SetClipboardData,
            },
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
        },
    },
};

fn free_global(memory: HGLOBAL) {
    unsafe {
        let _ = GlobalFree(Some(memory));
    }
}

fn unicode_clipboard_contents(text: &str) -> Result<Vec<u16>, String> {
    if text.contains('\0') {
        return Err("clipboard text cannot contain a null character".to_string());
    }

    let text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n");
    Ok(text.encode_utf16().chain(std::iter::once(0)).collect())
}

const CLIPBOARD_OPEN_ATTEMPTS: u32 = 10;
const CLIPBOARD_OPEN_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(20);

fn open_clipboard_with_retry(owner: windows::Win32::Foundation::HWND) -> Result<(), String> {
    let mut last_error = None;
    for attempt in 0..CLIPBOARD_OPEN_ATTEMPTS {
        match unsafe { OpenClipboard(Some(owner)) } {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < CLIPBOARD_OPEN_ATTEMPTS {
                    std::thread::sleep(CLIPBOARD_OPEN_RETRY_DELAY);
                }
            }
        }
    }

    Err(format!(
        "OpenClipboard failed: {}",
        last_error.expect("loop always sets last_error before exhausting attempts")
    ))
}

pub(super) fn write_unicode_clipboard(text: &str) -> Result<(), String> {
    let owner = unsafe { GetConsoleWindow() };
    if owner.is_invalid() {
        return Err("GetConsoleWindow returned no console window".to_string());
    }

    let contents = unicode_clipboard_contents(text)?;
    let byte_count = contents
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| "clipboard text is too large".to_string())?;
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_count) }
        .map_err(|error| format!("GlobalAlloc failed: {error}"))?;
    let destination = unsafe { GlobalLock(memory) };
    if destination.is_null() {
        let error = WinError::from_win32();
        free_global(memory);
        return Err(format!("GlobalLock failed: {error}"));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(contents.as_ptr(), destination.cast::<u16>(), contents.len());
        let _ = GlobalUnlock(memory);
    }

    if let Err(error) = open_clipboard_with_retry(owner) {
        free_global(memory);
        return Err(error);
    }

    let result = unsafe { EmptyClipboard() }
        .map_err(|error| format!("EmptyClipboard failed: {error}"))
        .and_then(|()| {
            unsafe {
                SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(memory.0)))
            }
            .map(|_| ())
            .map_err(|error| format!("SetClipboardData failed: {error}"))
        });
    let close_result = unsafe { CloseClipboard() }
        .map_err(|error| format!("CloseClipboard failed: {error}"));

    if result.is_err() {
        free_global(memory);
    }
    result?;
    close_result
}

pub(super) fn read_unicode_clipboard() -> Result<String, String> {
    let owner = unsafe { GetConsoleWindow() };
    if owner.is_invalid() {
        return Err("GetConsoleWindow returned no console window".to_string());
    }

    open_clipboard_with_retry(owner)?;

    let result = (|| {
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32) }.is_err() {
            return Ok(String::new());
        }
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32) }
            .map_err(|error| format!("GetClipboardData failed: {error}"))?;
        let memory = HGLOBAL(handle.0);
        let source = unsafe { GlobalLock(memory) };
        if source.is_null() {
            let error = WinError::from_win32();
            return Err(format!("GlobalLock failed: {error}"));
        }

        let text = unsafe {
            let mut length = 0usize;
            while *source.cast::<u16>().add(length) != 0 {
                length += 1;
            }
            let slice = std::slice::from_raw_parts(source.cast::<u16>(), length);
            String::from_utf16_lossy(slice)
        };

        let _ = unsafe { GlobalUnlock(memory) };
        Ok(text)
    })();

    let close_result = unsafe { CloseClipboard() }
        .map_err(|error| format!("CloseClipboard failed: {error}"));

    let text = result?;
    close_result?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_clipboard_contents_normalizes_line_endings() {
        assert_eq!(
            unicode_clipboard_contents("a\nb\rc\r\nd").unwrap(),
            vec![97, 13, 10, 98, 13, 10, 99, 13, 10, 100, 0]
        );
    }
}
