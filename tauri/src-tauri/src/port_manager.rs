use std::net::TcpListener;
use tokio::time::{sleep, Duration};

pub struct PortManager {
    preferred_port: u16,
    max_attempts: u16,
}

impl PortManager {
    pub fn new(preferred_port: u16) -> Self {
        Self {
            preferred_port,
            max_attempts: 100,
        }
    }
    
    pub async fn find_available_port(&self) -> Result<u16, Box<dyn std::error::Error>> {
        println!("Finding available port starting from {}", self.preferred_port);
        
        // Try preferred port and next 100 ports
        for port in self.preferred_port..(self.preferred_port + self.max_attempts) {
            if self.is_port_available(port).await {
                println!("Found available port: {}", port);
                return Ok(port);
            }
        }
        
        // If no ports in range are available, let OS assign one
        println!("No ports available in range, letting OS assign");
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener); // Release the port immediately
        
        // Wait a moment for port to be fully released
        sleep(Duration::from_millis(100)).await;
        
        Ok(port)
    }
    
    async fn is_port_available(&self, port: u16) -> bool {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => {
                drop(listener); // Release immediately
                // Small delay to ensure port is fully released
                sleep(Duration::from_millis(50)).await;
                true
            }
            Err(_) => false,
        }
    }
    
    pub async fn wait_for_server(&self, port: u16, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        
        println!("Waiting for server on port {} (timeout: {:?})", port, timeout);
        
        while start.elapsed() < timeout {
            match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port)).await {
                Ok(_) => {
                    println!("Server is ready on port {}", port);
                    return Ok(());
                }
                Err(_) => {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
        
        Err(format!("Server on port {} did not start within timeout", port).into())
    }
    
    pub fn cleanup_stale_processes(port: u16) {
        println!("Checking for stale processes on port {}", port);
        
        #[cfg(target_os = "windows")]
        {
            // Try to kill any process using the port
            let output = std::process::Command::new("cmd")
                .args(&["/C", &format!("netstat -ano | findstr :{} | findstr LISTENING", port)])
                .output();
                
            if let Ok(output) = output {
                let result = String::from_utf8_lossy(&output.stdout);
                // Parse PIDs from netstat output
                for line in result.lines() {
                    if let Some(pid_str) = line.split_whitespace().last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            println!("Found process {} on port {}, killing...", pid, port);
                            let _ = std::process::Command::new("taskkill")
                                .args(&["/PID", &pid.to_string(), "/F"])
                                .output();
                        }
                    }
                }
            }
        }
        
        #[cfg(unix)]
        {
            // Use lsof to find and kill processes
            let output = std::process::Command::new("lsof")
                .args(&["-ti", &format!(":{}", port)])
                .output();
                
            if let Ok(output) = output {
                let result = String::from_utf8_lossy(&output.stdout);
                for pid_str in result.lines() {
                    if let Ok(pid) = pid_str.trim().parse::<i32>() {
                        println!("Found process {} on port {}, killing...", pid, port);
                        let _ = std::process::Command::new("kill")
                            .args(&["-9", &pid.to_string()])
                            .output();
                    }
                }
            }
        }
    }
}