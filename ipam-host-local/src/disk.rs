use std::fs::{DirBuilder, File, OpenOptions};
use std::io::Write;
use std::net::IpAddr;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

const LINE_BREAK: &str = "\r\n";
const LAST_IPFILE_PREFIX: &str = "last_reserved_ip_";

// Store is a simple disk-backed store that creates one file per IP
// address in a given directory. The contents of the file are the container ID.
pub struct Store {
    pub dir: File,
    path: PathBuf,
}

impl Store {
    pub fn new(data_dir: Option<String>) -> anyhow::Result<Self> {
        let data_dir = data_dir.unwrap_or("/var/lib/cni/networks".into());
        DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(&data_dir)?;
        let path = PathBuf::from(data_dir);
        let file = File::open(&path)?;
        let store = Store { dir: file, path };
        Ok(store)
    }
    // GetByID returns the IPs which have been allocated to the specific ID
    pub fn get_by_id(&self, id: &str, ifname: &str) -> anyhow::Result<Vec<IpAddr>> {
        let text_match = format!("{}{}{}", id, LINE_BREAK, ifname);
        let mut result = vec![];
        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            if !entry.metadata()?.is_file() {
                continue;
            }
            let path = entry.path();
            let data = std::fs::read_to_string(&path)?;
            if data.trim() == text_match {
                let filename = path.file_name().unwrap().to_str().unwrap();
                let ip = filename.parse::<IpAddr>()?;
                result.push(ip);
            }
        }
        Ok(result)
    }
    pub fn reserve(
        &self,
        id: &str,
        ifname: &str,
        ip: IpAddr,
        range_id: &str,
    ) -> anyhow::Result<bool> {
        let file_path = self.path.join(ip.to_string());
        if file_path.exists() {
            return Ok(false);
        }
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(file_path)?;
        let content = format!("{}{}{}", id, LINE_BREAK, ifname);
        f.write_all(content.as_bytes())?;

        let last_ip_file_path = self
            .path
            .join(format!("{}{}", LAST_IPFILE_PREFIX, range_id));
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .open(last_ip_file_path)?;
        f.write_all(ip.to_string().as_bytes())?;
        Ok(true)
    }

    pub fn last_reserved_ip(&self, range_id: &str) -> Option<IpAddr> {
        let last_ip_file_path = self
            .path
            .join(format!("{}{}", LAST_IPFILE_PREFIX, range_id));
        std::fs::read_to_string(last_ip_file_path)
            .map(|it| it.parse().ok())
            .ok()
            .flatten()
    }

    pub fn release_by_id(&self, id: &str, ifname: &str) -> anyhow::Result<bool> {
        let text_match = format!("{}{}{}", id, LINE_BREAK, ifname);
        let mut found = false;
        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            if !entry.metadata()?.is_file() {
                continue;
            }
            let path = entry.path();
            let data = std::fs::read_to_string(&path)?;
            if data.trim() == text_match {
                std::fs::remove_file(&path)?;
                found = true;
            }
        }
        Ok(found)
    }
}

pub struct FileLock {
    fd: RawFd,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.unlock();
    }
}

pub trait FileLockExt {
    fn new_lock(&self) -> anyhow::Result<FileLock>;
}

impl FileLockExt for Store {
    fn new_lock(&self) -> anyhow::Result<FileLock> {
        let lock = FileLock {
            fd: self.dir.as_raw_fd(),
        };
        lock.lock()?;
        Ok(lock)
    }
}

impl FileLock {
    fn lock(&self) -> anyhow::Result<()> {
        nix::fcntl::flock(self.fd, nix::fcntl::FlockArg::LockExclusive)?;
        Ok(())
    }
    fn unlock(&self) -> anyhow::Result<()> {
        nix::fcntl::flock(self.fd, nix::fcntl::FlockArg::Unlock)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    #[test]
    fn test_file_lock() -> anyhow::Result<()> {
        let store = Store {
            dir: File::open("Cargo.toml").unwrap(),
            path: Default::default(),
        };

        {
            let lock = store.new_lock()?;
            println!("do something");
        }

        Ok(())
    }

    #[test]
    fn test_release() {
        std::fs::remove_dir_all("/tmp/ipam").unwrap_or_default();
        let store = Store::new(Some("/tmp/ipam".into())).unwrap();
        let _ = store.new_lock();
        store
            .reserve("id#0", "eth0", "192.168.1.2".parse().unwrap(), "1")
            .unwrap();
        sleep(std::time::Duration::from_secs(10));
        store.release_by_id("id#0", "eth0").unwrap();
    }
}
