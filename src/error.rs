use thiserror::Error;

#[derive(Error, Debug)]
pub enum AutoAimError {
    #[error("Kamera hatası: {0}")]
    CameraError(String),
    
    #[error("OpenCV hatası: {0}")]
    OpenCvError(String),
    
    #[error("Görüntü işleme hatası: {0}")]
    ImageProcessingError(String),
    
    #[error("Konfigürasyon hatası: {0}")]
    ConfigError(String),
    
    #[error("IO hatası: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Bilinmeyen hata: {0}")]
    UnknownError(String),
}

// OpenCV Error için From trait implementation'ı ekle
impl From<opencv::Error> for AutoAimError {
    fn from(error: opencv::Error) -> Self {
        AutoAimError::OpenCvError(error.to_string())
    }
}

// eframe Error için From trait implementation'ı ekle
impl From<eframe::Error> for AutoAimError {
    fn from(error: eframe::Error) -> Self {
        AutoAimError::UnknownError(format!("GUI hatası: {}", error))
    }
}

pub type Result<T> = std::result::Result<T, AutoAimError>; 