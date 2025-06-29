use opencv::{
    core::{Mat, Point, Rect, Scalar, Vector},
    imgcodecs::{self, IMWRITE_JPEG_QUALITY},
    imgproc::{self, FONT_HERSHEY_SIMPLEX},
    objdetect::CascadeClassifier,
    prelude::*,
};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use crate::{config::Config, error::{AutoAimError, Result}};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Detection {
    pub face_rect: Rect,
    pub confidence: f64,
    pub timestamp: std::time::Instant,
}

/// Yüz tespit istatistikleri
#[derive(Debug, Clone)]
pub struct DetectionStats {
    pub total_frames: u64,
    pub faces_detected: u64,
    pub detection_rate: f64,
    pub start_time: Instant,
    pub saved_faces: u64,
}

impl DetectionStats {
    pub fn new() -> Self {
        Self {
            total_frames: 0,
            faces_detected: 0,
            detection_rate: 0.0,
            start_time: Instant::now(),
            saved_faces: 0,
        }
    }
    
    pub fn update(&mut self, detected_count: usize) {
        self.total_frames += 1;
        self.faces_detected += detected_count as u64;
        self.detection_rate = (self.faces_detected as f64 / self.total_frames as f64) * 100.0;
    }
    
    pub fn increment_saved_faces(&mut self) {
        self.saved_faces += 1;
    }
    
    pub fn get_fps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_frames as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Yüz tespit edici sistem
pub struct FaceDetector {
    classifier: Arc<Mutex<CascadeClassifier>>,
    config: Config,
    detection_sender: broadcast::Sender<Vec<Detection>>,
    stats: Arc<Mutex<DetectionStats>>,
    last_detection: Arc<Mutex<Option<std::time::Instant>>>,
}

impl FaceDetector {
    /// Yeni yüz tespit edici oluştur
    pub fn new(config: Config) -> Result<Self> {
        // OpenCV'nin varsayılan Haar cascade dosyasını yükle
        let cascade_path = Self::get_cascade_path();
        let classifier = CascadeClassifier::new(&cascade_path)
            .map_err(|e| AutoAimError::OpenCvError(format!("Cascade yüklenemedi: {}", e)))?;
        
        if classifier.empty()
            .map_err(|e| AutoAimError::OpenCvError(format!("Cascade boş: {}", e)))? {
            return Err(AutoAimError::OpenCvError("Cascade dosyası boş".to_string()));
        }
        
        let (detection_sender, _) = broadcast::channel(100);
        
        // Kaydetme dizinini oluştur
        if config.save_detected_faces {
            std::fs::create_dir_all(&config.face_save_directory)?;
            println!("📁 Yüz kaydetme dizini oluşturuldu: {}", config.face_save_directory);
        }
        
        Ok(Self {
            classifier: Arc::new(Mutex::new(classifier)),
            config,
            detection_sender,
            stats: Arc::new(Mutex::new(DetectionStats::new())),
            last_detection: Arc::new(Mutex::new(None)),
        })
    }
    
    /// Tespit alıcısını döndür
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<Detection>> {
        self.detection_sender.subscribe()
    }
    
    /// Frame'de yüz tespit et
    pub async fn detect_faces(&self, frame: &Mat) -> Result<Vec<Detection>> {
        // Gri tona çevir
        let mut gray = Mat::default();
        imgproc::cvt_color(frame, &mut gray, imgproc::COLOR_BGR2GRAY, 0, opencv::core::AlgorithmHint::ALGO_HINT_ACCURATE)
            .map_err(|e| AutoAimError::ImageProcessingError(format!("Gri tona çevrilemedi: {}", e)))?;
        
        // Histogram eşitleme (görüntü kalitesini artırır)
        let mut equalized = Mat::default();
        imgproc::equalize_hist(&gray, &mut equalized)
            .map_err(|e| AutoAimError::ImageProcessingError(format!("Histogram eşitlenemedi: {}", e)))?;
        
        // Yüzleri tespit et
        let mut faces = Vector::<Rect>::new();
        
        {
            let mut classifier = self.classifier.lock()
                .map_err(|e| AutoAimError::OpenCvError(format!("Classifier lock hatası: {}", e)))?;
            
            classifier.detect_multi_scale(
                &equalized,
                &mut faces,
                1.1,                          // Scale factor
                3,                            // Min neighbors
                0,                            // Flags
                self.config.min_face_size.into(),  // Min size
                self.config.max_face_size.into(),  // Max size
            ).map_err(|e| AutoAimError::OpenCvError(format!("Yüz tespiti hatası: {}", e)))?;
        }
        
        let mut detections = Vec::new();
        let now = std::time::Instant::now();
        
        // Tespit edilen yüzleri işle
        for i in 0..faces.len() {
            let face_rect = faces.get(i)?;
            let detection = Detection {
                face_rect,
                confidence: self.config.detection_confidence, // Haar cascade confidence vermez, config'den al
                timestamp: now,
            };
            
            // Yüzü kaydet (eğer ayar açıksa ve yeterince büyükse)
            if self.config.save_detected_faces && 
               face_rect.width >= self.config.min_face_save_size.0 && 
               face_rect.height >= self.config.min_face_save_size.1 {
                if let Err(e) = self.save_face_image(frame, &face_rect) {
                    eprintln!("⚠️  Yüz kaydetme hatası: {}", e);
                } else {
                    println!("💾 Yüz kaydedildi: {}x{} boyutunda", face_rect.width, face_rect.height);
                }
            }
            
            detections.push(detection.clone());
            
            // Cooldown kontrolü
            if self.should_notify() {
                let _ = self.detection_sender.send(detections.clone());
                self.update_last_detection(now);
            }
        }
        
        if self.config.debug_mode && !detections.is_empty() {
            log::debug!("{} yüz tespit edildi", detections.len());
        }
        
        // İstatistikleri güncelle
        self.stats.lock().unwrap().update(detections.len());
        
        Ok(detections)
    }
    
    /// Frame üzerine tespit edilen yüzleri çiz
    pub fn draw_detections(&self, frame: &mut Mat, detections: &[Detection]) -> Result<()> {
        for detection in detections {
            let rect = detection.face_rect;
            
            // Yüz etrafına dikdörtgen çiz (yeşil)
            imgproc::rectangle(
                frame,
                rect,
                Scalar::new(0.0, 255.0, 0.0, 0.0), // Yeşil
                2,
                imgproc::LINE_8,
                0,
            ).map_err(|e| AutoAimError::ImageProcessingError(format!("Dikdörtgen çizilemedi: {}", e)))?;
            
            // Yüz merkezine nokta çiz (kırmızı)
            let center = Point::new(
                rect.x + rect.width / 2,
                rect.y + rect.height / 2,
            );
            
            imgproc::circle(
                frame,
                center,
                5,
                Scalar::new(0.0, 0.0, 255.0, 0.0), // Kırmızı
                -1,
                imgproc::LINE_8,
                0,
            ).map_err(|e| AutoAimError::ImageProcessingError(format!("Nokta çizilemedi: {}", e)))?;
            
            // Confidence bilgisini yaz
            let text = format!("Yüz: {:.2}", detection.confidence);
            let text_pos = Point::new(rect.x, rect.y - 10);
            
            imgproc::put_text(
                frame,
                &text,
                text_pos,
                FONT_HERSHEY_SIMPLEX,
                0.5,
                Scalar::new(0.0, 255.0, 0.0, 0.0), // Yeşil
                1,
                imgproc::LINE_8,
                false,
            ).map_err(|e| AutoAimError::ImageProcessingError(format!("Metin yazılamadı: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Bildirim gönderilmeli mi?
    fn should_notify(&self) -> bool {
        let last_detection = self.last_detection.lock().unwrap();
        
        match *last_detection {
            Some(last_time) => {
                let elapsed = last_time.elapsed();
                elapsed.as_millis() >= self.config.detection_cooldown_ms as u128
            }
            None => true,
        }
    }
    
    /// Son tespit zamanını güncelle
    fn update_last_detection(&self, time: std::time::Instant) {
        let mut last_detection = self.last_detection.lock().unwrap();
        *last_detection = Some(time);
    }
    
    /// Cascade dosyası yolunu al
    fn get_cascade_path() -> String {
        // Farklı olası yolları dene
        let possible_paths = vec![
            "/usr/share/opencv4/haarcascades/haarcascade_frontalface_alt.xml",
            "/usr/local/share/opencv4/haarcascades/haarcascade_frontalface_alt.xml",
            "/opt/homebrew/share/opencv4/haarcascades/haarcascade_frontalface_alt.xml",
            "./haarcascade_frontalface_alt.xml",
            "haarcascade_frontalface_alt.xml",
        ];
        
        for path in possible_paths {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }
        
        // Varsayılan yol (genellikle Linux)
        "/usr/share/opencv4/haarcascades/haarcascade_frontalface_alt.xml".to_string()
    }
    
    /// Cascade dosyasını indir (eğer yoksa)
    pub async fn download_cascade_if_missing() -> Result<()> {
        let cascade_path = "./haarcascade_frontalface_alt.xml";
        
        if std::path::Path::new(cascade_path).exists() {
            return Ok(());
        }
        
        log::info!("Haar cascade dosyası bulunamadı, indiriliyor...");
        
        let url = "https://raw.githubusercontent.com/opencv/opencv/master/data/haarcascades/haarcascade_frontalface_alt.xml";
        
        let response = reqwest::get(url).await
            .map_err(|e| AutoAimError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("HTTP isteği başarısız: {}", e)
            )))?;
        
        let content = response.bytes().await
            .map_err(|e| AutoAimError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("İçerik okunamadı: {}", e)
            )))?;
        
        std::fs::write(cascade_path, content)
            .map_err(|e| AutoAimError::IoError(e))?;
        
        log::info!("Haar cascade dosyası indirildi: {}", cascade_path);
        
        Ok(())
    }
    

    
    /// İstatistikleri al
    pub fn get_stats(&self) -> DetectionStats {
        self.stats.lock().unwrap().clone()
    }
    
    /// Tespit edilen yüzü dosyaya kaydet
    fn save_face_image(&self, frame: &Mat, face_rect: &Rect) -> Result<()> {
        // Önce tam frame'in bir kopyasını oluştur
        let mut frame_copy = frame.clone();
        
        // Tam frame üzerinde nişangahı çiz
        self.draw_forehead_crosshair(&mut frame_copy, face_rect)?;
        
        // Sonra nişangahlı frame'den yüz bölgesini crop et
        let face_roi = Mat::roi(&frame_copy, *face_rect)?;
        
        // Timestamp ile dosya adı oluştur
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        
        let filename = format!(
            "{}/face_{}x{}_{}.jpg", 
            self.config.face_save_directory,
            face_rect.width,
            face_rect.height,
            timestamp
        );
        
        // JPEG kalite parametreleri
        let mut params = Vector::<i32>::new();
        params.push(IMWRITE_JPEG_QUALITY);
        params.push(self.config.face_image_quality as i32);
        
        // Dosyayı kaydet
        imgcodecs::imwrite(&filename, &face_roi, &params)?;
        
        // İstatistiklerde kayıt sayısını artır
        self.stats.lock().unwrap().increment_saved_faces();
        
        Ok(())
    }
    
    /// Alnın çatısına nişangah çiz
    pub fn draw_forehead_crosshair(&self, frame: &mut Mat, face_rect: &Rect) -> Result<()> {
        // Alın çatısının konumunu hesapla (yüzün üst ortası)
        let forehead_center = Point::new(
            face_rect.x + face_rect.width / 2,    // Yatay orta
            face_rect.y + (face_rect.height as f32 * 0.15) as i32  // Alnın çatısı (yüzün %15'i aşağıda)
        );
        
        // Nişangah boyutlarını yüz boyutuna göre ayarla
        let face_size = (face_rect.width + face_rect.height) / 2;
        let outer_radius = (face_size as f32 * 0.08) as i32; // Yüz boyutunun %8'i
        let inner_radius = (outer_radius as f32 * 0.25) as i32; // Dış yarıçapın %25'i
        
        // Nişangah renkleri
        let crosshair_color = Scalar::new(0.0, 255.0, 255.0, 0.0); // Sarı (dikkat çekici)
        let center_color = Scalar::new(0.0, 0.0, 255.0, 0.0);      // Kırmızı (merkez nokta)
        
        // Dış çember (ince)
        imgproc::circle(
            frame,
            forehead_center,
            outer_radius,
            crosshair_color,
            2, // İnce çizgi
            imgproc::LINE_AA, // Anti-aliasing
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Nişangah çemberi çizilemedi: {}", e)))?;
        
        // İç çember (dolu merkez nokta)
        imgproc::circle(
            frame,
            forehead_center,
            inner_radius,
            center_color,
            -1, // Dolu çember
            imgproc::LINE_AA,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Nişangah merkezi çizilemedi: {}", e)))?;
        
        // Çapraz çizgiler (isteğe bağlı - daha detaylı nişangah için)
        let cross_length = outer_radius / 2;
        
        // Yatay çizgi
        imgproc::line(
            frame,
            Point::new(forehead_center.x - cross_length, forehead_center.y),
            Point::new(forehead_center.x + cross_length, forehead_center.y),
            crosshair_color,
            1,
            imgproc::LINE_AA,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Yatay çizgi çizilemedi: {}", e)))?;
        
        // Dikey çizgi
        imgproc::line(
            frame,
            Point::new(forehead_center.x, forehead_center.y - cross_length),
            Point::new(forehead_center.x, forehead_center.y + cross_length),
            crosshair_color,
            1,
            imgproc::LINE_AA,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Dikey çizgi çizilemedi: {}", e)))?;
        
        Ok(())
    }
} 