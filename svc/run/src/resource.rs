use std::{io, path::Path, process::Command};

use lao_run_api::{Budget, Mode};

use crate::BUILD;

const GIB: u64 = 1 << 30;

struct Pool {
    total: u64,
    available: u64,
    ceiling: u64,
}

pub fn plan(bin: &Path, mode: Mode) -> io::Result<Budget> {
    let mut pool = probe(bin)?;
    if pressured()? {
        pool.available = 0;
    }
    Ok(resolve(
        mode,
        pool,
        std::thread::available_parallelism()?.get(),
    ))
}

pub fn pressured() -> io::Result<bool> {
    Ok(pressure()? != 1)
}

fn resolve(mode: Mode, pool: Pool, cpus: usize) -> Budget {
    let (fraction, reserve, cpu, context) = match mode {
        Mode::Light => (25, (8 * GIB).max(percent(pool.total, 35)), 25, 32_768),
        Mode::Auto => (45, (8 * GIB).max(percent(pool.total, 30)), 50, 65_536),
        Mode::Maximum => (70, (6 * GIB).max(percent(pool.total, 15)), 90, 131_072),
    };
    let bytes = percent(pool.total, fraction)
        .min(pool.available.saturating_sub(reserve))
        .min(pool.ceiling);
    let mut threads = ((cpus * cpu) / 100).max(1);
    if mode == Mode::Light {
        threads = threads.min(4);
    }
    Budget {
        bytes,
        threads: threads.min(u16::MAX as usize) as u16,
        target_context: context,
    }
}

fn percent(value: u64, fraction: u64) -> u64 {
    ((value as u128 * fraction as u128) / 100).min(u64::MAX as u128) as u64
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn probe(bin: &Path) -> io::Result<Pool> {
    let version = Command::new(bin).arg("--version").output()?;
    let observed = [version.stdout, version.stderr].concat();
    if !version.status.success() || !String::from_utf8_lossy(&observed).contains(BUILD) {
        return Err(invalid("build"));
    }
    let devices = Command::new(bin).arg("--list-devices").output()?;
    if !devices.status.success() {
        return Err(invalid("metal"));
    }
    let observed = String::from_utf8([devices.stdout, devices.stderr].concat())
        .map_err(|_| invalid("metal"))?;
    let (metal, free) = metal(&observed)?;
    let total = mac::total()?;
    Ok(Pool {
        total,
        available: percent(total, u64::from(mac::level()?)),
        ceiling: metal.min(free),
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn probe(_: &Path) -> io::Result<Pool> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "platform"))
}

#[cfg(target_os = "macos")]
fn pressure() -> io::Result<u32> {
    mac::u32("kern.memorystatus_vm_pressure_level")
}

#[cfg(not(target_os = "macos"))]
fn pressure() -> io::Result<u32> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "platform"))
}

fn metal(output: &str) -> io::Result<(u64, u64)> {
    let line = output
        .lines()
        .find(|line| line.trim_start().starts_with("MTL0:"))
        .ok_or_else(|| invalid("metal"))?;
    let (_, values) = line.rsplit_once('(').ok_or_else(|| invalid("metal"))?;
    let values = values.strip_suffix(')').ok_or_else(|| invalid("metal"))?;
    let (total, free) = values
        .split_once(" MiB, ")
        .ok_or_else(|| invalid("metal"))?;
    let free = free
        .strip_suffix(" MiB free")
        .ok_or_else(|| invalid("metal"))?;
    let total = total.parse::<u64>().map_err(|_| invalid("metal"))? << 20;
    let free = free.parse::<u64>().map_err(|_| invalid("metal"))? << 20;
    if total == 0 || free > total {
        return Err(invalid("metal"));
    }
    Ok((total, free))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(target_os = "macos")]
mod mac {
    use std::{io, process::Command};

    pub fn total() -> io::Result<u64> {
        value("hw.memsize").and_then(|value| {
            (value != 0)
                .then_some(value)
                .ok_or_else(|| io::Error::other("memory"))
        })
    }

    pub fn level() -> io::Result<u32> {
        u32("kern.memorystatus_level").and_then(|value| {
            (value <= 100)
                .then_some(value)
                .ok_or_else(|| io::Error::other("memory"))
        })
    }

    pub fn u32(name: &str) -> io::Result<u32> {
        value(name)
    }

    fn value<T: std::str::FromStr>(name: &str) -> io::Result<T> {
        let output = Command::new("/usr/sbin/sysctl")
            .args(["-n", name])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("memory"));
        }
        String::from_utf8(output.stdout)
            .map_err(|_| io::Error::other("memory"))?
            .trim()
            .parse()
            .map_err(|_| io::Error::other("memory"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_keep_headroom() {
        for (total, mode, expected) in [
            (16, Mode::Light, percent(16 * GIB, 25)),
            (16, Mode::Auto, percent(16 * GIB, 45)),
            (16, Mode::Maximum, 10 * GIB),
            (24, Mode::Light, percent(24 * GIB, 25)),
            (24, Mode::Auto, percent(24 * GIB, 45)),
            (24, Mode::Maximum, percent(24 * GIB, 70)),
        ] {
            let budget = resolve(
                mode,
                Pool {
                    total: total * GIB,
                    available: total * GIB,
                    ceiling: u64::MAX,
                },
                10,
            );
            assert_eq!(budget.bytes, expected);
        }
        assert_eq!(
            resolve(
                Mode::Auto,
                Pool {
                    total: 24 * GIB,
                    available: 4 * GIB,
                    ceiling: u64::MAX,
                },
                10,
            )
            .bytes,
            0
        );
        assert_eq!(
            resolve(
                Mode::Maximum,
                Pool {
                    total: 24 * GIB,
                    available: 24 * GIB,
                    ceiling: 16 * GIB - (1 << 20),
                },
                10,
            )
            .bytes,
            16 * GIB - (1 << 20)
        );
    }

    #[test]
    fn pinned_metal_shape_is_strict() {
        assert_eq!(
            metal("Available devices:\n  MTL0: Apple M4 (16384 MiB, 16383 MiB free)\n").unwrap(),
            (16 * GIB, 16 * GIB - (1 << 20))
        );
        assert!(metal("MTL0: Apple M4 (broken)").is_err());
    }
}
