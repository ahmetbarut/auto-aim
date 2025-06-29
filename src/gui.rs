use eframe::{egui, glow};
use opencv::{core::Mat, prelude::*};
use std::sync::{Arc, Mutex};
use crate::{
    camera::Camera,
    config::Config,
    detector::{FaceDetector, Detection, DetectionStats},
    error::Result,
    notification::NotificationSystem,
};

/// Modern GUI uygulaması
pub struct AutoAimApp {
    // Kamera ve tespit sistemi
    camera: Arc<Camera>,
    detector: Arc<FaceDetector>,
    notification_system: NotificationSystem,
    
    // GUI durumu
    current_frame: Option<egui::TextureHandle>,
    frame_size: egui::Vec2,
    detections: Vec<Detection>,
    stats: Arc<Mutex<DetectionStats>>,
    config: Config,
    
    // GUI kontrolleri
    show_detections: bool,
    show_stats: bool,
    detection_sensitivity: f32,
    
    // Sistem durumu
    is_running: bool,
    fps_counter: f32,
    last_frame_time: std::time::Instant,
}

impl AutoAimApp {
    /// Yeni GUI uygulaması oluştur
    pub fn new(
        camera: Arc<Camera>,
        detector: Arc<FaceDetector>,
        notification_system: NotificationSystem,
        config: Config,
    ) -> Self {
        let stats = Arc::new(Mutex::new(DetectionStats::new()));
        
        Self {
            camera,
            detector,
            notification_system,
            current_frame: None,
            frame_size: egui::Vec2::new(640.0, 480.0),
            detections: Vec::new(),
            stats,
            config,
            show_detections: true,
            show_stats: true,
            detection_sensitivity: 0.5,
            is_running: false,
            fps_counter: 0.0,
            last_frame_time: std::time::Instant::now(),
        }
    }
    
    /// Kamera ve tespit sistemini başlat
    pub async fn start_systems(&mut self) -> Result<()> {
        // Kamerayı başlat
        self.camera.start().await?;
        self.is_running = true;
        
        // Frame işleme task'ını başlat
        let camera_clone = Arc::clone(&self.camera);
        let detector_clone = Arc::clone(&self.detector);
        let stats_clone = Arc::clone(&self.stats);
        
        tokio::spawn(async move {
            let mut frame_receiver = camera_clone.subscribe();
            
            while let Ok(frame) = frame_receiver.recv().await {
                // Yüz tespiti yap
                if let Ok(detections) = detector_clone.detect_faces(&frame).await {
                    // İstatistikleri güncelle
                    if let Ok(mut stats_guard) = stats_clone.lock() {
                        stats_guard.update(detections.len());
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Ana kontrol panelini çiz
    fn draw_control_panel(&mut self, ctx: &egui::Context) {
        egui::Window::new("🎯 Auto-Aim Kontrol Paneli")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Sistem Durumu");
                
                // Sistem durumu göstergesi
                ui.horizontal(|ui| {
                    let color = if self.is_running { 
                        egui::Color32::GREEN 
                    } else { 
                        egui::Color32::RED 
                    };
                    
                    ui.colored_label(color, "●");
                    ui.label(if self.is_running { 
                        "Çalışıyor" 
                    } else { 
                        "Durduruldu" 
                    });
                });
                
                ui.separator();
                
                // Kontroller
                ui.heading("Görüntü Ayarları");
                ui.checkbox(&mut self.show_detections, "Yüz tespitlerini göster");
                ui.checkbox(&mut self.show_stats, "İstatistikleri göster");
                
                ui.separator();
                
                // Tespit hassasiyeti
                ui.heading("Tespit Ayarları");
                ui.add(
                    egui::Slider::new(&mut self.detection_sensitivity, 0.1..=1.0)
                        .text("Hassasiyet")
                        .show_value(true)
                );
                
                ui.separator();
                
                // Sistem kontrolleri
                ui.heading("Sistem Kontrolleri");
                
                if ui.button("🔄 Sistemi Yeniden Başlat").clicked() {
                    // Sistem yeniden başlatma mantığı
                }
                
                if ui.button("📸 Anlık Görüntü Al").clicked() {
                    // Ekran görüntüsü alma mantığı
                }
                
                if ui.button("⚙️ Ayarları Kaydet").clicked() {
                    // Ayarları kaydetme mantığı
                }
            });
    }
    
    /// İstatistik panelini çiz
    fn draw_stats_panel(&self, ctx: &egui::Context) {
        if !self.show_stats {
            return;
        }
        
        egui::Window::new("📊 İstatistikler")
            .default_width(250.0)
            .show(ctx, |ui| {
                if let Ok(stats_guard) = self.stats.lock() {
                    ui.label(format!("FPS: {:.1}", self.fps_counter));
                    ui.label(format!("Toplam Frame: {}", stats_guard.total_frames));
                    ui.label(format!("Tespit Edilen Yüz: {}", stats_guard.faces_detected));
                    ui.label(format!("Tespit Oranı: {:.1}%", stats_guard.detection_rate));
                    ui.label(format!("Tespit Oranı: {:.1}%", stats_guard.detection_rate));
                    ui.label(format!("Sistem FPS: {:.1}", stats_guard.get_fps()));
                    
                    ui.separator();
                    
                    // Grafik gösterimi (basit progress bar)
                    ui.label("Tespit Performansı:");
                    ui.add(
                        egui::ProgressBar::new((stats_guard.detection_rate / 100.0) as f32)
                            .text(format!("{:.1}%", stats_guard.detection_rate))
                    );
                }
            });
    }
    
    /// Ana video panelini çiz
    fn draw_video_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("📹 Canlı Video Görüntüsü");
            
            // Video frame'ini göster
            if let Some(ref texture) = self.current_frame {
                let available_size = ui.available_size();
                
                // Görüntüyü uygun boyuta ölçekle
                let scale = (available_size.x / self.frame_size.x).min(available_size.y / self.frame_size.y).min(1.0);
                let scaled_size = self.frame_size * scale;
                
                ui.allocate_ui(scaled_size, |ui| {
                    ui.add(egui::Image::from_texture(texture).fit_to_exact_size(scaled_size));
                });
                
                // Tespit bilgilerini göster
                if self.show_detections && !self.detections.is_empty() {
                    ui.separator();
                    ui.label(format!("🎯 {} yüz tespit edildi", self.detections.len()));
                    
                    for (i, detection) in self.detections.iter().enumerate() {
                        ui.label(format!(
                            "  Yüz {}: {}x{} (Güven: {:.2})",
                            i + 1,
                            detection.face_rect.width,
                            detection.face_rect.height,
                            detection.confidence
                        ));
                    }
                }
            } else {
                // Kamera bekleniyor mesajı
                ui.centered_and_justified(|ui| {
                    ui.heading("📷 Kamera bekleniyor...");
                    ui.label("Kamera erişim izni verildiğinde video görüntülenecek");
                    
                    // Spinner efekti
                    ui.spinner();
                });
            }
        });
    }
    
    /// OpenCV Mat'ı egui texture'a çevir
    fn mat_to_texture(&self, ctx: &egui::Context, mat: &Mat) -> Result<egui::TextureHandle> {
        // Mat'ı RGB formatına çevir
        let mut rgb_mat = Mat::default();
        opencv::imgproc::cvt_color(mat, &mut rgb_mat, opencv::imgproc::COLOR_BGR2RGB, 0, opencv::core::AlgorithmHint::ALGO_HINT_ACCURATE)?;
        
        // Mat boyutlarını al
        let rows = rgb_mat.rows();
        let cols = rgb_mat.cols();
        
        // Mat'tan raw bytes'a çevir
        let data = rgb_mat.data_bytes()
            .map_err(|e| crate::error::AutoAimError::ImageProcessingError(format!("Mat data okuma hatası: {}", e)))?;
        
        // egui ColorImage oluştur
        let color_image = egui::ColorImage::from_rgb([cols as usize, rows as usize], data);
        
        // Texture oluştur
        let texture = ctx.load_texture("camera_frame", color_image, egui::TextureOptions::default());
        
        Ok(texture)
    }
    
    /// FPS hesapla
    fn update_fps(&mut self) {
        let now = std::time::Instant::now();
        let delta = now.duration_since(self.last_frame_time).as_secs_f32();
        
        if delta > 0.0 {
            self.fps_counter = 0.9 * self.fps_counter + 0.1 * (1.0 / delta);
        }
        
        self.last_frame_time = now;
    }
}

impl eframe::App for AutoAimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // FPS güncelle
        self.update_fps();
        
        // Sürekli yeniden çizim için
        ctx.request_repaint();
        
        // Panelleri çiz
        self.draw_control_panel(ctx);
        self.draw_stats_panel(ctx);
        self.draw_video_panel(ctx);
        
        // Kamera frame'ini güncelle (simüle edilmiş)
        // Bu kısım gerçek implementasyonda kamera frame'ini alacak
        // Şimdilik boş bırakıyoruz çünkü async context'te değiliz
    }
    
    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        // Temizlik işlemleri
        if self.is_running {
            let _ = self.camera.stop();
        }
    }
}

/// GUI uygulamasını başlat
pub async fn run_gui_app(
    camera: Arc<Camera>,
    detector: Arc<FaceDetector>,
    notification_system: NotificationSystem,
    config: Config,
) -> Result<()> {
    let mut app = AutoAimApp::new(camera, detector, notification_system, config);
    
    // Sistemleri başlat
    app.start_systems().await?;
    
    // GUI'yi başlat
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("🎯 Auto-Aim - Gerçek Zamanlı Yüz Tespit Sistemi"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Auto-Aim",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )?;
    
    Ok(())
} 