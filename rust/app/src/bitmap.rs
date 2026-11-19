use jni::sys::{JNIEnv, jobject};
use std::ffi::{c_char, c_int, c_void};
use std::ptr::NonNull;
use std::sync::OnceLock;
use tiny_skia::{Pixmap, PixmapMut};

#[repr(C)]
#[derive(Default)]
struct BitmapInfo {
    width: u32,
    height: u32,
    stride: u32,
    format: c_int,
    flags: u32,
}

type Info = unsafe extern "C" fn(*mut JNIEnv, jobject, *mut BitmapInfo) -> c_int;
type Lock = unsafe extern "C" fn(*mut JNIEnv, jobject, *mut *mut c_void) -> c_int;
type Unlock = unsafe extern "C" fn(*mut JNIEnv, jobject) -> c_int;

struct BitmapApi {
    info: Info,
    lock: Lock,
    unlock: Unlock,
}

static API: OnceLock<Option<BitmapApi>> = OnceLock::new();

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

fn api() -> Option<&'static BitmapApi> {
    API.get_or_init(|| {
        // Android exposes this public NDK library on every supported API level.
        let lib = unsafe { dlopen(c"libjnigraphics.so".as_ptr(), 2) };
        if lib.is_null() {
            return None;
        }
        let info = unsafe { dlsym(lib, c"AndroidBitmap_getInfo".as_ptr()) };
        let lock = unsafe { dlsym(lib, c"AndroidBitmap_lockPixels".as_ptr()) };
        let unlock = unsafe { dlsym(lib, c"AndroidBitmap_unlockPixels".as_ptr()) };
        if info.is_null() || lock.is_null() || unlock.is_null() {
            return None;
        }
        // These are the exact function signatures exported by android/bitmap.h.
        // The dlopen handle remains open for the process lifetime.
        Some(unsafe {
            BitmapApi {
                info: std::mem::transmute::<*mut c_void, Info>(info),
                lock: std::mem::transmute::<*mut c_void, Lock>(lock),
                unlock: std::mem::transmute::<*mut c_void, Unlock>(unlock),
            }
        })
    })
    .as_ref()
}

struct Locked<'a> {
    api: &'a BitmapApi,
    env: *mut JNIEnv,
    bitmap: jobject,
}

impl Drop for Locked<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.unlock)(self.env, self.bitmap) };
    }
}

pub(crate) fn paint(
    env: *mut JNIEnv,
    bitmap: jobject,
    draw: impl FnOnce(&mut PixmapMut<'_>),
) -> bool {
    let Some(api) = api() else { return false };
    if bitmap.is_null() {
        return false;
    }
    let mut info = BitmapInfo::default();
    let valid = unsafe { (api.info)(env, bitmap, &mut info) } == 0;
    if !valid || info.format != 1 || info.width == 0 || info.height == 0 {
        return false;
    }
    let stride = info.stride as usize;
    let row = info.width as usize * 4;
    let Some(length) = stride.checked_mul(info.height as usize) else {
        return false;
    };
    if stride < row || length > isize::MAX as usize {
        return false;
    }
    let mut address = std::ptr::null_mut();
    if unsafe { (api.lock)(env, bitmap, &mut address) } != 0 {
        return false;
    }
    let guard = Locked { api, env, bitmap };
    let Some(address) = NonNull::new(address) else {
        return false;
    };
    // The JVM owns this ARGB_8888 bitmap. The lock pins its mutable premultiplied
    // RGBA pixels exclusively until the guard is dropped.
    let data = unsafe { std::slice::from_raw_parts_mut(address.as_ptr().cast::<u8>(), length) };
    if stride == row {
        let Some(mut view) = PixmapMut::from_bytes(data, info.width, info.height) else {
            return false;
        };
        draw(&mut view);
    } else {
        let Some(mut surface) = Pixmap::new(info.width, info.height) else {
            return false;
        };
        draw(&mut surface.as_mut());
        for (destination, source) in data
            .chunks_exact_mut(stride)
            .zip(surface.data().chunks_exact(row))
        {
            destination[..row].copy_from_slice(source);
        }
    }
    drop(guard);
    true
}
