use std::alloc::System;

use phala_allocator::StatSizeAllocator;
use wapod_rpc::prpc::MemoryUsage;

#[global_allocator]
static ALLOCATOR: StatSizeAllocator<System> = StatSizeAllocator::new(System);

pub fn mem_usage() -> MemoryUsage {
    let stats = ALLOCATOR.stats();
    MemoryUsage {
        rust_used: stats.current as _,
        rust_peak: stats.peak as _,
        rust_spike: stats.spike as _,
        peak: vm_peak().unwrap_or(0) as _,
        free: mem_free().unwrap_or(0) as _,
    }
}

fn vm_peak() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("VmPeak:") {
            let peak = line.split_ascii_whitespace().nth(1)?;
            return peak.parse().ok();
        }
    }
    None
}

fn mem_free() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in status.lines() {
        if line.starts_with("MemFree:") {
            let peak = line.split_ascii_whitespace().nth(1)?;
            return peak.parse().ok();
        }
    }
    None
}
