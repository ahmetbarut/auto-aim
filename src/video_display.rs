use opencv::{
    core::{Mat, Point, Rect, Scalar},
    highgui::{self, WindowFlags},
    imgproc::{self, FONT_HERSHEY_SIMPLEX, LINE_8},
    prelude::*,
};
use std::collections::VecDeque;
use std::time::Instant;
use crate::{
    detector::{Detection, DetectionStats},
    error::Result,
};

/// Gelişmiş video görüntü sistemi
pub struct VideoDisplay {
    window_name: String,
    fps_history: VecDeque<f64>,
    last_frame_time: Instant,
    show_overlay: bool,
    overlay_alpha: f64,
    window_created: bool,
}

impl VideoDisplay {
    /// Yeni video görüntü sistemi oluştur
    pub fn new(window_name: &str) -> Result<Self> {
        let mut display = Self {
            window_name: window_name.to_string(),
            fps_history: VecDeque::with_capacity(30),
            last_frame_time: Instant::now(),
            show_overlay: true,
            overlay_alpha: 0.7,
            window_created: false,
        };
        
        // Pencereyi oluşturmayı dene
        if let Err(e) = display.setup_window() {
            eprintln!("⚠️  Pencere oluşturma hatası: {}", e);
            eprintln!("💡 macOS'ta kamera izni gerekli olabilir (Sistem Tercihleri > Güvenlik ve Gizlilik > Kamera)");
            return Err(e);
        }
        
        Ok(display)
    }
    
    /// Pencereyi ayarla
    fn setup_window(&mut self) -> Result<()> {
        // Önce OpenCV'nin GUI sistemini başlat
        match highgui::start_window_thread() {
            Ok(_) => {},
            Err(e) => {
                eprintln!("⚠️  GUI thread başlatma hatası: {}", e);
                // Bu hata macOS'ta normal olabilir, devam edelim
            }
        }
        
        // Pencere oluştur - daha basit flags ile
        match highgui::named_window(&self.window_name, WindowFlags::WINDOW_AUTOSIZE as i32) {
            Ok(_) => {
                self.window_created = true;
                println!("🖼️  Video penceresi oluşturuldu: {}", self.window_name);
            }
            Err(e) => {
                eprintln!("❌ Pencere oluşturulamadı: {}", e);
                return Err(e.into());
            }
        }
        
        // Pencere pozisyonunu ayarla - hata yakalama ile
        if let Err(e) = highgui::move_window(&self.window_name, 100, 100) {
            eprintln!("⚠️  Pencere pozisyonu ayarlanamadı: {}", e);
            // Bu kritik değil, devam edelim
        }
        
        println!("💡 ESC tuşu ile kapatabilirsiniz");
        println!("💡 Spacebar tuşu ile bilgi overlay'ini açıp kapatabilirsiniz");
        
        Ok(())
    }
    
    /// Frame'i gelişmiş bilgilerle göster
    pub fn show_frame_with_info(
        &mut self,
        frame: &mut Mat,
        detections: &[Detection],
        stats: &DetectionStats,
    ) -> Result<bool> {
        if !self.window_created {
            // Pencere oluşturulmamışsa, tekrar dene
            if let Err(_) = self.setup_window() {
                // Pencere oluşturulamazsa sessizce devam et
                return Ok(true);
            }
        }
        
        // FPS hesapla
        let current_fps = self.calculate_fps();
        
        // Yüz tespitlerini çiz
        if let Err(e) = self.draw_face_detections(frame, detections) {
            eprintln!("⚠️  Yüz çizim hatası: {}", e);
        }
        
        // Bilgi overlay'ini çiz
        if self.show_overlay {
            if let Err(e) = self.draw_info_overlay(frame, current_fps, detections, stats) {
                eprintln!("⚠️  Overlay çizim hatası: {}", e);
            }
        }
        
        // Frame'i göster
        match highgui::imshow(&self.window_name, frame) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("⚠️  Frame gösterim hatası: {}", e);
                eprintln!("💡 OpenCV pencere sistemi macOS'ta sorun yaşıyor olabilir");
                return Ok(false); // Çıkış sinyali
            }
        }
        
        // Klavye girişini kontrol et - güvenli şekilde
        let key = match highgui::wait_key(1) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("⚠️  Klavye okuma hatası: {}", e);
                return Ok(true); // Devam et
            }
        };
        
        // ESC tuşu kontrolü
        if key == 27 {
            println!("👋 ESC tuşu basıldı, pencere kapatılıyor...");
            return Ok(false); // Çıkış sinyali
        }
        
        // Spacebar ile overlay açma/kapama
        if key == 32 {
            self.show_overlay = !self.show_overlay;
            println!("🔄 Overlay {}", if self.show_overlay { "açıldı" } else { "kapatıldı" });
        }
        
        Ok(true) // Devam et
    }
    
    /// Yüz tespitlerini çiz
    fn draw_face_detections(&self, frame: &mut Mat, detections: &[Detection]) -> Result<()> {
        for (i, detection) in detections.iter().enumerate() {
            let rect = detection.face_rect;
            
            // Yüz dikdörtgeni (yeşil)
            imgproc::rectangle(
                frame,
                rect,
                Scalar::new(0.0, 255.0, 0.0, 0.0), // Yeşil
                3,
                LINE_8,
                0,
            )?;
            
            // Yüz merkezi (kırmızı nokta)
            let center = Point::new(
                rect.x + rect.width / 2,
                rect.y + rect.height / 2,
            );
            
            imgproc::circle(
                frame,
                center,
                8,
                Scalar::new(0.0, 0.0, 255.0, 0.0), // Kırmızı
                -1,
                LINE_8,
                0,
            )?;
            
            // ⭐ YENİ: Alnın çatısına nişangah çiz
            self.draw_forehead_crosshair(frame, &rect)?;
            
            // Yüz numarası ve boyut bilgisi
            let text = format!("Yüz {} ({}x{})", 
                i + 1, 
                rect.width, 
                rect.height
            );
            
            let text_pos = Point::new(
                rect.x,
                rect.y - 10.max(rect.y - 30)
            );
            
            // Metin arkaplanı (siyah)
            let text_size = imgproc::get_text_size(
                &text,
                FONT_HERSHEY_SIMPLEX,
                0.6,
                2,
                &mut 0,
            )?;
            
            imgproc::rectangle(
                frame,
                Rect::new(
                    text_pos.x - 5,
                    text_pos.y - text_size.height - 5,
                    text_size.width + 10,
                    text_size.height + 10,
                ),
                Scalar::new(0.0, 0.0, 0.0, 0.0), // Siyah
                -1,
                LINE_8,
                0,
            )?;
            
            // Metin (beyaz)
            imgproc::put_text(
                frame,
                &text,
                text_pos,
                FONT_HERSHEY_SIMPLEX,
                0.6,
                Scalar::new(255.0, 255.0, 255.0, 0.0), // Beyaz
                2,
                LINE_8,
                false,
            )?;
        }
        
        Ok(())
    }
    
    /// Alnın çatısına nişangah çiz (video display için)
    fn draw_forehead_crosshair(&self, frame: &mut Mat, face_rect: &Rect) -> Result<()> {
        // Alın çatısının konumunu hesapla (yüzün üst ortası)
        let forehead_center = Point::new(
            face_rect.x + face_rect.width / 2,    // Yatay orta
            face_rect.y + (face_rect.height as f32 * 0.15) as i32  // Alnın çatısı (yüzün %15'i aşağıda)
        );
        
        // Nişangah boyutlarını yüz boyutuna göre ayarla
        let face_size = (face_rect.width + face_rect.height) / 2;
        let outer_radius = (face_size as f32 * 0.08) as i32; // Yüz boyutunun %8'i
        let inner_radius = (outer_radius as f32 * 0.25) as i32; // Dış yarıçapın %25'i
        
        // Nişangah renkleri - video'da daha parlak renkler
        let crosshair_color = Scalar::new(0.0, 255.0, 255.0, 0.0); // Sarı (dikkat çekici)
        let center_color = Scalar::new(0.0, 0.0, 255.0, 0.0);      // Kırmızı (merkez nokta)
        
        // Dış çember (kalın çizgi - video'da daha görünür)
        imgproc::circle(
            frame,
            forehead_center,
            outer_radius,
            crosshair_color,
            3, // Kalın çizgi
            LINE_8,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Nişangah çemberi çizilemedi: {}", e)))?;
        
        // İç çember (dolu merkez nokta)
        imgproc::circle(
            frame,
            forehead_center,
            inner_radius,
            center_color,
            -1, // Dolu çember
            LINE_8,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Nişangah merkezi çizilemedi: {}", e)))?;
        
        // Çapraz çizgiler (daha kalın - video'da görünürlük için)
        let cross_length = outer_radius / 2;
        
        // Yatay çizgi
        imgproc::line(
            frame,
            Point::new(forehead_center.x - cross_length, forehead_center.y),
            Point::new(forehead_center.x + cross_length, forehead_center.y),
            crosshair_color,
            2, // Kalın çizgi
            LINE_8,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Yatay çizgi çizilemedi: {}", e)))?;
        
        // Dikey çizgi
        imgproc::line(
            frame,
            Point::new(forehead_center.x, forehead_center.y - cross_length),
            Point::new(forehead_center.x, forehead_center.y + cross_length),
            crosshair_color,
            2, // Kalın çizgi
            LINE_8,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Dikey çizgi çizilemedi: {}", e)))?;
        
        // İsteğe bağlı: Nişangah etrafına daha büyük bir halka (hedef hissi için)
        imgproc::circle(
            frame,
            forehead_center,
            outer_radius + 8,
            Scalar::new(0.0, 255.0, 0.0, 0.0), // Yeşil dış halka
            1, // İnce çizgi
            LINE_8,
            0,
        ).map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Dış halka çizilemedi: {}", e)))?;
        
        Ok(())
    }
    
    /// Bilgi overlay'ini çiz
    fn draw_info_overlay(
        &self,
        frame: &mut Mat,
        fps: f64,
        detections: &[Detection],
        stats: &DetectionStats,
    ) -> Result<()> {
        let frame_height = frame.rows();
        let frame_width = frame.cols();
        
        // Overlay arkaplanı (semi-transparent)
        let overlay_height = 120;
        imgproc::rectangle(
            frame,
            Rect::new(10, 10, 400, overlay_height),
            Scalar::new(0.0, 0.0, 0.0, 0.0),
            -1,
            LINE_8,
            0,
        )?;
        
        // Bilgi metinleri
        let info_texts = vec![
            format!("🎯 Auto-Aim - Yüz Tespit Sistemi"),
            format!("📊 FPS: {:.1}", fps),
            format!("👥 Tespit Edilen: {} yüz", detections.len()),
            format!("📈 Toplam Frame: {}", stats.total_frames),
            format!("🎯 Toplam Tespit: {}", stats.faces_detected),
            format!("📋 Spacebar: Overlay açma/kapama"),
        ];
        
        for (i, text) in info_texts.iter().enumerate() {
            let y_pos = 35 + (i as i32 * 18);
            
            imgproc::put_text(
                frame,
                text,
                Point::new(20, y_pos),
                FONT_HERSHEY_SIMPLEX,
                0.5,
                Scalar::new(255.0, 255.0, 255.0, 0.0), // Beyaz
                1,
                LINE_8,
                false,
            )?;
        }
        
        // Sağ alt köşede sistem durumu
        let status_text = if detections.is_empty() {
            "🔍 Yüz aranıyor..."
        } else {
            "✅ Yüz tespit edildi!"
        };
        
        let status_pos = Point::new(
            frame_width - 200,
            frame_height - 20,
        );
        
        imgproc::put_text(
            frame,
            status_text,
            status_pos,
            FONT_HERSHEY_SIMPLEX,
            0.6,
            if detections.is_empty() {
                Scalar::new(0.0, 255.0, 255.0, 0.0) // Sarı
            } else {
                Scalar::new(0.0, 255.0, 0.0, 0.0) // Yeşil
            },
            2,
            LINE_8,
            false,
        )?;
        
        Ok(())
    }
    
    /// FPS hesapla
    fn calculate_fps(&mut self) -> f64 {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time).as_secs_f64();
        
        if delta > 0.0 {
            let current_fps = 1.0 / delta;
            self.fps_history.push_back(current_fps);
            
            // Son 30 frame'in ortalamasını al
            if self.fps_history.len() > 30 {
                self.fps_history.pop_front();
            }
        }
        
        self.last_frame_time = now;
        
        // Ortalama FPS hesapla
        if self.fps_history.is_empty() {
            0.0
        } else {
            self.fps_history.iter().sum::<f64>() / self.fps_history.len() as f64
        }
    }
    
    /// Overlay'i aç/kapat
    pub fn toggle_overlay(&mut self) {
        self.show_overlay = !self.show_overlay;
    }
    
    /// Pencereyi kapat
    pub fn close(&self) -> Result<()> {
        highgui::destroy_window(&self.window_name)?;
        println!("🖼️  Video penceresi kapatıldı");
        Ok(())
    }
}

impl Drop for VideoDisplay {
    fn drop(&mut self) {
        let _ = self.close();
    }
} 