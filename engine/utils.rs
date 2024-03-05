use std::borrow::Cow;
use std::ffi::{c_char, CStr, CString};
use std::fmt::Display;

use glam::{Mat3, Mat4};
use log::debug;

use crate::logger;

pub trait CheckError<T> {
    #[track_caller]
    fn check_err(self, action: &'static str) -> T;
}

impl<T> CheckError<T> for Option<T> {
    fn check_err(self, action: &'static str) -> T {
        match self {
            Some(t) => t,
            None => panic!("failed to {}", action),
        }
    }
}

impl<T, E: Display> CheckError<T> for Result<T, E> {
    fn check_err(self, action: &'static str) -> T {
        match self {
            Ok(t) => t,
            Err(e) => panic!("failed to {}: err = {}", action, e),
        }
    }
}

pub const fn cstr(bytes: &[u8]) -> &CStr {
    match CStr::from_bytes_with_nul(bytes) {
        Ok(c) => c,
        Err(_) => panic!("failed to construct CStr"),
    }
}

pub const fn str_to_u32(s: &str) -> u32 {
    assert!(s.is_ascii());

    let mut bytes = s.as_bytes();
    let mut num = 0;

    while let [byte, rest @ ..] = bytes {
        let byte = *byte;
        assert!(b'0' <= byte && byte <= b'9');

        let digit = (byte - b'0') as u32;

        num *= 10;
        num += digit;

        bytes = rest;
    }

    num
}

#[allow(clippy::cast_possible_wrap)]
pub const fn to_i32(x: u32) -> i32 {
    (x & 0x7fff_ffff) as i32
}

#[allow(clippy::cast_precision_loss, clippy::manual_assert)]
pub const fn to_f32(x: u32) -> f32 {
    // 23 set bits, i.e. mantissa of f32
    let max = 0x007f_ffff;

    if x > max {
        panic!("maximum value exceeded");
    }

    x as f32
}

#[allow(clippy::cast_precision_loss)]
pub const fn i32_to_f32(x: i32) -> f32 {
    // 23 set bits, i.e. mantissa of f32
    let max = 0x007f_ffff;

    assert!(x <= max, "maximum value exceeded");

    x as f32
}

#[allow(clippy::cast_possible_truncation)]
pub const fn pair_to_i32(pair: (f64, f64)) -> (i32, i32) {
    let (x, y) = pair;

    (x as i32, y as i32)
}

#[allow(clippy::cast_possible_truncation, clippy::manual_assert)]
pub const fn to_u32(len: usize) -> u32 {
    let max = u32::MAX as usize;

    if len > max {
        panic!("maximum length exceeded");
    }

    len as u32
}

pub fn convert_to_strings(strs: &[&str]) -> Vec<String> {
    strs.iter().map(ToString::to_string).collect()
}

pub fn convert_to_c_strs(strings: &[String]) -> Vec<CString> {
    strings
        .iter()
        .map(|string| CString::new(string.clone()).check_err("convert to CString"))
        .collect()
}

pub fn convert_to_c_ptrs(cstrings: &[CString]) -> Vec<*const c_char> {
    cstrings.iter().map(|cstring| cstring.as_ptr()).collect()
}

pub fn pack_to_u32s(bytes: &[u8]) -> Vec<u32> {
    assert!(bytes.len() % 4 == 0, "code length must be a multiple of 4");

    bytes
        .chunks_exact(4)
        .map(|chunk| match chunk {
            &[b0, b1, b2, b3] => u32::from_ne_bytes([b0, b1, b2, b3]),
            _ => unreachable!(),
        })
        .collect()
}

pub fn print_textual_items<T>(
    desc: &'static str,
    items: &[T],
    as_ptr: impl Fn(&T) -> *const c_char,
) {
    let cols = 86;
    let mut buffer = format!("{}: ", desc);

    for item in items {
        let cstr = unsafe { CStr::from_ptr(as_ptr(item)) };
        let name = cstr.to_str().unwrap_or("unknown");

        if buffer.len() > cols {
            debug!("{}", buffer);
            buffer = "    ".to_owned();
        }

        buffer.push_str(name);
        buffer.push(' ');
    }

    debug!("{}", buffer);
}

pub fn print_item_list<T>(desc: &'static str, items: &[T], format: impl Fn(&T) -> String) {
    if !logger::verbose() {
        return;
    }

    debug!("{}:", desc);

    for item in items {
        debug!("    {}", format(item));
    }
}

pub const unsafe fn any_as_bytes<T>(x: &T) -> &[u8] {
    let data = (x as *const T).cast::<u8>();
    let len = std::mem::size_of::<T>();

    std::slice::from_raw_parts(data, len)
}

pub fn list_without_element(original: &[u32], needle: u32) -> Vec<u32> {
    original.iter().filter(|&x| *x != needle).copied().collect()
}

pub fn list_without_elements(original: &[u32], needles: &[u32]) -> Vec<u32> {
    let in_needles = |&x| needles.iter().any(|&n| n == x);

    original.iter().filter(|&x| !in_needles(x)).copied().collect()
}

pub fn print_mat3(mat: &Mat3) {
    let num_items = 3;
    let precision = 2;
    let width = calc_width(&mat.to_cols_array(), precision);
    let num_whitespace = 1 + num_items * (width + 1);

    let w = width;
    let p = precision;
    let s = num_whitespace;

    let r0 = mat.row(0);
    let r1 = mat.row(1);
    let r2 = mat.row(2);

    println!("┌{:s$}┐", ' ');
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} │", r0.x, r0.y, r0.z);
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} │", r1.x, r1.y, r1.z);
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} │", r2.x, r2.y, r2.z);
    println!("└{:s$}┘", ' ');
}

fn calc_width(xs: &[f32], precision: usize) -> usize {
    let mut decimal_width = 2;

    for x in xs {
        if *x >= 10.0 {
            decimal_width = 3;
        }
    }

    decimal_width + 1 + precision
}

pub fn print_mat4(mat: &Mat4) {
    let num_items = 4;
    let precision = 2;
    let width = calc_width(&mat.to_cols_array(), precision);
    let num_whitespace = 1 + num_items * (width + 1);

    let w = width;
    let p = precision;
    let s = num_whitespace;

    let r0 = mat.row(0);
    let r1 = mat.row(1);
    let r2 = mat.row(2);
    let r3 = mat.row(3);

    println!("┌{:s$}┐", ' ');
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} {:w$.p$} │", r0.x, r0.y, r0.z, r0.w);
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} {:w$.p$} │", r1.x, r1.y, r1.z, r1.w);
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} {:w$.p$} │", r2.x, r2.y, r2.z, r2.w);
    println!("│ {:w$.p$} {:w$.p$} {:w$.p$} {:w$.p$} │", r3.x, r3.y, r3.z, r3.w);
    println!("└{:s$}┘", ' ');
}

pub const fn opt_to_ptr<T>(opt: &Option<T>) -> *const T {
    match opt.as_ref() {
        Some(x) => x,
        None => std::ptr::null(),
    }
}

pub fn cstr_to_cow(ptr: *const c_char) -> Cow<'static, str> {
    if ptr.is_null() {
        Cow::from("")
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy()
    }
}
