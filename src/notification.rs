use std::io::{self, Write};
use std::process::Command;
use tokio::sync::broadcast;
use crate::{config::Config, detector::Detection, error::{AutoAimError, Result}};

pub struct NotificationSystem {
    config: Config,
}

impl NotificationSystem {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
    
    /// Tespit bildirimleri için dinleyici başlat
    pub async fn start_listening(&self, mut receiver: broadcast::Receiver<Vec<Detection>>) -> Result<()> {
        log::info!("Bildirim sistemi başlatılıyor...");
        
        while let Ok(detections) = receiver.recv().await {
            self.handle_detections(detections).await?;
        }
        
        Ok(())
    }
    
    /// Tespit işlemi gerçekleştiğinde çağrılır
    async fn handle_detections(&self, detections: Vec<Detection>) -> Result<()> {
        if detections.is_empty() {
            return Ok(());
        }
        
        let message = if detections.len() == 1 {
            let detection = &detections[0];
            format!(
                "YÜZ TESPİT EDİLDİ! Konum: ({}, {}) Boyut: {}x{}",
                detection.face_rect.x,
                detection.face_rect.y,
                detection.face_rect.width,
                detection.face_rect.height
            )
        } else {
            format!(
                "{} YÜZ TESPİT EDİLDİ! İlk yüz: ({}, {}) Boyut: {}x{}",
                detections.len(),
                detections[0].face_rect.x,
                detections[0].face_rect.y,
                detections[0].face_rect.width,
                detections[0].face_rect.height
            )
        };
        
        log::info!("{}", message);
        
        // Farklı bildirim türleri
        if let Err(e) = self.console_notification(&message).await {
            log::warn!("Konsol bildirimi başarısız: {}", e);
        }
        
        if let Err(e) = self.system_notification(&message).await {
            log::warn!("Sistem bildirimi başarısız: {}", e);
        }
        
        if let Err(e) = self.audio_notification().await {
            log::warn!("Ses bildirimi başarısız: {}", e);
        }
        
        if let Err(e) = self.terminal_bell().await {
            log::warn!("Terminal bell başarısız: {}", e);
        }
        
        Ok(())
    }
    
    /// Konsola renkli bildirim yazdır
    async fn console_notification(&self, message: &str) -> Result<()> {
        // ANSI renk kodları
        const RED: &str = "\x1b[31m";
        const GREEN: &str = "\x1b[32m";
        const YELLOW: &str = "\x1b[33m";
        const BLUE: &str = "\x1b[34m";
        const MAGENTA: &str = "\x1b[35m";
        const CYAN: &str = "\x1b[36m";
        const BOLD: &str = "\x1b[1m";
        const RESET: &str = "\x1b[0m";
        
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        println!();
        println!("{}{}🎯 ==========================================={}", BOLD, RED, RESET);
        println!("{}{}📸 YÜZ TESPİT SİSTEMİ UYARISI{}", BOLD, YELLOW, RESET);
        println!("{}{}⏰ Zaman: {}{}", BOLD, CYAN, timestamp, RESET);
        println!("{}{}📍 {}{}", BOLD, GREEN, message, RESET);
        println!("{}{}🎯 ==========================================={}", BOLD, RED, RESET);
        println!();
        
        // Stdout'u flush et
        io::stdout().flush()
            .map_err(|e| AutoAimError::IoError(e))?;
        
        Ok(())
    }
    
    /// Sistem bildirimi gönder (macOS, Linux, Windows)
    async fn system_notification(&self, message: &str) -> Result<()> {
        let title = "Auto-Aim: Yüz Tespit Edildi";
        
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                r#"display notification "{}" with title "{}""#,
                message, title
            );
            
            let output = Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    log::debug!("macOS bildirimi gönderildi");
                }
                Ok(result) => {
                    let error = String::from_utf8_lossy(&result.stderr);
                    log::warn!("macOS bildirimi başarısız: {}", error);
                }
                Err(e) => {
                    log::warn!("osascript çalıştırılamadı: {}", e);
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("notify-send")
                .arg(title)
                .arg(message)
                .arg("--urgency=critical")
                .arg("--icon=camera-video")
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    log::debug!("Linux bildirimi gönderildi");
                }
                Ok(result) => {
                    let error = String::from_utf8_lossy(&result.stderr);
                    log::warn!("Linux bildirimi başarısız: {}", error);
                }
                Err(e) => {
                    log::warn!("notify-send çalıştırılamadı: {}", e);
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Windows için PowerShell kullan
            let script = format!(
                r#"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('{}', '{}', 'OK', 'Information')"#,
                message, title
            );
            
            let output = Command::new("powershell")
                .arg("-Command")
                .arg(&script)
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    log::debug!("Windows bildirimi gönderildi");
                }
                Ok(result) => {
                    let error = String::from_utf8_lossy(&result.stderr);
                    log::warn!("Windows bildirimi başarısız: {}", error);
                }
                Err(e) => {
                    log::warn!("PowerShell çalıştırılamadı: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Ses bildirimi çal
    async fn audio_notification(&self) -> Result<()> {
        // macOS için say komutu
        #[cfg(target_os = "macos")]
        {
            let voice_text = "Yüz tespit edildi";
            
            let output = Command::new("say")
                .arg(voice_text)
                .arg("--rate=200")
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    log::debug!("Ses bildirimi çalındı (macOS)");
                }
                Ok(result) => {
                    let error = String::from_utf8_lossy(&result.stderr);
                    log::warn!("Ses bildirimi başarısız: {}", error);
                }
                Err(e) => {
                    log::warn!("say komutu çalıştırılamadı: {}", e);
                }
            }
        }
        
        // Linux için espeak veya beep
        #[cfg(target_os = "linux")]
        {
            // Önce espeak dene
            let espeak_output = Command::new("espeak")
                .arg("Yüz tespit edildi")
                .arg("--speed=150")
                .output();
                
            match espeak_output {
                Ok(result) if result.status.success() => {
                    log::debug!("Ses bildirimi çalındı (espeak)");
                    return Ok(());
                }
                _ => {
                    // espeak başarısız, beep dene
                    let beep_output = Command::new("beep")
                        .arg("-f")
                        .arg("1000")
                        .arg("-l")
                        .arg("500")
                        .output();
                        
                    match beep_output {
                        Ok(result) if result.status.success() => {
                            log::debug!("Beep sesi çalındı");
                        }
                        _ => {
                            log::warn!("Ses bildirimi çalınamadı (espeak ve beep başarısız)");
                        }
                    }
                }
            }
        }
        
        // Windows için system beep
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("rundll32")
                .arg("user32.dll,MessageBeep")
                .arg("0")
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    log::debug!("System beep çalındı (Windows)");
                }
                Ok(result) => {
                    let error = String::from_utf8_lossy(&result.stderr);
                    log::warn!("System beep başarısız: {}", error);
                }
                Err(e) => {
                    log::warn!("rundll32 çalıştırılamadı: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Terminal bell ses çal
    async fn terminal_bell(&self) -> Result<()> {
        // ASCII Bell karakteri (0x07)
        print!("\x07");
        io::stdout().flush()
            .map_err(|e| AutoAimError::IoError(e))?;
        
        log::debug!("Terminal bell çalındı");
        Ok(())
    }
    
    /// Bildirim sesini test et
    pub async fn test_notifications(&self) -> Result<()> {
        println!("🧪 Bildirim sistemi test ediliyor...");
        
        println!("1. Konsol bildirimi:");
        self.console_notification("Test mesajı - bu bir deneme bildirimidir").await?;
        
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        println!("2. Sistem bildirimi:");
        self.system_notification("Test mesajı - bu bir deneme bildirimidir").await?;
        
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        println!("3. Ses bildirimi:");
        self.audio_notification().await?;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        println!("4. Terminal bell:");
        self.terminal_bell().await?;
        
        println!("✅ Bildirim testleri tamamlandı!");
        
        Ok(())
    }
} 