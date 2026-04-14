use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MountConfig {
    pub acregmin: Duration,
    pub acregmax: Duration,
    pub acdirmin: Duration,
    pub acdirmax: Duration,
    pub rsize: usize,
    pub wsize: usize,
    pub write_through: bool,
    pub uid: u32,
    pub gid: u32,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            acregmin: Duration::from_secs(1),
            acregmax: Duration::from_secs(60),
            acdirmin: Duration::from_secs(3),
            acdirmax: Duration::from_secs(60),
            rsize: 1_048_576,
            wsize: 1_048_576,
            write_through: false,
            uid: 65534,
            gid: 65534,
        }
    }
}
