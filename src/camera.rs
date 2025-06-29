use opencv::{
    core::Mat,
    prelude::*,
    videoio::{VideoCapture, VideoCaptureTrait, CAP_ANY},
};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use crate::{config::Config, error::{AutoAimError, Result}};

pub struct Camera {
    capture: Arc<Mutex<VideoCapture>>,
    config: Config,
    frame_sender: broadcast::Sender<Mat>,
    is_running: Arc<Mutex<bool>>,
}

impl Camera {
    /// Yeni kamera örneği oluştur
    pub fn new(config: Config) -> Result<Self> {
        let mut capture = VideoCapture::new(config.camera_id, CAP_ANY)
            .map_err(|e| AutoAimError::CameraError(format!("Kamera açılamadı: {}", e)))?;
        
        if !capture.is_opened()
            .map_err(|e| AutoAimError::CameraError(format!("Kamera durumu kontrol edilemedi: {}", e)))? {
            return Err(AutoAimError::CameraError("Kamera açık değil".to_string()));
        }
        
        // Kamera ayarlarını yapılandır
        capture.set(opencv::videoio::CAP_PROP_FRAME_WIDTH, config.frame_width as f64)
            .map_err(|e| AutoAimError::CameraError(format!("Genişlik ayarlanamadı: {}", e)))?;
        
        capture.set(opencv::videoio::CAP_PROP_FRAME_HEIGHT, config.frame_height as f64)
            .map_err(|e| AutoAimError::CameraError(format!("Yükseklik ayarlanamadı: {}", e)))?;
        
        capture.set(opencv::videoio::CAP_PROP_FPS, config.fps)
            .map_err(|e| AutoAimError::CameraError(format!("FPS ayarlanamadı: {}", e)))?;
        
        let (frame_sender, _) = broadcast::channel(10);
        
        Ok(Self {
            capture: Arc::new(Mutex::new(capture)),
            config,
            frame_sender,
            is_running: Arc::new(Mutex::new(false)),
        })
    }
    
    /// Frame alıcısını döndür
    pub fn subscribe(&self) -> broadcast::Receiver<Mat> {
        self.frame_sender.subscribe()
    }
    
    /// Kamerayı başlat
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.lock()
            .map_err(|e| AutoAimError::CameraError(format!("Lock hatası: {}", e)))?;
        
        if *is_running {
            return Err(AutoAimError::CameraError("Kamera zaten çalışıyor".to_string()));
        }
        
        *is_running = true;
        
        log::info!("Kamera başlatılıyor...");
        
        let capture = Arc::clone(&self.capture);
        let sender = self.frame_sender.clone();
        let is_running_clone = Arc::clone(&self.is_running);
        let debug_mode = self.config.debug_mode;
        
        tokio::spawn(async move {
            let mut frame = Mat::default();
            let mut frame_count = 0u64;
            
            loop {
                // Çalışma durumunu kontrol et
                {
                    let running = is_running_clone.lock().unwrap();
                    if !*running {
                        break;
                    }
                }
                
                // Frame'i yakala
                let read_result = {
                    match capture.lock() {
                        Ok(mut cap) => {
                            cap.read(&mut frame)
                        }
                        Err(e) => {
                            log::error!("Kamera lock hatası: {}", e);
                            Err(opencv::Error::new(opencv::core::StsError, "Lock error"))
                        }
                    }
                };
                
                match read_result {
                    Ok(true) => {
                        if !frame.empty() {
                            frame_count += 1;
                            
                            if debug_mode && frame_count % 30 == 0 {
                                log::debug!("Frame yakalandı: #{}", frame_count);
                            }
                            
                            // Frame'i gönder (hata durumunda devam et)
                            let _ = sender.send(frame.clone());
                        } else {
                            log::warn!("Boş frame alındı");
                        }
                    }
                    Ok(false) => {
                        log::warn!("Frame okunamadı, tekrar denenecek...");
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        log::error!("Frame okuma hatası: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
                
                // FPS kontrolü için kısa bekleme
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
            
            log::info!("Kamera durduruldu");
        });
        
        Ok(())
    }
    
    /// Kamerayı durdur
    pub fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.lock()
            .map_err(|e| AutoAimError::CameraError(format!("Lock hatası: {}", e)))?;
        
        *is_running = false;
        log::info!("Kamera durduruluyor...");
        
        Ok(())
    }
    
    /// Kamera çalışıyor mu?
    pub fn is_running(&self) -> bool {
        self.is_running.lock().map(|r| *r).unwrap_or(false)
    }
    
    /// Kamera bilgilerini al
    pub fn get_info(&self) -> Result<CameraInfo> {
        let capture = self.capture.lock()
            .map_err(|e| AutoAimError::CameraError(format!("Lock hatası: {}", e)))?;
        
        let width = capture.get(opencv::videoio::CAP_PROP_FRAME_WIDTH)
            .map_err(|e| AutoAimError::CameraError(format!("Genişlik alınamadı: {}", e)))? as i32;
        
        let height = capture.get(opencv::videoio::CAP_PROP_FRAME_HEIGHT)
            .map_err(|e| AutoAimError::CameraError(format!("Yükseklik alınamadı: {}", e)))? as i32;
        
        let fps = capture.get(opencv::videoio::CAP_PROP_FPS)
            .map_err(|e| AutoAimError::CameraError(format!("FPS alınamadı: {}", e)))?;
        
        Ok(CameraInfo {
            width,
            height,
            fps,
            device_id: self.config.camera_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub width: i32,
    pub height: i32,
    pub fps: f64,
    pub device_id: i32,
}

impl Drop for Camera {
    fn drop(&mut self) {
        let _ = self.stop();
    }
} 