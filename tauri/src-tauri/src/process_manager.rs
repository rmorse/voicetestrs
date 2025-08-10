use std::sync::{Arc, Mutex};
use std::process::Stdio;
use std::time::Duration;
use command_group::{CommandGroup, GroupChild};
use tokio::time::sleep;

pub struct ProcessManager {
    vite_process: Arc<Mutex<Option<GroupChild>>>,
    port: u16,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            vite_process: Arc::new(Mutex::new(None)),
            port: 5173,
        }
    }
    
    pub fn spawn_vite(&mut self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting Vite dev server on port {}", port);
        self.port = port;
        
        // Set up the command to run npm run dev
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = std::process::Command::new("cmd");
            c.args(&["/C", "npm", "run", "dev"]);
            c
        } else {
            let mut c = std::process::Command::new("npm");
            c.args(&["run", "dev"]);
            c
        };
        
        // Get the tauri directory path (parent of src-tauri)
        let tauri_dir = std::env::current_dir()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .expect("Failed to find tauri directory");
        
        println!("Running npm from directory: {:?}", tauri_dir);
        
        cmd.current_dir(tauri_dir)
            .env("VITE_DEV_PORT", port.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        
        // Use command-group to spawn as a process group
        let child = cmd.group_spawn()?;
        
        // Store the process handle
        *self.vite_process.lock().unwrap() = Some(child);
        
        println!("Vite process spawned successfully");
        Ok(())
    }
    
    pub async fn wait_for_server(&self, timeout_secs: u64) -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        
        println!("Waiting for Vite server to be ready on port {}...", self.port);
        
        while start.elapsed() < timeout {
            if self.check_server_ready().await {
                println!("Vite server is ready!");
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }
        
        Err("Vite server failed to start within timeout".into())
    }
    
    async fn check_server_ready(&self) -> bool {
        // Try to connect to the port - try both IPv4 and IPv6
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", self.port)).await.is_ok() {
            return true;
        }
        
        // Also try IPv6 localhost
        if tokio::net::TcpStream::connect(format!("[::1]:{}", self.port)).await.is_ok() {
            return true;
        }
        
        // Also try localhost hostname
        if tokio::net::TcpStream::connect(format!("localhost:{}", self.port)).await.is_ok() {
            return true;
        }
        
        false
    }
    
    pub fn cleanup_all(&mut self) {
        println!("Cleaning up Vite process...");
        
        if let Some(mut child) = self.vite_process.lock().unwrap().take() {
            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
                
                if let Some(pid) = child.id() {
                    let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
            
            // Force kill if still running
            match child.try_wait() {
                Ok(Some(_)) => {
                    println!("Vite process exited gracefully");
                }
                _ => {
                    println!("Force killing Vite process");
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        
        // On Windows, also try taskkill for any orphaned processes
        #[cfg(target_os = "windows")]
        {
            // Kill any node processes on our port
            let _ = std::process::Command::new("cmd")
                .args(&["/C", &format!("for /f \"tokens=5\" %a in ('netstat -ano ^| findstr :{} ^| findstr LISTENING') do taskkill /PID %a /F", self.port)])
                .output();
        }
        
        println!("Process cleanup complete");
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.cleanup_all();
    }
}