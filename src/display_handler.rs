#[cfg(windows)] extern crate winapi;
use std::io::Error;
use winapi::um::winuser::SPI_GETBEEP;

#[cfg(windows)]
fn print_message(msg: &str) -> Result<i32, Error> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use winapi::um::winuser::{MB_OK, MessageBoxW};
    let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
    let title: Vec<u16> = OsStr::new("Launcher").encode_wide().chain(once(0)).collect();
    let ret = unsafe {
        MessageBoxW(null_mut(), wide.as_ptr(), title.as_ptr(), MB_OK)
    };
    if ret == 0 { Err(Error::last_os_error()) }
    else { Ok(ret) }
}

#[cfg(not(windows))]
fn print_message(msg: &str) -> Result<(), Error> {
    println!("{}", msg);
    Ok(())
}
//todo this or windows crate
pub fn message(msg: &str) {
    unsafe {
        use winapi::um::winuser::{MessageBeep};
        MessageBeep(SPI_GETBEEP);
    }
    if let Ok(_) = print_message(msg) {
        // no-op
    }
}