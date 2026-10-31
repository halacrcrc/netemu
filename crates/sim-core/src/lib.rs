//! 网络仿真内核：拓扑/设备模型、事件引擎、逐包转发。
//!
//! 设计原则：
//! - 设备状态是**厂商中立**的（接口表、ARP、RIB、MAC 表），CLI 方言在
//!   cli-dialects 中作为独立前端存在，同一状态可挂多棵命令树。
//! - 逐包仿真：每个帧按 事件(到达时间) 排队，逐跳处理，
//!   虚拟时钟确定性推进（同 (time, seq) 顺序稳定）。

pub mod device;
pub mod engine;
pub mod net;
pub mod summary;

pub use device::*;
pub use engine::{Engine, Stats, PING_HORIZON_NS};
pub use net::{Link, Net};
pub use sim_packets as packets;
