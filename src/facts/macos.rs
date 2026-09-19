//! macOS probes.

use super::Facts;
use std::ffi::CString;

/// Reads a sysctl value as raw bytes. Returns `None` on any failure so the
/// caller's fact is simply left absent.
fn sysctl_bytes(name: &str) -> Option<Vec<u8>> {
    let cname = CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    // SAFETY: `cname` is a valid, null terminated C string and the null
    // `oldp` asks sysctlbyname to report the required buffer size in `len`.
    let rc = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    // SAFETY: `buf` was just allocated with `len` bytes as reported above,
    // matching the buffer size sysctlbyname is told about via `&mut len`.
    let rc = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    buf.truncate(len);
    Some(buf)
}

/// Reads a sysctl string value, dropping a trailing NUL if present.
fn sysctl_string(name: &str) -> Option<String> {
    let mut bytes = sysctl_bytes(name)?;
    if bytes.last() == Some(&0) {
        bytes.pop();
    }
    String::from_utf8(bytes).ok()
}

/// Reads a sysctl 32 bit integer value.
fn sysctl_u32(name: &str) -> Option<u32> {
    let bytes = sysctl_bytes(name)?;
    let arr: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_ne_bytes(arr))
}

/// Reads a sysctl 64 bit integer value.
fn sysctl_u64(name: &str) -> Option<u64> {
    let bytes = sysctl_bytes(name)?;
    let arr: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
    Some(u64::from_ne_bytes(arr))
}

/// The machine's hostname with a trailing ".local" removed, if present.
fn host_name() -> Option<String> {
    let mut buf = vec![0u8; 256];
    // SAFETY: `buf` is a valid, writable buffer of its own length that we
    // own for the duration of the call.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let name = String::from_utf8(buf[..end].to_vec()).ok()?;
    Some(name.strip_suffix(".local").unwrap_or(&name).to_string())
}

/// Disk size, free space (in GB, rounded) and used percentage (rounded),
/// from `statvfs` on `$HOME`, falling back to `/`.
fn disk_facts() -> Option<(u64, u64, u32)> {
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());
    let path = home.unwrap_or_else(|| "/".to_string());
    // SAFETY: `statvfs` is a C struct of plain integer fields, so an all
    // zero bit pattern is a valid value.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let stat_at = |path: &str, stat: &mut libc::statvfs| -> Option<i32> {
        let cpath = CString::new(path).ok()?;
        // SAFETY: `cpath` is a valid, null terminated C string and `stat`
        // points at a valid, writable `statvfs` we own for the call.
        Some(unsafe { libc::statvfs(cpath.as_ptr(), stat) })
    };
    let rc = stat_at(&path, &mut stat)?;
    let rc = if rc == 0 {
        rc
    } else {
        stat_at("/", &mut stat)?
    };
    if rc != 0 {
        return None;
    }
    let size_bytes = stat.f_blocks as u128 * stat.f_frsize as u128;
    let free_bytes = stat.f_bavail as u128 * stat.f_frsize as u128;
    if size_bytes == 0 {
        return None;
    }
    let size_gb = (size_bytes as f64 / 1e9).round() as u64;
    let free_gb = (free_bytes as f64 / 1e9).round() as u64;
    let used_pct = (((size_bytes - free_bytes) as f64 / size_bytes as f64) * 100.0).round() as u32;
    Some((size_gb, free_gb, used_pct))
}

/// The parent process's name, leading `-` (login shell marker) stripped.
fn parent_shell_name(ppid: libc::pid_t) -> Option<String> {
    let mut buf = vec![0u8; 256];
    // SAFETY: `buf` is a valid, writable buffer of its own length that we
    // own for the duration of the call.
    let len = unsafe {
        libc::proc_name(
            ppid,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len() as u32,
        )
    };
    if len <= 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let name = String::from_utf8(buf[..end].to_vec()).ok()?;
    Some(name.strip_prefix('-').unwrap_or(&name).to_string())
}

/// Milliseconds since the parent process started, or `None` when it is
/// unavailable or exceeds the 60 second freshness window.
fn parent_boot_ms(ppid: libc::pid_t) -> Option<u64> {
    // SAFETY: `proc_bsdinfo` is a C struct of plain integer and array
    // fields, so an all zero bit pattern is a valid value.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` points at a valid, writable `proc_bsdinfo` sized
    // buffer, matching `size` passed to proc_pidinfo.
    let written = unsafe {
        libc::proc_pidinfo(
            ppid,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if written != size {
        return None;
    }
    let start_ms = info
        .pbi_start_tvsec
        .saturating_mul(1000)
        .saturating_add(info.pbi_start_tvusec / 1000);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let now_ms = now.as_millis() as u64;
    let elapsed = now_ms.saturating_sub(start_ms);
    if elapsed > 60_000 {
        return None;
    }
    Some(elapsed)
}

pub(crate) fn probe(facts: &mut Facts) {
    if let Some(name) = sysctl_string("machdep.cpu.brand_string") {
        facts.insert("cpu.name", name);
    }
    if let Some(cores) = sysctl_u32("hw.ncpu") {
        facts.insert("cpu.cores", cores.to_string());
    }
    if let Some(memsize) = sysctl_u64("hw.memsize") {
        facts.insert("mem.kb", (memsize / 1024).to_string());
    }

    facts.insert("os.name", "macOS");
    if let Some(version) = sysctl_string("kern.osproductversion") {
        facts.insert("os.version", version);
    }

    if let Some(host) = host_name() {
        facts.insert("host.name", host);
    }

    if let Some((size_gb, free_gb, used_pct)) = disk_facts() {
        facts.insert("disk.size_gb", size_gb.to_string());
        facts.insert("disk.free_gb", free_gb.to_string());
        facts.insert("disk.used_pct", used_pct.to_string());
    }

    // SAFETY: getppid takes no arguments and cannot fail.
    let ppid = unsafe { libc::getppid() };
    if let Some(shell) = parent_shell_name(ppid) {
        facts.insert("shell.name", shell);
    }
    if let Some(boot_ms) = parent_boot_ms(ppid) {
        facts.insert("shell.boot_ms", boot_ms.to_string());
    }
}
