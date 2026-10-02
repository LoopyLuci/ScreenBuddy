//! Hardware Detection and Capability Reporting
use std::collections::HashMap;
use sysinfo::{System, CpuRefreshKind, MemoryRefreshKind, RefreshKind};

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
    pub driver_version: String,
    pub api_support: Vec<GraphicsApi>, // Vulkan, DirectX, Metal, OpenGL
    pub compute_score: u32, // relative compute capability
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GpuVendor { Nvidia, Amd, Intel, Apple, Unknown }

#[derive(Debug, Clone, PartialEq)]
pub enum GpuDeviceType { Discrete, Integrated, Virtual, CpuSoftware }

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsApi { Vulkan, DirectX11, DirectX12, Metal, OpenGl, WebGpu }

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

        Self { cpu, gpus, memory, numa_nodes, os_name, os_version }
    }

    fn detect_cpu(sys: &System) -> CpuInfo {
        let cpus = sys.cpus();
        let physical_cores = sys.physical_core_count().unwrap_or(1);
        let logical_cores = cpus.len();
        let name = cpus.first().map(|c| c.brand().to_string()).unwrap_or_default();
        let arch = std::env::consts::OS.to_string();

        CpuInfo {
            name,
            physical_cores,
            logical_cores,
            base_frequency_mhz: cpus.first().map(|c| c.frequency()).unwrap_or(0),
            max_frequency_mhz: cpus.first().map(|c| c.frequency()).unwrap_or(0),
            architecture: arch,
            supports_avx2: is_x86_feature_detected!("avx2"),
            supports_avx512: is_x86_feature_detected!("avx512f"),
            supports_neon: cfg!(target_arch = "aarch64"),
            l1_cache_kb: 32,  // TODO: read from CPUID
            l2_cache_kb: 256,
            l3_cache_kb: 8192,
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
                vram_mb: 0, // Would need DXGI/nvml to get actual VRAM
                driver_version: info.driver,
                api_support: vec![GraphicsApi::WebGpu, GraphicsApi::Vulkan],
                compute_score: 100, // Would need actual benchmarks
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
                driver_version: "N/A".into(),
                api_support: vec![GraphicsApi::WebGpu],
                compute_score: 10,
                is_primary: true,
            });
        }

        gpus
    }

    fn detect_memory(sys: &System) -> MemoryInfo {
        MemoryInfo {
            total_ram_mb: sys.total_memory() / 1024,
            available_ram_mb: sys.total_memory() / 1024 - sys.used_memory() / 1024,
            ram_type: "Unknown".into(),
            frequency_mhz: 0,
        }
    }

    fn detect_numa() -> usize {
        // On Linux, count NUMA nodes from /sys/devices/system/node/
        // On Windows, use GetNumaNodeCount
        1 // Simplified: assume single NUMA node
    }

    fn detect_os() -> (String, String) {
        (System::name().unwrap_or_default(), System::os_version().unwrap_or_default())
    }

    /// Recommend thread pool size based on hardware
    pub fn recommended_threads(&self) -> usize {
        // Use physical core count, not logical, to avoid hyperthreading contention
        self.cpu.physical_cores.max(2)
    }

    /// Check if system has a discrete GPU suitable for compute
    pub fn has_discrete_gpu(&self) -> bool {
        self.gpus.iter().any(|g| g.device_type == GpuDeviceType::Discrete)
    }

    /// Get the best GPU for compute work
    pub fn best_compute_gpu(&self) -> Option<&GpuInfo> {
        self.gpus.iter()
            .filter(|g| g.device_type == GpuDeviceType::Discrete)
            .max_by_key(|g| g.compute_score)
            .or_else(|| self.gpus.first())
    }

    /// Print a summary of detected hardware
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("CPU: {} ({} physical cores, {} logical)\n",
            self.cpu.name, self.cpu.physical_cores, self.cpu.logical_cores));
        for (i, gpu) in self.gpus.iter().enumerate() {
            s.push_str(&format!("GPU {}: {} ({:?}, {:?})\n",
                i, gpu.name, gpu.vendor, gpu.device_type));
        }
        s.push_str(&format!("RAM: {} MB total, {} MB available\n",
            self.memory.total_ram_mb, self.memory.available_ram_mb));
        s.push_str(&format!("NUMA nodes: {}", self.numa_nodes));
        s
    }
}

/// Get the global hardware profile (cached)
pub fn hardware_profile() -> &'static HardwareProfile {
    use std::sync::OnceLock;
    static PROFILE: OnceLock<HardwareProfile> = OnceLock::new();
    PROFILE.get_or_init(HardwareProfile::detect)
}
