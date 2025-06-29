mod error;
mod config;
mod camera;
mod detector;
mod notification;
mod gui;
mod video_display;

use clap::{Arg, Command, Parser, Subcommand};
use std::sync::Arc;
use tokio::signal;
use std::path::PathBuf;

use crate::{
    camera::Camera,
    config::Config,
    detector::{FaceDetector, DetectionStats, Detection},
    error::{AutoAimError, Result},
    notification::NotificationSystem,
    gui::run_gui_app,
    video_display::VideoDisplay,
};
use opencv::core::Mat;

#[derive(Parser)]
#[command(name = "auto-aim")]
#[command(about = "🎯 Auto-Aim - Gerçek Zamanlı Yüz Tespit Sistemi")]
struct Cli {
    #[command(subcommand)]
    mode: Option<Mode>,
    
    /// Konfigürasyon dosyası yolu
    #[arg(long, short)]
    config: Option<PathBuf>,
    
    /// Debug mod
    #[arg(long, short)]
    debug: bool,
    
    /// Video görüntüleme penceresi aç
    #[arg(long)]
    video: bool,
    
    /// Modern GUI arayüzü
    #[arg(long)]
    gui: bool,
    
    /// Terminal tabanlı mod (macOS uyumlu)
    #[arg(long)]
    terminal: bool,
}

#[derive(Subcommand)]
enum Mode {
    /// Kamera bilgilerini göster
    CameraInfo,
    /// Bildirim sistemini test et
    TestNotifications,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Logging başlat
    env_logger::init();
    
    let args = Cli::parse();
    
    println!("🎯 Auto-Aim - Gerçek Zamanlı Yüz Tespit Sistemi");
    println!("===============================================");
    
    // Config yükle
    let config_path = args.config.unwrap_or_else(|| "config.toml".into());
    let config = Config::load_or_create(&config_path)?;
    
    // Mod kontrolü
    match args.mode {
        Some(Mode::CameraInfo) => {
            return camera_info_mode(&config).await;
        }
        Some(Mode::TestNotifications) => {
            return test_notifications_mode(&config).await;
        }
        None => {
            // Ana modlar
            if args.gui {
                return gui_mode(config).await;
            } else if args.video {
                return video_mode(config, args.debug).await;
            } else if args.terminal {
                return terminal_mode(config, args.debug).await;
            } else {
                return basic_mode(config, args.debug).await;
            }
        }
    }
}

/// Konfigürasyonu yükle veya varsayılan olarak oluştur
async fn load_or_create_config(config_path: &str) -> Result<Config> {
    if std::path::Path::new(config_path).exists() {
        log::info!("Konfigürasyon dosyası yükleniyor: {}", config_path);
        let config = Config::load_from_file(config_path)?;
        config.validate()?;
        Ok(config)
    } else {
        log::info!("Konfigürasyon dosyası bulunamadı, varsayılan oluşturuluyor: {}", config_path);
        let config = Config::default();
        config.save_to_file(config_path)?;
        log::info!("Varsayılan konfigürasyon kaydedildi: {}", config_path);
        Ok(config)
    }
}

/// Kamera bilgilerini göster
async fn show_camera_info(config: &Config) -> Result<()> {
    println!("📷 Kamera Bilgileri:");
    println!("===================");
    
    let camera = Camera::new(config.clone())?;
    let info = camera.get_info()?;
    
    println!("Device ID: {}", info.device_id);
    println!("Çözünürlük: {}x{}", info.width, info.height);
    println!("FPS: {:.1}", info.fps);
    println!("Min yüz boyutu: {}x{}", config.min_face_size.0, config.min_face_size.1);
    println!("Max yüz boyutu: {}x{}", config.max_face_size.0, config.max_face_size.1);
    println!("Tespit hassasiyeti: {:.2}", config.detection_confidence);
    println!("Bildirim cooldown: {} ms", config.detection_cooldown_ms);
    
    Ok(())
}

/// Bildirim sistemini test et
async fn test_notifications(config: &Config) -> Result<()> {
    let notification_system = NotificationSystem::new(config.clone());
    notification_system.test_notifications().await
}

/// GUI modunu çalıştır
async fn run_gui_mode(config: Config) -> Result<()> {
    println!("🖥️  Modern GUI arayüzü başlatılıyor...");
    
    // Haar cascade dosyasını kontrol et ve indir
    FaceDetector::download_cascade_if_missing().await?;
    
    // Sistemleri oluştur
    let camera = Arc::new(Camera::new(config.clone())?);
    let detector = Arc::new(FaceDetector::new(config.clone())?);
    let notification_system = NotificationSystem::new(config.clone());
    
    println!("✅ Sistemler hazırlandı, GUI penceresi açılıyor...");
    
    // GUI uygulamasını çalıştır
    run_gui_app(camera, detector, notification_system, config).await?;
    
    Ok(())
}

/// Ana tespit sistemini çalıştır
async fn run_detection_system(config: Config, show_video: bool) -> Result<()> {
    println!("🚀 Sistem başlatılıyor...");
    
    // Kamerayı başlat
    let camera = Arc::new(Camera::new(config.clone())?);
    let _frame_receiver = camera.subscribe();
    camera.start().await?;
    
    println!("📷 Kamera başlatıldı (Device ID: {})", config.camera_id);
    
    // Yüz tespit ediciyi başlat
    let detector = Arc::new(FaceDetector::new(config.clone())?);
    let detection_receiver = detector.subscribe();
    
    println!("🎯 Yüz tespit sistemi hazır");
    
    // Bildirim sistemini başlat
    let notification_system = NotificationSystem::new(config.clone());
    
    println!("🔔 Bildirim sistemi aktif");
    
    // İstatistikleri başlat
    let stats = Arc::new(std::sync::Mutex::new(DetectionStats::new()));
    
    // Gelişmiş video görüntü sistemi
    let mut video_display = if show_video {
        Some(VideoDisplay::new("🎯 Auto-Aim - Gerçek Zamanlı Yüz Tespit")?)
    } else {
        None
    };
    
    println!();
    println!("✅ Sistem hazır! Yüz tespit edildiğinde bildirim gönderilecek.");
    println!("💡 Çıkmak için Ctrl+C tuşlayın");
    println!();
    
    // Ana işlem döngüsü
    let camera_clone = Arc::clone(&camera);
    let detector_clone = Arc::clone(&detector);
    
    // Video display için frame sharing
    let (video_tx, video_rx) = if show_video {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<(Mat, Vec<Detection>)>();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };
    
    // Frame işleme task'ı
    let stats_clone = Arc::clone(&stats);
    let processing_task: tokio::task::JoinHandle<std::result::Result<(), AutoAimError>> = tokio::spawn(async move {
        let mut frame_receiver = camera_clone.subscribe();
        
        loop {
            match frame_receiver.recv().await {
                Ok(mut frame) => {
                    // Yüz tespiti yap
                    match detector_clone.detect_faces(&frame).await {
                        Ok(detections) => {
                            if let Ok(mut stats_guard) = stats_clone.lock() {
                                stats_guard.update(detections.len());
                            }
                            
                            // Video display için frame gönder (sadece frame ve detections)
                            if let Some(ref tx) = video_tx {
                                let _ = tx.send((frame.clone(), detections));
                            }
                            
                            // İstatistikleri periyodik olarak yazdır
                            if config.debug_mode {
                                if let Ok(stats_guard) = stats_clone.lock() {
                                    if stats_guard.total_frames % 100 == 0 {
                                        log::debug!(
                                            "📊 Frame: {}, Tespit: {}, Oran: {:.1}%, FPS: {:.1}",
                                            stats_guard.total_frames,
                                            stats_guard.faces_detected,
                                            stats_guard.detection_rate,
                                            stats_guard.get_fps()
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Tespit hatası: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Frame alma hatası: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
        
        Ok(())
    });
    
    // Bildirim dinleme task'ı
    let notification_task = tokio::spawn(async move {
        notification_system.start_listening(detection_receiver).await
    });
    
    // Video display task'ı (OpenCV UI ana thread'de çalışmalı)
    let video_task = if let (Some(mut display), Some(mut rx)) = (video_display, video_rx) {
        let stats_for_video = Arc::clone(&stats);
        
        Some(tokio::task::spawn_blocking(move || -> Result<()> {
            loop {
                // Frame al
                if let Some((mut frame, detections)) = rx.blocking_recv() {
                    // Stats'ı al
                    if let Ok(stats_guard) = stats_for_video.lock() {
                        // Frame'i göster
                        match display.show_frame_with_info(&mut frame, &detections, &stats_guard) {
                            Ok(true) => continue, // Devam et
                            Ok(false) => break,   // ESC tuşu
                            Err(e) => {
                                log::error!("Video display hatası: {}", e);
                                break;
                            }
                        }
                    }
                } else {
                    break; // Channel kapatıldı
                }
            }
            Ok(())
        }))
    } else {
        None
    };
    
    // Signal handler (Ctrl+C)
    let signal_task = tokio::spawn(async move {
        signal::ctrl_c().await.expect("Ctrl+C signal handler başarısız");
        println!("\n🛑 Çıkış sinyali alındı, sistem durduruluyor...");
    });
    
    // Task'ları bekle
    if let Some(video_task) = video_task {
        tokio::select! {
            result = processing_task => {
                match result {
                    Ok(Ok(())) => log::info!("Frame işleme tamamlandı"),
                    Ok(Err(e)) => log::error!("Frame işleme hatası: {}", e),
                    Err(e) => log::error!("Task hatası: {}", e),
                }
            }
            result = notification_task => {
                match result {
                    Ok(Ok(())) => log::info!("Bildirim sistemi durduruldu"),
                    Ok(Err(e)) => log::error!("Bildirim sistemi hatası: {}", e),
                    Err(e) => log::error!("Bildirim task hatası: {}", e),
                }
            }
            result = video_task => {
                match result {
                    Ok(Ok(())) => log::info!("Video display tamamlandı"),
                    Ok(Err(e)) => log::error!("Video display hatası: {}", e),
                    Err(e) => log::error!("Video task hatası: {}", e),
                }
            }
            _ = signal_task => {
                log::info!("Çıkış sinyali işlendi");
            }
        }
    } else {
        tokio::select! {
            result = processing_task => {
                match result {
                    Ok(Ok(())) => log::info!("Frame işleme tamamlandı"),
                    Ok(Err(e)) => log::error!("Frame işleme hatası: {}", e),
                    Err(e) => log::error!("Task hatası: {}", e),
                }
            }
            result = notification_task => {
                match result {
                    Ok(Ok(())) => log::info!("Bildirim sistemi durduruldu"),
                    Ok(Err(e)) => log::error!("Bildirim sistemi hatası: {}", e),
                    Err(e) => log::error!("Bildirim task hatası: {}", e),
                }
            }
            _ = signal_task => {
                log::info!("Çıkış sinyali işlendi");
            }
        }
    }
    
    // Temizlik
    camera.stop()?;
    
    // Final istatistikler
    println!();
    println!("📊 Final İstatistikler:");
    println!("=======================");
    if let Ok(stats_guard) = stats.lock() {
        println!("Toplam frame: {}", stats_guard.total_frames);
        println!("Tespit edilen yüz: {}", stats_guard.faces_detected);
        println!("Tespit oranı: {:.1}%", stats_guard.detection_rate);
        println!("Ortalama FPS: {:.1}", stats_guard.get_fps());
        println!("Tespit oranı: {:.1}%", stats_guard.detection_rate);
    }
    
    println!();
    println!("👋 Auto-Aim sistemi kapatıldı. Güle güle!");
    
    Ok(())
}

/// Terminal tabanlı mod (macOS uyumlu - pencere olmadan)
async fn terminal_mode(config: Config, debug: bool) -> Result<()> {
    println!("💻 Terminal tabanlı mod başlatılıyor (macOS uyumlu)...");
    
    // Haar cascade dosyasını kontrol et ve indir
    FaceDetector::download_cascade_if_missing().await?;
    
    // Kamera başlat
    let camera = Arc::new(Camera::new(config.clone())?);
    let _frame_receiver = camera.subscribe();
    camera.start().await?;
    
    println!("📷 Kamera başlatıldı (Device ID: {})", config.camera_id);
    
    // Yüz tespit sistemi
    let detector = Arc::new(FaceDetector::new(config.clone())?);
    let detection_receiver = detector.subscribe();
    
    println!("🎯 Yüz tespit sistemi hazır");
    
    // Bildirim sistemi
    let notification_system = NotificationSystem::new(config.clone());
    
    println!("🔔 Bildirim sistemi aktif");
    println!("💻 Terminal modunda çalışıyor (pencere yok)");
    println!();
    println!("✅ Sistem hazır! Yüz tespit edildiğinde terminal'de bildirim gösterilecek.");
    println!("💡 Çıkmak için Ctrl+C tuşlayın");
    println!();
    
    // Ana işlem döngüsü
    let camera_clone = Arc::clone(&camera);
    let detector_clone = Arc::clone(&detector);
    
    // Frame işleme task'ı
    let processing_task = tokio::spawn(async move {
        let mut frame_receiver = camera_clone.subscribe();
        let mut frame_count = 0;
        let mut detection_count = 0;
        
        loop {
            match frame_receiver.recv().await {
                Ok(frame) => {
                    frame_count += 1;
                    
                    // Yüz tespiti yap
                    match detector_clone.detect_faces(&frame).await {
                        Ok(detections) => {
                            if !detections.is_empty() {
                                detection_count += detections.len();
                                
                                // Terminal'de güzel bir çıktı
                                println!("🔍 Frame #{}: {} yüz tespit edildi!", 
                                    frame_count, detections.len());
                                
                                for (i, detection) in detections.iter().enumerate() {
                                    println!("   └─ Yüz {}: {}x{} boyutunda", 
                                        i + 1, 
                                        detection.face_rect.width, 
                                        detection.face_rect.height
                                    );
                                }
                            }
                            
                            // Her 100 frame'de bir durum raporu
                            if frame_count % 100 == 0 {
                                let current_stats = detector_clone.get_stats();
                                println!("📊 Durum: {} frame işlendi, {} yüz tespit edildi, {} fotoğraf kaydedildi", 
                                    current_stats.total_frames, 
                                    current_stats.faces_detected,
                                    current_stats.saved_faces);
                            }
                        }
                        Err(e) => {
                            if debug {
                                eprintln!("⚠️  Yüz tespit hatası: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    if debug {
                        eprintln!("⚠️  Frame alma hatası: {}", e);
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    });
    
    // Bildirim dinleme task'ı
    let notification_task = tokio::spawn(async move {
        notification_system.start_listening(detection_receiver).await
    });
    
    // Signal handler (Ctrl+C)
    let signal_task = tokio::spawn(async move {
        signal::ctrl_c().await.expect("Ctrl+C signal handler başarısız");
        println!("\n🛑 Çıkış sinyali alındı, sistem durduruluyor...");
    });
    
    // Task'ları bekle
    tokio::select! {
        _ = processing_task => {
            println!("📹 Frame işleme tamamlandı");
        }
        _ = notification_task => {
            println!("🔔 Bildirim sistemi durduruldu");
        }
        _ = signal_task => {
            println!("✋ Kullanıcı çıkış istedi");
        }
    }
    
    // Final istatistikler - detector'dan al
    let final_stats = detector.get_stats();
    println!("📊 Final İstatistikler:");
    println!("   └─ İşlenen frame sayısı: {}", final_stats.total_frames);
    println!("   └─ Tespit edilen yüz sayısı: {}", final_stats.faces_detected);
    println!("   └─ Kaydedilen yüz fotoğrafı: {}", final_stats.saved_faces);
    println!("   └─ Tespit oranı: {:.1}%", final_stats.detection_rate);
    println!("   └─ Ortalama FPS: {:.1}", final_stats.get_fps());
    
    println!("👋 Sistem temiz şekilde kapatıldı.");
    
    Ok(())
}

/// Kamera bilgileri modu
async fn camera_info_mode(config: &Config) -> Result<()> {
    println!("📷 Kamera Bilgileri:");
    println!("===================");
    
    let camera = Camera::new(config.clone())?;
    let info = camera.get_info()?;
    
    println!("Device ID: {}", info.device_id);
    println!("Çözünürlük: {}x{}", info.width, info.height);
    println!("FPS: {:.1}", info.fps);
    println!("Min yüz boyutu: {}x{}", config.min_face_size.0, config.min_face_size.1);
    println!("Max yüz boyutu: {}x{}", config.max_face_size.0, config.max_face_size.1);
    println!("Tespit hassasiyeti: {:.2}", config.detection_confidence);
    println!("Bildirim cooldown: {} ms", config.detection_cooldown_ms);
    
    Ok(())
}

/// Bildirim test modu
async fn test_notifications_mode(config: &Config) -> Result<()> {
    let notification_system = NotificationSystem::new(config.clone());
    notification_system.test_notifications().await
}

/// GUI modu
async fn gui_mode(config: Config) -> Result<()> {
    println!("🖥️  Modern GUI arayüzü başlatılıyor...");
    
    // Haar cascade dosyasını kontrol et ve indir
    FaceDetector::download_cascade_if_missing().await?;
    
    // Sistemleri oluştur
    let camera = Arc::new(Camera::new(config.clone())?);
    let detector = Arc::new(FaceDetector::new(config.clone())?);
    let notification_system = NotificationSystem::new(config.clone());
    
    println!("✅ Sistemler hazırlandı, GUI penceresi açılıyor...");
    
    // GUI uygulamasını çalıştır
    run_gui_app(camera, detector, notification_system, config).await?;
    
    Ok(())
}

/// Video modu
async fn video_mode(config: Config, _debug: bool) -> Result<()> {
    // Haar cascade dosyasını kontrol et ve indir
    FaceDetector::download_cascade_if_missing().await?;
    
    // run_detection_system ile video display'i çalıştır
    run_detection_system(config, true).await
}

/// Temel mod
async fn basic_mode(config: Config, _debug: bool) -> Result<()> {
    // Haar cascade dosyasını kontrol et ve indir
    FaceDetector::download_cascade_if_missing().await?;
    
    // run_detection_system'i video olmadan çalıştır
    run_detection_system(config, false).await
}
