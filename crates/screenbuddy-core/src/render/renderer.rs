//! Multi-GPU Rendering Architecture for ScreenBuddy
use std::sync::Arc;

use crate::error::Result;

/// Trait combining window handle access
pub trait WindowHandle: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Send + Sync {}
impl<T: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Send + Sync> WindowHandle for T {}

/// GPU device priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuPriority { Other = 0, Integrated = 1, Discrete = 2 }

impl From<wgpu::DeviceType> for GpuPriority {
    fn from(dt: wgpu::DeviceType) -> Self {
        match dt {
            wgpu::DeviceType::DiscreteGpu => GpuPriority::Discrete,
            wgpu::DeviceType::IntegratedGpu => GpuPriority::Integrated,
            _ => GpuPriority::Other,
        }
    }
}

/// GPU adapter info
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub priority: GpuPriority,
    pub device_type: wgpu::DeviceType,
}

/// GPU scheduler trait
pub trait GpuScheduler: Send + Sync {
    fn name(&self) -> &str;
    fn select_gpu(&mut self, num_gpus: usize) -> usize;
}

/// Round-robin scheduler
pub struct RoundRobinScheduler { current: usize }
impl RoundRobinScheduler { pub fn new() -> Self { Self { current: 0 } } }
impl GpuScheduler for RoundRobinScheduler {
    fn name(&self) -> &str { "RoundRobin" }
    fn select_gpu(&mut self, num_gpus: usize) -> usize {
        let idx = self.current;
        self.current = (self.current + 1) % num_gpus.max(1);
        idx
    }
}

/// Load-balanced scheduler
pub struct LoadBalancedScheduler;
impl GpuScheduler for LoadBalancedScheduler {
    fn name(&self) -> &str { "LoadBalanced" }
    fn select_gpu(&mut self, _num_gpus: usize) -> usize { 0 }
}

/// Single-GPU scheduler
pub struct SingleGpuScheduler;
impl GpuScheduler for SingleGpuScheduler {
    fn name(&self) -> &str { "SingleGPU" }
    fn select_gpu(&mut self, _num_gpus: usize) -> usize { 0 }
}

/// Multi-GPU renderer - detects and manages all available GPUs
pub struct MultiGpuRenderer {
    pub gpus: Vec<GpuInfo>,
    scheduler: Box<dyn GpuScheduler>,
}

impl MultiGpuRenderer {
    pub fn new(window: Arc<dyn WindowHandle>, _width: u32, _height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let _surface = instance.create_surface(window.clone())
            .map_err(|e| crate::error::Error::Render(e.to_string()))?;

        let mut gpus: Vec<GpuInfo> = instance.enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .map(|adapter| {
                let info = adapter.get_info();
                GpuInfo {
                    name: info.name,
                    priority: GpuPriority::from(info.device_type),
                    device_type: info.device_type,
                }
            })
            .collect();

        gpus.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(Self { gpus, scheduler: Box::new(RoundRobinScheduler::new()) })
    }

    pub fn gpu_count(&self) -> usize { self.gpus.len() }
    pub fn gpu_info(&self) -> &[GpuInfo] { &self.gpus }
    pub fn poll_devices(&self, _wait: bool) {}
    pub fn set_scheduler(&mut self, scheduler: Box<dyn GpuScheduler>) { self.scheduler = scheduler; }
    pub fn force_single_gpu(&mut self) { self.scheduler = Box::new(SingleGpuScheduler); }
}

pub type Renderer = MultiGpuRenderer;
