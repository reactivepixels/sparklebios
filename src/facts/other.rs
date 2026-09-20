//! Linux probes. Every probe fails silently by leaving its key absent: an
//! unreadable file, an unexpected format, or a failing syscall is never
//! treated as an error, only as a fact we do not have. No probe here spawns
//! a process; everything comes from a file read or a libc call.
//!
//! The parsing is kept separate from the file reading throughout, so every
//! parser can be unit tested against a fixture string rather than against
//! whatever `/proc` or `/etc` happens to hold on the machine running the
//! tests.

use super::Facts;
use std::ffi::CString;

/// The trimmed value of the first `key: value` line in `text`, `/proc`
/// style (colon separated, arbitrary leading whitespace on either side).
fn field_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim() == key).then(|| v.trim())
    })
}

/// The CPU's name from a `/proc/cpuinfo` string: `model name` on most
/// machines, falling back to `Hardware` or `Model`, the fields ARM boards
/// use instead. `None`, rather than a guess, when none of the three appear.
fn parse_cpu_name(cpuinfo: &str) -> Option<String> {
    field_value(cpuinfo, "model name")
        .or_else(|| field_value(cpuinfo, "Hardware"))
        .or_else(|| field_value(cpuinfo, "Model"))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// The number of `processor` lines in a `/proc/cpuinfo` string, one per
/// logical core. `None` when there are none, so a real probe falls back to
/// `sysconf` instead of reporting zero cores.
fn parse_cpu_cores(cpuinfo: &str) -> Option<u32> {
    let cores = cpuinfo
        .lines()
        .filter(|line| {
            line.split_once(':')
                .is_some_and(|(k, _)| k.trim() == "processor")
        })
        .count();
    (cores > 0).then_some(cores as u32)
}

/// `MemTotal`, already in kB, from a `/proc/meminfo` string.
fn parse_mem_kb(meminfo: &str) -> Option<u64> {
    field_value(meminfo, "MemTotal")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// The OS name and version from an `/etc/os-release` string: `NAME` and
/// `VERSION_ID`, quotes stripped, falling back to splitting `PRETTY_NAME` on
/// its first space when `NAME` (and, for the version, `VERSION_ID`) is
/// missing.
fn parse_os_release(contents: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut version = None;
    let mut pretty = None;
    for line in contents.lines() {
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        let value = raw.trim().trim_matches('"');
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "NAME" => name = Some(value.to_string()),
            "VERSION_ID" => version = Some(value.to_string()),
            "PRETTY_NAME" => pretty = Some(value.to_string()),
            _ => {}
        }
    }
    if name.is_none() {
        if let Some(pretty) = &pretty {
            let mut parts = pretty.splitn(2, ' ');
            name = parts.next().map(str::to_string).filter(|s| !s.is_empty());
            if version.is_none() {
                version = parts
                    .next()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }
        }
    }
    (name, version)
}

/// The shell name from a `/proc/<pid>/comm` string: the trimmed line, with a
/// leading `-` (the login shell marker some kernels still carry through)
/// stripped, the same as `macos.rs` strips it from `proc_name`.
fn parse_comm(contents: &str) -> Option<String> {
    let name = contents.trim();
    (!name.is_empty()).then(|| name.strip_prefix('-').unwrap_or(name).to_string())
}

/// Field 22 (`starttime`, in clock ticks since boot) of a `/proc/<pid>/stat`
/// string. Parsed past the last `)`, since the `comm` field before it (field
/// 2) is parenthesised and can itself contain spaces or parentheses.
fn parse_starttime_ticks(stat: &str) -> Option<u64> {
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// The system uptime in seconds, the first field of a `/proc/uptime` string.
fn parse_uptime_secs(uptime: &str) -> Option<f64> {
    uptime.split_whitespace().next()?.parse().ok()
}

/// Milliseconds since a process with the given `starttime` (in clock ticks)
/// started, given the clock's ticks per second and the system's current
/// uptime in seconds. `None` when the ticks per second is unknown, the
/// arithmetic would go negative (a stale or bogus read), or the result
/// exceeds the 10 second freshness window `macos.rs` also applies.
fn boot_ms_from_parts(starttime_ticks: u64, clk_tck: u64, uptime_secs: f64) -> Option<u64> {
    if clk_tck == 0 {
        return None;
    }
    let start_secs = starttime_ticks as f64 / clk_tck as f64;
    let elapsed_secs = uptime_secs - start_secs;
    if elapsed_secs < 0.0 {
        return None;
    }
    let elapsed_ms = (elapsed_secs * 1000.0).round() as u64;
    (elapsed_ms <= 10_000).then_some(elapsed_ms)
}

/// The number of online CPUs from `sysconf`, used when `/proc/cpuinfo` is
/// unreadable or has no `processor` lines to count.
fn cpu_cores_via_sysconf() -> Option<u32> {
    // SAFETY: `sysconf` takes a fixed constant and only reads kernel state.
    let n = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    (n > 0).then_some(n as u32)
}

/// The clock's ticks per second, needed to turn `/proc/<pid>/stat`'s
/// `starttime` field into seconds.
fn clk_tck() -> Option<u64> {
    // SAFETY: `sysconf` takes a fixed constant and only reads kernel state.
    let n = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    (n > 0).then_some(n as u64)
}

/// The machine's hostname, via `gethostname`.
fn host_name() -> Option<String> {
    let mut buf = vec![0u8; 256];
    // SAFETY: `buf` is a valid, writable buffer of its own length that we
    // own for the duration of the call.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8(buf[..end].to_vec()).ok()
}

/// The kernel name and release from `uname`, used only when `/etc/os-release`
/// itself cannot be read: not a distro name, but better than nothing.
fn uname_name_release() -> Option<(String, String)> {
    // SAFETY: `utsname` is a C struct of plain byte arrays, so an all zero
    // bit pattern is a valid value.
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    // SAFETY: `uts` points at a valid, writable `utsname` we own for the
    // call.
    let rc = unsafe { libc::uname(&mut uts) };
    if rc != 0 {
        return None;
    }
    let sysname = c_char_string(&uts.sysname)?;
    let release = c_char_string(&uts.release)?;
    Some((sysname, release))
}

/// A NUL terminated `c_char` array (as `utsname`'s fields are) as a `String`.
fn c_char_string(buf: &[libc::c_char]) -> Option<String> {
    // `c_char` is `i8` on some targets and `u8` on others (aarch64 Linux),
    // so this cast is a real conversion on one and a no-op on the other.
    #[allow(clippy::unnecessary_cast)]
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    (!bytes.is_empty())
        .then(|| String::from_utf8(bytes).ok())
        .flatten()
}

/// Disk size, free space (in GB, rounded) and used percentage (rounded),
/// from `statvfs` on `$HOME`, falling back to `/`. Mirrors `macos.rs`'s
/// `disk_facts` exactly: same source, same rounding.
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

pub(crate) fn probe(facts: &mut Facts) {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok();
    if let Some(name) = cpuinfo.as_deref().and_then(parse_cpu_name) {
        facts.insert("cpu.name", name);
    }
    let cores = cpuinfo
        .as_deref()
        .and_then(parse_cpu_cores)
        .or_else(cpu_cores_via_sysconf);
    if let Some(cores) = cores {
        facts.insert("cpu.cores", cores.to_string());
    }

    if let Some(kb) = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .as_deref()
        .and_then(parse_mem_kb)
    {
        facts.insert("mem.kb", kb.to_string());
    }

    if let Some((size_gb, free_gb, used_pct)) = disk_facts() {
        facts.insert("disk.size_gb", size_gb.to_string());
        facts.insert("disk.free_gb", free_gb.to_string());
        facts.insert("disk.used_pct", used_pct.to_string());
    }

    match std::fs::read_to_string("/etc/os-release") {
        Ok(os_release) => {
            let (name, version) = parse_os_release(&os_release);
            if let Some(name) = name {
                facts.insert("os.name", name);
            }
            if let Some(version) = version {
                facts.insert("os.version", version);
            }
        }
        Err(_) => {
            if let Some((sysname, release)) = uname_name_release() {
                facts.insert("os.name", sysname);
                facts.insert("os.version", release);
            }
        }
    }

    if let Some(host) = host_name() {
        facts.insert("host.name", host);
    }

    // SAFETY: getppid takes no arguments and cannot fail.
    let ppid = unsafe { libc::getppid() };

    if let Some(name) = std::fs::read_to_string(format!("/proc/{ppid}/comm"))
        .ok()
        .as_deref()
        .and_then(parse_comm)
    {
        facts.insert("shell.name", name);
    }

    let boot_ms = (|| {
        let stat = std::fs::read_to_string(format!("/proc/{ppid}/stat")).ok()?;
        let uptime = std::fs::read_to_string("/proc/uptime").ok()?;
        let starttime = parse_starttime_ticks(&stat)?;
        let uptime_secs = parse_uptime_secs(&uptime)?;
        let tck = clk_tck()?;
        boot_ms_from_parts(starttime, tck, uptime_secs)
    })();
    if let Some(boot_ms) = boot_ms {
        facts.insert("shell.boot_ms", boot_ms.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real `/proc/cpuinfo` excerpt (x86_64, two logical cores shown).
    const CPUINFO_X86: &str = "\
processor\t: 0
vendor_id\t: GenuineIntel
cpu family\t: 6
model\t\t: 158
model name\t: Intel(R) Core(TM) i7-9700K CPU @ 3.60GHz
stepping\t: 10
microcode\t: 0xf0
cpu MHz\t\t: 3600.000
cache size\t: 12288 KB
physical id\t: 0
siblings\t: 1
core id\t\t: 0
cpu cores\t: 1

processor\t: 1
vendor_id\t: GenuineIntel
model name\t: Intel(R) Core(TM) i7-9700K CPU @ 3.60GHz
physical id\t: 0
siblings\t: 1
core id\t\t: 1
cpu cores\t: 1
";

    // A real `/proc/cpuinfo` excerpt from an ARM board with no `model name`.
    const CPUINFO_ARM: &str = "\
processor\t: 0
BogoMIPS\t: 108.00
Features\t: fp asimd evtstrm aes pmull sha1 sha2 crc32 cpuid
CPU implementer\t: 0x41
CPU architecture: 8
CPU variant\t: 0x0
CPU part\t: 0xd08
CPU revision\t: 3

Hardware\t: BCM2711
Revision\t: c03112
Serial\t\t: 0000000012345678
Model\t\t: Raspberry Pi 4 Model B Rev 1.2
";

    const MEMINFO: &str = "\
MemTotal:       32874356 kB
MemFree:         1234567 kB
MemAvailable:   20000000 kB
Buffers:          123456 kB
Cached:          7654321 kB
";

    const OS_RELEASE_UBUNTU: &str = "\
PRETTY_NAME=\"Ubuntu 22.04.3 LTS\"
NAME=\"Ubuntu\"
VERSION_ID=\"22.04\"
VERSION=\"22.04.3 LTS (Jammy Jellyfish)\"
ID=ubuntu
ID_LIKE=debian
";

    // A real `/etc/os-release` excerpt from a distro with no `NAME`/`VERSION_ID`.
    const OS_RELEASE_NO_NAME: &str = "\
PRETTY_NAME=\"Alpine Linux v3.19\"
ID=alpine
";

    #[test]
    fn parse_cpu_name_reads_model_name() {
        assert_eq!(
            parse_cpu_name(CPUINFO_X86),
            Some("Intel(R) Core(TM) i7-9700K CPU @ 3.60GHz".to_string())
        );
    }

    #[test]
    fn parse_cpu_name_falls_back_to_hardware_or_model_on_arm() {
        assert_eq!(
            parse_cpu_name(CPUINFO_ARM),
            Some("BCM2711".to_string()),
            "Hardware should win when present"
        );
        let model_only = CPUINFO_ARM.replace("Hardware\t: BCM2711\n", "");
        assert_eq!(
            parse_cpu_name(&model_only),
            Some("Raspberry Pi 4 Model B Rev 1.2".to_string())
        );
    }

    #[test]
    fn parse_cpu_name_is_none_without_any_matching_field() {
        assert_eq!(parse_cpu_name("vendor_id\t: GenuineIntel\n"), None);
    }

    #[test]
    fn parse_cpu_cores_counts_processor_lines() {
        assert_eq!(parse_cpu_cores(CPUINFO_X86), Some(2));
    }

    #[test]
    fn parse_cpu_cores_is_none_when_there_are_no_processor_lines() {
        assert_eq!(parse_cpu_cores("vendor_id\t: GenuineIntel\n"), None);
    }

    #[test]
    fn parse_mem_kb_reads_memtotal() {
        assert_eq!(parse_mem_kb(MEMINFO), Some(32_874_356));
    }

    #[test]
    fn parse_mem_kb_is_none_without_memtotal() {
        assert_eq!(parse_mem_kb("MemFree: 1234 kB\n"), None);
    }

    #[test]
    fn parse_os_release_reads_name_and_version_id() {
        assert_eq!(
            parse_os_release(OS_RELEASE_UBUNTU),
            (Some("Ubuntu".to_string()), Some("22.04".to_string()))
        );
    }

    #[test]
    fn parse_os_release_falls_back_to_pretty_name() {
        assert_eq!(
            parse_os_release(OS_RELEASE_NO_NAME),
            (Some("Alpine".to_string()), Some("Linux v3.19".to_string()))
        );
    }

    #[test]
    fn parse_os_release_is_empty_without_any_usable_line() {
        assert_eq!(parse_os_release("ID=custom\n"), (None, None));
    }

    #[test]
    fn parse_comm_strips_a_leading_dash_and_trailing_newline() {
        assert_eq!(parse_comm("zsh\n"), Some("zsh".to_string()));
        assert_eq!(parse_comm("-zsh\n"), Some("zsh".to_string()));
    }

    #[test]
    fn parse_comm_is_none_for_an_empty_read() {
        assert_eq!(parse_comm(""), None);
        assert_eq!(parse_comm("\n"), None);
    }

    // A real `/proc/<pid>/stat` line, comm field containing a space and a
    // closing paren to exercise the "split past the last `)`" parsing.
    const STAT_LINE: &str = "4242 (some (shell)) S 4200 4242 4242 34816 4242 4194560 1234 0 0 0 5 3 0 0 20 0 1 0 987654 123456789 1234 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0\n";

    #[test]
    fn parse_starttime_ticks_reads_field_22_past_the_last_paren() {
        assert_eq!(parse_starttime_ticks(STAT_LINE), Some(987_654));
    }

    #[test]
    fn parse_starttime_ticks_is_none_without_a_closing_paren() {
        assert_eq!(parse_starttime_ticks("no parens here"), None);
    }

    #[test]
    fn parse_uptime_secs_reads_the_first_field() {
        assert_eq!(parse_uptime_secs("123456.78 654321.00\n"), Some(123_456.78));
    }

    #[test]
    fn parse_uptime_secs_is_none_when_empty() {
        assert_eq!(parse_uptime_secs(""), None);
    }

    #[test]
    fn boot_ms_from_parts_computes_elapsed_time_since_start() {
        // Ticks are 100 per second (the usual Linux HZ); the shell started
        // 418 ms before the uptime snapshot.
        let starttime_ticks = 987_654;
        let uptime_secs = 9_876.958;
        assert_eq!(
            boot_ms_from_parts(starttime_ticks, 100, uptime_secs),
            Some(418)
        );
    }

    #[test]
    fn boot_ms_from_parts_is_none_past_the_freshness_window() {
        assert_eq!(boot_ms_from_parts(0, 100, 11.0), None);
    }

    #[test]
    fn boot_ms_from_parts_is_none_with_an_unknown_clock_rate() {
        assert_eq!(boot_ms_from_parts(100, 0, 1.0), None);
    }

    #[test]
    fn boot_ms_from_parts_is_none_when_the_arithmetic_would_go_negative() {
        // A starttime after the uptime snapshot: a stale or inconsistent read.
        assert_eq!(boot_ms_from_parts(100_000, 100, 1.0), None);
    }
}
