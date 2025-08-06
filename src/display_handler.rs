use std::io::Error;

#[cfg(target_os = "windows")]
fn print_message(msg: &str) -> Result<i32, Error> {
    use windows_sys::{
        core::*, Win32::UI::WindowsAndMessaging::*,
    };
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
    let ret = unsafe {
        MessageBoxW(0 as _, wide.as_ptr(), w!("Launcher"), MB_OK)
    };
    if ret == 0 {
        Err(Error::last_os_error())
    } else {
        Ok(ret)
    }
}

#[cfg(not(target_os = "windows"))]
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
