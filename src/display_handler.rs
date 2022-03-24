#[cfg(windows)]
extern crate winapi;

use std::io::Error;

#[cfg(windows)]
fn print_message(msg: &str) -> Result<i32, Error> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use winapi::um::winuser::{MessageBoxW, MB_OK};
    let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
    let title: Vec<u16> = OsStr::new("Launcher").encode_wide().chain(once(0)).collect();
    let ret = unsafe {
        use winapi::um::winuser::{MessageBeep, SPI_GETBEEP};
        MessageBeep(SPI_GETBEEP);
        MessageBoxW(null_mut(), wide.as_ptr(), title.as_ptr(), MB_OK)
    };
    if ret == 0 {
        Err(Error::last_os_error())
    } else {
        Ok(ret)
    }
}

#[cfg(not(windows))]
fn print_message(msg: &str) -> Result<(), Error> {
    println!("{}", msg);
    Ok(())
}

/// Display a native message dialog.<br>
/// Eats any errors that occur.
pub fn message(msg: &str) {
    if let Ok(_) = print_message(msg) {
        // no-op
    }
}
