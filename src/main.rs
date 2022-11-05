#![windows_subsystem = "windows"]

use std::fs::File;
use std::ffi::OsStr;
use std::{mem, ffi::OsString};
use std::os::windows::ffi::OsStringExt;
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{
            WindowsAndMessaging::*,
            Shell::*,
        },
        System::LibraryLoader::GetModuleHandleW,
    }
};

#[derive(Debug)]
pub struct App {
    hedit: HWND,
}

impl Default for App {
    fn default() -> Self {
        App {
            hedit: HWND(0),
        }
    }
}

unsafe fn get_app_from_window<'a>(hwnd: HWND) -> Option<&'a mut App> {
    let user_data = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    user_data.as_mut()
}

fn get_png_metadata(filename: &OsStr) -> anyhow::Result<String> {
    let f = File::open(filename)?;
    let decoder = png::Decoder::new(f);
    let reader = decoder.read_info()?;
    let info = reader.info();
    let mut ret = String::new();
    for chunk in &info.uncompressed_latin1_text {
        let text = chunk.text.replace("\n", "\r\n");
        ret.push_str("【");
        ret.push_str(&chunk.keyword);
        ret.push_str("】\r\n");
        ret.push_str(&text);
        ret.push_str("\r\n\r\n");
    }
    Ok(ret)
}

macro_rules! loword {
    ( $x:expr ) => {
        ((($x.0 as u32) & 0xffffu32) as u16).into()
    };
}

macro_rules! hiword {
    ( $x:expr ) => {
        ((($x.0 as u32) >> 16u32) as u16).into()
    };
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_CREATE => {
            let instance = unsafe { GetModuleHandleW(None) }.unwrap();
            let create_struct: &CREATESTRUCTW = unsafe { mem::transmute(lparam) };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_struct.lpCreateParams as _) };

            // TextBox 作成
            let app = unsafe { get_app_from_window(hwnd) }.unwrap();
            let hedit = unsafe { CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("EDIT"),
                None,
                WINDOW_STYLE(
                    WS_CHILD.0 | WS_VISIBLE.0 |
                    ES_WANTRETURN as u32 | ES_MULTILINE as u32 |
                    ES_AUTOVSCROLL as u32 | WS_VSCROLL.0),
                0, 0, 0, 0,
                hwnd, HMENU(1234), instance, None) };
            app.hedit = hedit;
            unsafe { SetWindowTextW(hedit, w!("DRAG AND DROP HERE!!")) };

            // フォントの作成
            let hfont = unsafe { CreateFontW(
                22, 0, 0, 0,
                400,
                0, 0, 0,
                ANSI_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32,
                w!("Georgia"),
            ) };
            unsafe { SendMessageW(hedit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(0)) };

            // ファイルのドラッグアンドドロップを許可
            unsafe { DragAcceptFiles(hwnd, true) };

            LRESULT::default()
        }
        WM_SIZE => {
            if let Some(app) = unsafe { get_app_from_window(hwnd) } {
                unsafe { MoveWindow(app.hedit, 0, 0, loword!(lparam), hiword!(lparam), true) };
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_DROPFILES => {
            if let Some(app) = unsafe { get_app_from_window(hwnd) } {
                let hdrop = HDROP(wparam.0 as isize);
                let n_files = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
                if n_files > 0 {
                    let mut buf: Vec<u16> = vec![0; 1024];
                    unsafe { DragQueryFileW(hdrop, 0, Some(&mut buf)) };
                    let last = buf.iter().position(|&x| x == 0).unwrap_or(buf.len());
                    let filename = OsString::from_wide(&buf[0..last]);
                    match get_png_metadata(&filename) {
                        Ok(metadata) => {
                            let new_text = HSTRING::from(metadata);
                            unsafe { SetWindowTextW(app.hedit, &new_text) };
                        },
                        Err(e) => {
                            let new_text = HSTRING::from(format!("ERROR: {e}"));
                            unsafe { SetWindowTextW(app.hedit, &new_text) };
                        }
                    }
                }
            }
            LRESULT::default()
        }
        WM_DESTROY => {
            if let Some(app) = unsafe { get_app_from_window(hwnd) } {
                unsafe { DestroyWindow(app.hedit) };
            }
            unsafe { PostQuitMessage(0) };
            LRESULT::default()
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

pub fn create_window(app: &mut App, width: i32, height: i32) -> anyhow::Result<()> {
    let instance = unsafe { GetModuleHandleW(None) }?;

    let class_name = w!("MetaView");

    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: class_name,
        hbrBackground: HBRUSH(unsafe { GetStockObject(WHITE_BRUSH) }.0),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassExW(&wc) };
    anyhow::ensure!(atom != 0, "RegisterClassExW failed");

    let mut window_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false) };

    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("MetaView"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            None,
            None,
            instance,
            Some(app as *mut _ as _),
        )
    };
    Ok(())
}

pub fn main_loop() -> anyhow::Result<()> {
    loop {
        let mut message = MSG::default();
        let ret = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if ret == -1 {
            anyhow::bail!("GetMessageW failed");
        }
        if ret == 0 {
            return Ok(());
        }
        unsafe { TranslateMessage(&message) };
        unsafe { DispatchMessageW(&message) };
    }
}

fn main() -> anyhow::Result<()> {
    let mut app = App {
        ..Default::default()
    };
    create_window(&mut app, 800, 800)?;
    main_loop()
}
