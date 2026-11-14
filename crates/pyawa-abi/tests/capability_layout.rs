//! **自动生成，勿手改**（`tools/gen_pa_capabilities.py`）—— §12.17 A3 的 Rust 侧布局探针。
//!
//! 打印 `layout|C 结构名|字段|offset|size`（`@size` 行给整结构尺寸），供
//! `tests/ci/check_capability_layout.py` 与 C 探针逐行对照。只打印、不 assert。

#![allow(dead_code)]

use core::ffi::c_void;
use pyawa_capabilities::{
    clock::{CpTm, CpClockVtable},
    fs::{CapStatus, CpFsVtable, FileInfo, FileTimes, Handle},
    net::{CpNetVtable, PollFd, SocketHandle},
};

#[test]
fn print_layout() {
    println!("layout|pa_fs_vtable|@size|0|{}", core::mem::size_of::<CpFsVtable>());
    println!("layout|pa_fs_vtable|version|{}|{}", core::mem::offset_of!(CpFsVtable, version), core::mem::size_of::<u32>());
    println!("layout|pa_fs_vtable|size|{}|{}", core::mem::offset_of!(CpFsVtable, size), core::mem::size_of::<u32>());
    println!("layout|pa_fs_vtable|state|{}|{}", core::mem::offset_of!(CpFsVtable, state), core::mem::size_of::<*mut c_void>());
    println!("layout|pa_fs_vtable|open|{}|{}", core::mem::offset_of!(CpFsVtable, open), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const u8, usize, i32, u32, *mut Handle, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|stat|{}|{}", core::mem::offset_of!(CpFsVtable, stat), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const u8, usize, *mut FileInfo, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|fstat|{}|{}", core::mem::offset_of!(CpFsVtable, fstat), core::mem::size_of::<Option<extern "C" fn(*mut c_void, Handle, *mut FileInfo, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|listdir|{}|{}", core::mem::offset_of!(CpFsVtable, listdir), core::mem::size_of::<Option< extern "C" fn(*mut c_void, Handle, *mut u8, usize, *mut usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|mkdir|{}|{}", core::mem::offset_of!(CpFsVtable, mkdir), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const u8, usize, u32, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|rmdir|{}|{}", core::mem::offset_of!(CpFsVtable, rmdir), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const u8, usize, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|unlink|{}|{}", core::mem::offset_of!(CpFsVtable, unlink), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const u8, usize, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|rename|{}|{}", core::mem::offset_of!(CpFsVtable, rename), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|readlink|{}|{}", core::mem::offset_of!(CpFsVtable, readlink), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, *mut u8, usize, *mut usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|symlink|{}|{}", core::mem::offset_of!(CpFsVtable, symlink), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|chmod|{}|{}", core::mem::offset_of!(CpFsVtable, chmod), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const u8, usize, u32, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|utime|{}|{}", core::mem::offset_of!(CpFsVtable, utime), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, i64, i64, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|read|{}|{}", core::mem::offset_of!(CpFsVtable, read), core::mem::size_of::<Option< extern "C" fn(*mut c_void, Handle, *mut u8, usize, usize, *mut usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|write|{}|{}", core::mem::offset_of!(CpFsVtable, write), core::mem::size_of::<Option< extern "C" fn(*mut c_void, Handle, *const u8, usize, *mut usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|seek|{}|{}", core::mem::offset_of!(CpFsVtable, seek), core::mem::size_of::<Option<extern "C" fn(*mut c_void, Handle, i64, i32, *mut u64, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|tell|{}|{}", core::mem::offset_of!(CpFsVtable, tell), core::mem::size_of::<Option<extern "C" fn(*mut c_void, Handle, *mut u64, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|truncate|{}|{}", core::mem::offset_of!(CpFsVtable, truncate), core::mem::size_of::<Option<extern "C" fn(*mut c_void, Handle, u64, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|fsync|{}|{}", core::mem::offset_of!(CpFsVtable, fsync), core::mem::size_of::<Option<extern "C" fn(*mut c_void, Handle, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|close|{}|{}", core::mem::offset_of!(CpFsVtable, close), core::mem::size_of::<Option<extern "C" fn(*mut c_void, Handle, *mut i32) -> CapStatus>>());
    println!("layout|pa_fs_vtable|stat_times|{}|{}", core::mem::offset_of!(CpFsVtable, stat_times), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, *mut FileTimes, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|pipe|{}|{}", core::mem::offset_of!(CpFsVtable, pipe), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *mut Handle, *mut Handle, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|lutime|{}|{}", core::mem::offset_of!(CpFsVtable, lutime), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, i64, i64, *mut i32) -> CapStatus, >>());
    println!("layout|pa_fs_vtable|utime_ns|{}|{}", core::mem::offset_of!(CpFsVtable, utime_ns), core::mem::size_of::<Option< extern "C" fn(*mut c_void, *const u8, usize, i64, i64, *mut i32) -> CapStatus, >>());
    println!("layout|pa_clock_vtable|@size|0|{}", core::mem::size_of::<CpClockVtable>());
    println!("layout|pa_clock_vtable|version|{}|{}", core::mem::offset_of!(CpClockVtable, version), core::mem::size_of::<u32>());
    println!("layout|pa_clock_vtable|size|{}|{}", core::mem::offset_of!(CpClockVtable, size), core::mem::size_of::<u32>());
    println!("layout|pa_clock_vtable|state|{}|{}", core::mem::offset_of!(CpClockVtable, state), core::mem::size_of::<*mut c_void>());
    println!("layout|pa_clock_vtable|now_ns|{}|{}", core::mem::offset_of!(CpClockVtable, now_ns), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *mut i64) -> CapStatus>>());
    println!("layout|pa_clock_vtable|monotonic_ns|{}|{}", core::mem::offset_of!(CpClockVtable, monotonic_ns), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *mut i64) -> CapStatus>>());
    println!("layout|pa_clock_vtable|sleep_ns|{}|{}", core::mem::offset_of!(CpClockVtable, sleep_ns), core::mem::size_of::<Option<extern "C" fn(*mut c_void, i64, *mut i32) -> CapStatus>>());
    println!("layout|pa_clock_vtable|localtime|{}|{}", core::mem::offset_of!(CpClockVtable, localtime), core::mem::size_of::<Option<extern "C" fn(*mut c_void, i64, *mut CpTm, *mut i32) -> CapStatus>>());
    println!("layout|pa_clock_vtable|strftime|{}|{}", core::mem::offset_of!(CpClockVtable, strftime), core::mem::size_of::<Option< extern "C" fn( *mut c_void, *const u8, usize, *const CpTm, *mut u8, usize, *mut usize, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_clock_vtable|mktime|{}|{}", core::mem::offset_of!(CpClockVtable, mktime), core::mem::size_of::<Option<extern "C" fn(*mut c_void, *const CpTm, *mut i64, *mut i32) -> CapStatus>>());
    println!("layout|pa_net_vtable|@size|0|{}", core::mem::size_of::<CpNetVtable>());
    println!("layout|pa_net_vtable|version|{}|{}", core::mem::offset_of!(CpNetVtable, version), core::mem::size_of::<u32>());
    println!("layout|pa_net_vtable|size|{}|{}", core::mem::offset_of!(CpNetVtable, size), core::mem::size_of::<u32>());
    println!("layout|pa_net_vtable|state|{}|{}", core::mem::offset_of!(CpNetVtable, state), core::mem::size_of::<*mut c_void>());
    println!("layout|pa_net_vtable|socket|{}|{}", core::mem::offset_of!(CpNetVtable, socket), core::mem::size_of::<Option< extern "C" fn(*mut c_void, i32, i32, i32, *mut SocketHandle, *mut i32) -> CapStatus, >>());
    println!("layout|pa_net_vtable|connect|{}|{}", core::mem::offset_of!(CpNetVtable, connect), core::mem::size_of::<Option< extern "C" fn(*mut c_void, SocketHandle, i32, *const u8, usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_net_vtable|bind|{}|{}", core::mem::offset_of!(CpNetVtable, bind), core::mem::size_of::<Option< extern "C" fn(*mut c_void, SocketHandle, i32, *const u8, usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_net_vtable|listen|{}|{}", core::mem::offset_of!(CpNetVtable, listen), core::mem::size_of::<Option<extern "C" fn(*mut c_void, SocketHandle, i32, *mut i32) -> CapStatus>>());
    println!("layout|pa_net_vtable|accept|{}|{}", core::mem::offset_of!(CpNetVtable, accept), core::mem::size_of::<Option< extern "C" fn( *mut c_void, SocketHandle, *mut SocketHandle, *mut i32, *mut u8, usize, *mut usize, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_net_vtable|send|{}|{}", core::mem::offset_of!(CpNetVtable, send), core::mem::size_of::<Option< extern "C" fn(*mut c_void, SocketHandle, *const u8, usize, *mut usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_net_vtable|recv|{}|{}", core::mem::offset_of!(CpNetVtable, recv), core::mem::size_of::<Option< extern "C" fn(*mut c_void, SocketHandle, *mut u8, usize, *mut usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_net_vtable|shutdown|{}|{}", core::mem::offset_of!(CpNetVtable, shutdown), core::mem::size_of::<Option<extern "C" fn(*mut c_void, SocketHandle, i32, *mut i32) -> CapStatus>>());
    println!("layout|pa_net_vtable|getsockname|{}|{}", core::mem::offset_of!(CpNetVtable, getsockname), core::mem::size_of::<Option< extern "C" fn( *mut c_void, SocketHandle, *mut i32, *mut u8, usize, *mut usize, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_net_vtable|getpeername|{}|{}", core::mem::offset_of!(CpNetVtable, getpeername), core::mem::size_of::<Option< extern "C" fn( *mut c_void, SocketHandle, *mut i32, *mut u8, usize, *mut usize, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_net_vtable|getsockopt|{}|{}", core::mem::offset_of!(CpNetVtable, getsockopt), core::mem::size_of::<Option< extern "C" fn( *mut c_void, SocketHandle, i32, i32, *mut u8, usize, *mut usize, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_net_vtable|setsockopt|{}|{}", core::mem::offset_of!(CpNetVtable, setsockopt), core::mem::size_of::<Option< extern "C" fn(*mut c_void, SocketHandle, i32, i32, *const u8, usize, *mut i32) -> CapStatus, >>());
    println!("layout|pa_net_vtable|poll|{}|{}", core::mem::offset_of!(CpNetVtable, poll), core::mem::size_of::<Option< extern "C" fn( *mut c_void, *const PollFd, usize, i64, *mut PollFd, usize, *mut usize, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_net_vtable|socketpair|{}|{}", core::mem::offset_of!(CpNetVtable, socketpair), core::mem::size_of::<Option< extern "C" fn( *mut c_void, i32, i32, i32, *mut SocketHandle, *mut SocketHandle, *mut i32, ) -> CapStatus, >>());
    println!("layout|pa_net_vtable|close|{}|{}", core::mem::offset_of!(CpNetVtable, close), core::mem::size_of::<Option<extern "C" fn(*mut c_void, SocketHandle, *mut i32) -> CapStatus>>());
}
