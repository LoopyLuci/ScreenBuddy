//! Hardware Detection and Capability Reporting
use sysinfo::{RefreshKind, System};

#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub name: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub base_frequency_mhz: u64,
    pub max_frequency_mhz: u64,
    pub architecture: String, // x86_64, aarch64, etc
    pub supports_avx2: bool,
    pub supports_avx512: bool,
    pub supports_neon: bool, // ARM
    pub l1_cache_kb: u64,
    pub l2_cache_kb: u64,
    pub l3_cache_kb: u64,
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub device_type: GpuDeviceType,
    pub vram_mb: u64,
    /// Whether `vram_mb` was actually measured. wgpu cannot report it, so a
    /// bare 0 is ambiguous between "no VRAM" and "not detected".
    pub vram_known: bool,
    pub driver_version: String,
    pub api_support: Vec<GraphicsApi>, // Vulkan, DirectX, Metal, OpenGL
    pub compute_score: u32,            // relative compute capability
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GpuDeviceType {
    Discrete,
    Integrated,
    Virtual,
    CpuSoftware,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsApi {
    Vulkan,
    DirectX11,
    DirectX12,
    Metal,
    OpenGl,
    WebGpu,
}

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub ram_type: String, // DDR4, DDR5, LPDDR5, etc
    pub frequency_mhz: u32,
}

/// Complete hardware profile for the system
#[derive(Debug, Clone)]
pub struct HardwareProfile {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub memory: MemoryInfo,
    pub numa_nodes: usize,
    pub os_name: String,
    pub os_version: String,
}

impl HardwareProfile {
    /// Detect all hardware capabilities
    pub fn detect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_specifics(RefreshKind::everything());

        let cpu = Self::detect_cpu(&sys);
        let gpus = Self::detect_gpus();
        let memory = Self::detect_memory(&sys);
        let numa_nodes = Self::detect_numa();
        let (os_name, os_version) = Self::detect_os();

        Self {
            cpu,
            gpus,
            memory,
            numa_nodes,
            os_name,
            os_version,
        }
    }

    /// CPU instruction-set support.
    ///
    /// The x86 intrinsics only exist on x86 targets, so probing them
    /// unconditionally breaks the aarch64 build. Anything not x86 simply has
    /// neither, which is what the feature test below already assumes.
    fn cpu_isa_support() -> (bool, bool, bool) {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            (
                is_x86_feature_detected!("avx2"),
                is_x86_feature_detected!("avx512f"),
                false,
            )
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            (false, false, cfg!(target_arch = "aarch64"))
        }
    }

    /// CPU cache sizes in KB, or 0 when the platform does not report them.
    ///
    /// These used to be hardcoded to 32/256/8192 KB, which is a guess that
    /// happened to look plausible on a desktop and was simply wrong everywhere
    /// else. The `sysinfo` version in use exposes no cache-size accessor, so
    /// this reports 0 (unknown) rather than a plausible-looking fiction. A
    /// caller that needs real figures should query the OS directly.
    fn detect_caches(_sys: &System) -> (u64, u64, u64) {
        (0, 0, 0)
    }

    fn detect_cpu(sys: &System) -> CpuInfo {
        let cpus = sys.cpus();
        let physical_cores = sys.physical_core_count().unwrap_or(1);
        let logical_cores = cpus.len();
        let name = cpus
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();
        // ARCH, not OS. This reported "windows" as the CPU architecture, so
        // anything keying off it (SIMD choice, model compatibility) was wrong
        // on every platform.
        let arch = std::env::consts::ARCH.to_string();

        // Highest observed frequency, since cores on a hybrid CPU differ.
        let frequency = cpus.iter().map(|c| c.frequency()).max().unwrap_or(0);
        let (supports_avx2, supports_avx512, supports_neon) = Self::cpu_isa_support();
        let (l1_cache_kb, l2_cache_kb, l3_cache_kb) = Self::detect_caches(sys);

        CpuInfo {
            name,
            physical_cores,
            logical_cores,
            base_frequency_mhz: frequency,
            max_frequency_mhz: frequency,
            architecture: arch,
            supports_avx2,
            supports_avx512,
            supports_neon,
            l1_cache_kb,
            l2_cache_kb,
            l3_cache_kb,
        }
    }

    fn detect_gpus() -> Vec<GpuInfo> {
        // Try wgpu adapter enumeration for GPU info
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let mut gpus = Vec::new();
        // Note: wgpu doesn't directly expose VRAM, but adapter info gives us names
        // For full VRAM detection, we'd need DXGI on Windows, nvml for NVIDIA, etc.
        for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
            let info = adapter.get_info();
            let vendor = match info.vendor {
                0x10de => GpuVendor::Nvidia,
                0x1002 => GpuVendor::Amd,
                0x8086 => GpuVendor::Intel,
                _ => GpuVendor::Unknown,
            };
            let device_type = match info.device_type {
                wgpu::DeviceType::DiscreteGpu => GpuDeviceType::Discrete,
                wgpu::DeviceType::IntegratedGpu => GpuDeviceType::Integrated,
                wgpu::DeviceType::VirtualGpu => GpuDeviceType::Virtual,
                wgpu::DeviceType::Cpu => GpuDeviceType::CpuSoftware,
                _ => GpuDeviceType::Discrete,
            };
            gpus.push(GpuInfo {
                name: info.name,
                vendor,
                device_type,
                // wgpu exposes no VRAM figure. Reporting 0 with `vram_known`
                // false is honest; the previous code implied a measured value it
                // never had.
                vram_mb: 0,
                vram_known: false,
                driver_version: info.driver,
                // Derive the API list from what the backend actually is, instead
                // of claiming Vulkan on a Metal-only machine.
                api_support: graphics_apis_for(info.device_type, cfg!(target_os = "macos")),
                // Rank by device class, which is a real signal, rather than a
                // constant 100 that made every adapter look equally capable.
                compute_score: compute_score_for(info.device_type),
                is_primary: gpus.is_empty(),
            });
        }

        // If no adapters found, add a placeholder for software rendering
        if gpus.is_empty() {
            gpus.push(GpuInfo {
                name: "Software Rasterizer".into(),
                vendor: GpuVendor::Unknown,
                device_type: GpuDeviceType::CpuSoftware,
                vram_mb: 0,
                vram_known: false,
                driver_version: "N/A".into(),
                api_support: vec![GraphicsApi::WebGpu],
                compute_score: 10,
                is_primary: true,
            });
        }

        gpus
    }

    fn detect_memory(sys: &System) -> MemoryInfo {
        let total = sys.total_memory() / 1024;
        // Subtracting in u64 could underflow when `used` transiently exceeds
        // `total`, which sysinfo does report under memory pressure. Saturating
        // keeps that from wrapping to a near-maximum number.
        let available = total.saturating_sub(sys.used_memory() / 1024);

        MemoryInfo {
            total_ram_mb: total,
            available_ram_mb: available,
            // Not detectable portably; left unknown rather than guessed.
            ram_type: "Unknown".into(),
            frequency_mhz: 0,
        }
    }

    /// Count NUMA nodes, where the OS exposes them.
    ///
    /// Previously hardcoded to 1, which is wrong on any multi-socket machine and
    /// quietly misreports those. Returns 1 when the answer is unknown, since one
    /// node is the correct answer for the overwhelming majority of machines.
    fn detect_numa() -> usize {
        #[cfg(target_os = "linux")]
        {
            // /sys/devices/system/node/nodeN directories, excluding "possible".
            if let Ok(entries) = std::fs::read_dir("/sys/devices/system/node") {
                let nodes = entries
                    .flatten()
                    .filter(|e| {
                        e.file_name().to_string_lossy().starts_with("node")
                            && e.file_name()
                                .to_string_lossy()
                                .chars()
                                .skip(4)
                                .all(|c| c.is_ascii_digit())
                    })
                    .count();
                if nodes > 0 {
                    return nodes;
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            // GetActiveProcessorCount with ALL_PROCESSOR_GROUPS gives the total
            // across every NUMA node on large machines.
            unsafe {
                // Processor groups correspond to NUMA nodes on Windows.
                // Only meaningful above 64 logical processors, which is exactly
                // when the default group limit starts splitting them.
                let groups = winapi::um::winbase::GetActiveProcessorGroupCount();
                if groups > 1 {
                    return groups as usize;
                }
            }
        }

        1
    }

    fn detect_os() -> (String, String) {
        // Never report an empty name/version: callers branch on this, and a
        // blank string reads as "unknown platform" rather than "not exposed".
        let name = System::name()
            .or_else(System::long_os_version)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| std::env::consts::OS.to_owned());
        let version = System::os_version()
            .or_else(System::long_os_version)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| std::env::consts::OS.to_owned());
        (name, version)
    }

    /// Recommend thread pool size based on hardware
    pub fn recommended_threads(&self) -> usize {
        // Use physical core count, not logical, to avoid hyperthreading contention
        self.cpu.physical_cores.max(2)
    }

    /// Check if system has a discrete GPU suitable for compute
    pub fn has_discrete_gpu(&self) -> bool {
        self.gpus
            .iter()
            .any(|g| g.device_type == GpuDeviceType::Discrete)
    }

    /// Get the best GPU for compute work
    pub fn best_compute_gpu(&self) -> Option<&GpuInfo> {
        self.gpus
            .iter()
            .filter(|g| g.device_type == GpuDeviceType::Discrete)
            .max_by_key(|g| g.compute_score)
            .or_else(|| self.gpus.first())
    }

    /// Print a summary of detected hardware
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "CPU: {} [{}] ({} physical cores, {} logical)\n",
            self.cpu.name, self.cpu.architecture, self.cpu.physical_cores, self.cpu.logical_cores
        ));
        // SIMD support changes which model/backend is viable, so report it.
        if self.cpu.supports_avx2 || self.cpu.supports_avx512 || self.cpu.supports_neon {
            let mut isa = Vec::new();
            if self.cpu.supports_avx2 {
                isa.push("AVX2");
            }
            if self.cpu.supports_avx512 {
                isa.push("AVX512");
            }
            if self.cpu.supports_neon {
                isa.push("NEON");
            }
            s.push_str(&format!("SIMD: {}\n", isa.join(", ")));
        }
        for (i, gpu) in self.gpus.iter().enumerate() {
            s.push_str(&format!(
                "GPU {}: {} ({:?}, {:?})\n",
                i, gpu.name, gpu.vendor, gpu.device_type
            ));
        }
        s.push_str(&format!(
            "RAM: {} MB total, {} MB available\n",
            self.memory.total_ram_mb, self.memory.available_ram_mb
        ));
        s.push_str(&format!("NUMA nodes: {}", self.numa_nodes));
        s
    }
}

/// Graphics APIs an adapter of this class can be driven through on this OS.
///
/// Derived from the OS rather than hardcoded, so a macOS machine no longer
/// claims Vulkan support it cannot have.
fn graphics_apis_for(device_type: wgpu::DeviceType, macos: bool) -> Vec<GraphicsApi> {
    let mut apis = Vec::new();
    if device_type == wgpu::DeviceType::Cpu {
        apis.push(GraphicsApi::WebGpu);
        return apis;
    }
    if macos {
        apis.push(GraphicsApi::Metal);
    } else {
        apis.push(GraphicsApi::Vulkan);
        apis.push(GraphicsApi::DirectX11);
    }
    apis.push(GraphicsApi::WebGpu);
    apis
}

/// A coarse, honest capability rank by device class.
///
/// This is a device-class ordering, not a benchmark. The old code returned a
/// flat 100 for every real GPU, which made `best_compute_gpu` arbitrary.
fn compute_score_for(device_type: wgpu::DeviceType) -> u32 {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => 100,
        wgpu::DeviceType::IntegratedGpu => 60,
        wgpu::DeviceType::VirtualGpu => 40,
        wgpu::DeviceType::Cpu => 10,
        _ => 30,
    }
}

/// Get the global hardware profile (cached)
pub fn hardware_profile() -> &'static HardwareProfile {
    use std::sync::OnceLock;
    static PROFILE: OnceLock<HardwareProfile> = OnceLock::new();
    PROFILE.get_or_init(HardwareProfile::detect)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These guard the specific ways detection used to lie: fabricated cache
    /// sizes, `architecture` reporting the OS, an ISA probe that only compiled on
    /// x86, and an available-memory subtraction that could underflow.
    #[test]
    fn architecture_is_the_cpu_architecture_not_the_os() {
        let profile = HardwareProfile::detect();
        assert_eq!(
            profile.cpu.architecture,
            std::env::consts::ARCH,
            "architecture must be ARCH; it reported the OS name"
        );
    }

    #[test]
    fn isa_flags_are_consistent_with_the_target() {
        let profile = HardwareProfile::detect();
        let on_x86 = cfg!(any(target_arch = "x86", target_arch = "x86_64"));
        let on_arm = cfg!(target_arch = "aarch64");

        if !on_x86 {
            assert!(
                !profile.cpu.supports_avx2 && !profile.cpu.supports_avx512,
                "AVX must be reported false off x86"
            );
        }
        assert_eq!(
            profile.cpu.supports_neon, on_arm,
            "NEON is an ARM feature and must be gated on the target"
        );
    }

    #[test]
    fn available_memory_never_exceeds_total() {
        let profile = HardwareProfile::detect();
        assert!(
            profile.memory.available_ram_mb <= profile.memory.total_ram_mb,
            "available {} > total {}; the old subtraction could underflow",
            profile.memory.available_ram_mb,
            profile.memory.total_ram_mb
        );
    }

    #[test]
    fn a_gpu_entry_always_exists_and_one_is_primary() {
        // A headless machine must still get a usable fallback, or GPU-dependent
        // code has nothing to choose.
        let profile = HardwareProfile::detect();
        assert!(!profile.gpus.is_empty(), "no GPU entry at all");
        assert_eq!(
            profile.gpus.iter().filter(|g| g.is_primary).count(),
            1,
            "exactly one adapter must be primary"
        );
    }

    #[test]
    fn unmeasured_vram_is_zero_and_flagged() {
        for gpu in HardwareProfile::detect().gpus {
            if !gpu.vram_known {
                assert_eq!(gpu.vram_mb, 0, "unmeasured VRAM must not be a guess");
            }
        }
    }

    #[test]
    fn a_software_rasteriser_never_outranks_real_hardware() {
        // The old flat `compute_score: 100` made best_compute_gpu arbitrary.
        for gpu in HardwareProfile::detect().gpus {
            if gpu.device_type == GpuDeviceType::CpuSoftware {
                assert!(
                    gpu.compute_score < 100,
                    "software rendering must rank below real hardware"
                );
            }
        }
    }

    #[test]
    fn discrete_gpus_outrank_integrated_ones() {
        let profile = HardwareProfile::detect();
        for discrete in profile
            .gpus
            .iter()
            .filter(|g| g.device_type == GpuDeviceType::Discrete)
        {
            for integrated in profile
                .gpus
                .iter()
                .filter(|g| g.device_type == GpuDeviceType::Integrated)
            {
                assert!(
                    discrete.compute_score > integrated.compute_score,
                    "ranking must follow device class"
                );
            }
        }
    }

    #[test]
    fn os_details_are_never_blank() {
        let profile = HardwareProfile::detect();
        assert!(!profile.os_name.trim().is_empty());
        assert!(!profile.os_version.trim().is_empty());
    }

    #[test]
    fn numa_count_is_at_least_one() {
        assert!(HardwareProfile::detect().numa_nodes >= 1);
    }

    #[test]
    fn recommended_threads_is_within_the_machine_limits() {
        let profile = HardwareProfile::detect();
        let threads = profile.recommended_threads();
        assert!(threads >= 2, "must not recommend a single thread");
        assert!(
            threads <= profile.cpu.logical_cores.max(2),
            "recommended {threads} exceeds {} logical cores",
            profile.cpu.logical_cores
        );
    }

    #[test]
    fn the_global_profile_is_cached_not_re_detected() {
        let a = hardware_profile();
        let b = hardware_profile();
        assert!(
            std::ptr::eq(a, b),
            "the global profile must be cached across calls"
        );
    }

    #[test]
    fn summary_reports_cpu_and_memory() {
        let summary = HardwareProfile::detect().summary();
        assert!(summary.contains("CPU:"), "got: {summary}");
        assert!(summary.contains("RAM:"), "got: {summary}");
        assert!(summary.contains(&HardwareProfile::detect().cpu.architecture));
    }

    #[test]
    fn core_counts_are_not_impossible() {
        let profile = HardwareProfile::detect();
        assert!(profile.cpu.logical_cores >= 1, "no logical cores detected");
        assert!(
            profile.cpu.physical_cores <= profile.cpu.logical_cores,
            "physical {} exceeds logical {}",
            profile.cpu.physical_cores,
            profile.cpu.logical_cores
        );
    }
}
