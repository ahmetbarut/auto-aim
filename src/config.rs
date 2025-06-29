use serde::{Deserialize, Serialize};
use crate::error::{AutoAimError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Kamera device ID'si (0 = varsayılan kamera)
    pub camera_id: i32,
    
    /// Video çözünürlüğü genişlik
    pub frame_width: i32,
    
    /// Video çözünürlüğü yükseklik  
    pub frame_height: i32,
    
    /// FPS (saniyede frame)
    pub fps: f64,
    
    /// Tespit hassasiyeti (0.1 - 1.0)
    pub detection_confidence: f64,
    
    /// Minimum tespit edilen yüz boyutu (piksel)
    pub min_face_size: (i32, i32),
    
    /// Maksimum tespit edilen yüz boyutu (piksel)
    pub max_face_size: (i32, i32),
    
    /// Bildirim ses seviyesi (0-100)
    pub notification_volume: u8,
    
    /// Debug modu aktif mi
    pub debug_mode: bool,
    
    /// Tespit sonrası bekleme süresi (milisaniye)
    pub detection_cooldown_ms: u64,
    
    /// Tespit edilen yüzleri kaydet
    pub save_detected_faces: bool,
    
    /// Yüz kaydetme dizini
    pub face_save_directory: String,
    
    /// Kaydedilen yüz resmi kalitesi (1-100)
    pub face_image_quality: u8,
    
    /// Yüz kaydetme için minimum boyut (küçük yüzleri kaydetme)
    pub min_face_save_size: (i32, i32),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            camera_id: 0,
            frame_width: 640,
            frame_height: 480,
            fps: 30.0,
            detection_confidence: 0.5,
            min_face_size: (30, 30),
            max_face_size: (300, 300),
            notification_volume: 80,
            debug_mode: false,
            detection_cooldown_ms: 1000, // 1 saniye
            save_detected_faces: true,
            face_save_directory: "detected_faces".to_string(),
            face_image_quality: 90,
            min_face_save_size: (50, 50), // En az 50x50 piksel olan yüzleri kaydet
        }
    }
}

impl Config {
    /// Konfigürasyonu yükle veya varsayılan olarak oluştur
    pub fn load_or_create(config_path: &std::path::Path) -> Result<Self> {
        if config_path.exists() {
            log::info!("Konfigürasyon dosyası yükleniyor: {:?}", config_path);
            let config = Self::load_from_file(&config_path.to_string_lossy())?;
            config.validate()?;
            Ok(config)
        } else {
            log::info!("Konfigürasyon dosyası bulunamadı, varsayılan oluşturuluyor: {:?}", config_path);
            let config = Self::default();
            config.save_to_file(&config_path.to_string_lossy())?;
            log::info!("Varsayılan konfigürasyon kaydedildi: {:?}", config_path);
            Ok(config)
        }
    }

    /// Konfigürasyon dosyasından yükle
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AutoAimError::ConfigError(format!("Dosya okunamadı: {}", e)))?;
        
        let config: Config = toml::from_str(&content)
            .map_err(|e| AutoAimError::ConfigError(format!("TOML parse hatası: {}", e)))?;
        
        Ok(config)
    }
    
    /// Konfigürasyonu dosyaya kaydet
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| AutoAimError::ConfigError(format!("TOML serialize hatası: {}", e)))?;
        
        std::fs::write(path, content)
            .map_err(|e| AutoAimError::ConfigError(format!("Dosya yazılamadı: {}", e)))?;
        
        Ok(())
    }
    
    /// Konfigürasyonu doğrula
    pub fn validate(&self) -> Result<()> {
        if self.detection_confidence < 0.1 || self.detection_confidence > 1.0 {
            return Err(AutoAimError::ConfigError(
                "Detection confidence 0.1-1.0 arasında olmalı".to_string()
            ));
        }
        
        if self.notification_volume > 100 {
            return Err(AutoAimError::ConfigError(
                "Notification volume 0-100 arasında olmalı".to_string()
            ));
        }
        
        if self.fps <= 0.0 || self.fps > 120.0 {
            return Err(AutoAimError::ConfigError(
                "FPS 0-120 arasında olmalı".to_string()
            ));
        }
        
        Ok(())
    }
} 