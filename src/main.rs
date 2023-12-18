use std::{
    sync::{Mutex, OnceLock},
    thread,
};

use clap::Parser;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuExt, DiskExt, NetworkExt, NetworksExt, ProcessExt, System, SystemExt};

static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemStats {
    total: u64,
    used: u64,
    free: u64,
    available: u64,
    total_swap: u64,
    used_swap: u64,
    free_swap: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CpuCoreStats {
    usage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CpuStats {
    usage: f32,
    cpus: Vec<CpuCoreStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskPartStats {
    name: String,
    mount_point: String,
    total: u64,
    free: u64,
    used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskStats {
    total: u64,
    free: u64,
    used: u64,
    read: u64,
    write: u64,
    disks: Vec<DiskPartStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetInterfaceStats {
    name: String,
    up: u64,
    down: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetStats {
    total_up: u64,
    total_down: u64,
    up: u64,
    down: u64,
    interfaces: Vec<NetInterfaceStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SysStats {
    mem: MemStats,
    cpu: CpuStats,
    disks: DiskStats,
    net: NetStats,
}

impl From<&System> for SysStats {
    fn from(value: &System) -> Self {
        let total_cpu_usage =
            value.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / value.cpus().len() as f32;
        Self {
            mem: MemStats {
                total: value.total_memory(),
                used: value.used_memory(),
                free: value.free_memory(),
                available: value.available_memory(),
                total_swap: value.total_swap(),
                used_swap: value.used_swap(),
                free_swap: value.free_swap(),
            },
            cpu: CpuStats {
                usage: total_cpu_usage,
                cpus: value
                    .cpus()
                    .iter()
                    .map(|cpu| CpuCoreStats {
                        usage: cpu.cpu_usage(),
                    })
                    .collect(),
            },
            disks: {
                let mut disks = DiskStats {
                    total: 0,
                    free: 0,
                    used: 0,
                    read: 0,
                    write: 0,
                    disks: Vec::new(),
                };
                for disk in value.disks() {
                    let disk_part = DiskPartStats {
                        name: disk.name().to_string_lossy().to_string(),
                        mount_point: disk.mount_point().to_string_lossy().to_string(),
                        total: disk.total_space(),
                        free: disk.available_space(),
                        used: disk.total_space() - disk.available_space(),
                    };
                    disks.total += disk_part.total;
                    disks.free += disk_part.free;
                    disks.used += disk_part.used;
                    disks.disks.push(disk_part);
                }

                #[cfg(any(target_os = "windows", target_os = "freebsd"))]
                {
                    if let Some((_, process)) = value.processes().iter().next() {
                        disks.read += process.disk_usage().read_bytes;
                        disks.write += process.disk_usage().written_bytes;
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "freebsd")))]
                {
                    value.processes().iter().for_each(|(_, process)| {
                        disks.read += process.disk_usage().read_bytes;
                        disks.write += process.disk_usage().written_bytes;
                    });
                }
                disks
            },
            net: NetStats {
                total_up: value
                    .networks()
                    .iter()
                    .map(|(_, net)| net.total_transmitted())
                    .sum(),
                total_down: value
                    .networks()
                    .iter()
                    .map(|(_, net)| net.total_received())
                    .sum(),
                up: value
                    .networks()
                    .iter()
                    .map(|(_, net)| net.transmitted())
                    .sum(),
                down: value.networks().iter().map(|(_, net)| net.received()).sum(),
                interfaces: value
                    .networks()
                    .iter()
                    .map(|(name, net)| NetInterfaceStats {
                        name: name.clone(),
                        up: net.transmitted(),
                        down: net.received(),
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Clone, Parser)]
enum SubCommand {
    Loop {
        #[clap(short, long, default_value = "1.0")]
        interval: f32,
    },
}

#[derive(Debug, Clone, Parser)]
struct Args {
    #[clap(subcommand)]
    command: Option<SubCommand>,
}

fn main() {
    let args = Args::parse();
    SYSTEM.get_or_init(|| Mutex::new(System::new_all()));

    match args.command {
        Some(SubCommand::Loop { interval }) => loop_command(interval),
        None => {
            let mut system = SYSTEM
                .get_or_init(|| Mutex::new(System::new_all()))
                .lock()
                .unwrap();
            system.refresh_all();
            let stats = SysStats::from(&*system);

            println!("{}", serde_json::to_string(&stats).unwrap());
        }
    }
}

fn loop_command(interval: f32) {
    loop {
        let mut system = SYSTEM.get().unwrap().lock().unwrap();
        thread::sleep(std::time::Duration::from_secs_f32(interval));
        system.refresh_all();
        let stats = SysStats::from(&*system);

        println!("{}", serde_json::to_string(&stats).unwrap());
    }
}
