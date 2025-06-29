# 🎯 Auto-Aim - Gerçek Zamanlı Yüz Tespit Sistemi

Rust ile geliştirilmiş, gerçek zamanlı yüz tespit sistemi. Kameranızda insan kafası/yüz tespit ettiğinde anlık bildirim gönderir.

## ✨ Özellikler

- 🎥 **Gerçek Zamanlı Tespit**: Kameradan sürekli video akışı alarak yüz tespiti yapar
- 📱 **Çoklu Bildirim**: Konsol, sistem bildirimi, ses bildirimi ve terminal bell desteği
- ⚙️ **Konfigürasyon**: TOML dosyası ile tüm ayarlar özelleştirilebilir
- 🖥️ **Video Görüntüleme**: Opsiyonel olarak tespit edilen yüzleri video penceresinde gösterir
- 📊 **İstatistikler**: FPS, tespit oranı gibi detaylı istatistikler
- 🎛️ **Komut Satırı**: Zengin komut satırı arayüzü
- 🔄 **Async/Paralel**: Tokio async runtime ile yüksek performans

## 🛠️ Kurulum

### Gereksinimler

- Rust 1.70+
- OpenCV 4.x
- Kamera (webcam veya dahili kamera)

#### macOS

```bash
# Homebrew ile OpenCV kurulumu
brew install opencv

# veya MacPorts ile
sudo port install opencv4
```

#### Linux (Ubuntu/Debian)

```bash
# Sistem paketlerini yükle
sudo apt update
sudo apt install libopencv-dev clang libclang-dev

# Opsiyonel bildirim araçları
sudo apt install libnotify-bin espeak-ng # Linux bildirimleri için
```

#### Windows

1. [OpenCV](https://opencv.org/releases/) indirip yükleyin
2. `OPENCV_DIR` environment variable'ını ayarlayın
3. Visual Studio Build Tools yüklü olmalı

### Projeyi Klonlama ve Derleme

```bash
# Repo'yu klonla
git clone <repository-url>
cd auto-aim

# Projeyi derle
cargo build --release

# Veya direkt çalıştır
cargo run --release
```

## 🚀 Kullanım

### Temel Kullanım

```bash
# Sistemi başlat
cargo run --release

# Debug modu ile
cargo run --release -- --debug

# Video görüntüsü ile
cargo run --release -- --show-video
```

### Komut Satırı Seçenekleri

```bash
# Yardım
cargo run -- --help

# Konfigürasyon dosyası belirt
cargo run -- --config my-config.toml

# Kamera bilgilerini göster
cargo run -- --camera-info

# Bildirimleri test et
cargo run -- --test-notifications

# Video penceresini aç
cargo run -- --show-video

# Debug modu
cargo run -- --debug
```

## ⚙️ Konfigürasyon

İlk çalıştırmada `config.toml` dosyası otomatik oluşturulur:

```toml
# Kamera ayarları
camera_id = 0           # Kamera device ID (0 = varsayılan)
frame_width = 640       # Video genişlik
frame_height = 480      # Video yükseklik
fps = 30.0             # Frames per second

# Tespit ayarları
detection_confidence = 0.5              # Tespit hassasiyeti (0.1-1.0)
min_face_size = [30, 30]               # Minimum yüz boyutu [genişlik, yükseklik]
max_face_size = [300, 300]             # Maksimum yüz boyutu
detection_cooldown_ms = 1000           # Bildirimler arası minimum süre (ms)

# Sistem ayarları
notification_volume = 80               # Bildirim ses seviyesi (0-100)
debug_mode = false                    # Debug modu
```

## 🔔 Bildirim Türleri

Sistem 4 farklı bildirim yöntemi kullanır:

1. **Konsol Bildirimi**: Renkli terminal çıktısı
2. **Sistem Bildirimi**: İşletim sistemi bildirimi (macOS/Linux/Windows)
3. **Ses Bildirimi**: 
   - macOS: `say` komutu
   - Linux: `espeak` veya `beep`
   - Windows: System beep
4. **Terminal Bell**: ASCII bell karakteri (`\x07`)

## 📊 Örnek Çıktı

```
🎯 Auto-Aim - Gerçek Zamanlı Yüz Tespit Sistemi
===============================================
🚀 Sistem başlatılıyor...
📷 Kamera başlatıldı (Device ID: 0)
🎯 Yüz tespit sistemi hazır
🔔 Bildirim sistemi aktif

✅ Sistem hazır! Yüz tespit edildiğinde bildirim gönderilecek.
💡 Çıkmak için Ctrl+C tuşlayın

🎯 ===========================================
📸 YÜZ TESPİT SİSTEMİ UYARISI
⏰ Zaman: 14:23:45.123
📍 YÜZ TESPİT EDİLDİ! Konum: (245, 132) Boyut: 85x85
🎯 ===========================================
```

## 🏗️ Proje Yapısı

```
auto-aim/
├── src/
│   ├── main.rs          # Ana uygulama
│   ├── camera.rs        # Kamera yönetimi
│   ├── detector.rs      # Yüz tespit algoritmaları
│   ├── notification.rs  # Bildirim sistemi
│   ├── config.rs        # Konfigürasyon yönetimi
│   └── error.rs         # Hata türleri
├── Cargo.toml           # Rust dependencies
├── config.toml          # Uygulama konfigürasyonu (otomatik oluşur)
└── README.md
```

## 🔧 Geliştirme

### Hata Ayıklama

```bash
# Debug logları ile çalıştır
RUST_LOG=debug cargo run -- --debug

# Belirli modül logları
RUST_LOG=auto_aim::detector=debug cargo run

# Kamera problemleri için
cargo run -- --camera-info
```

### Test

```bash
# Bildirimleri test et
cargo run -- --test-notifications

# Kamera test
cargo run -- --show-video --debug
```

## 🎛️ Performans İpuçları

- Daha hızlı tespit için `frame_width` ve `frame_height` değerlerini düşürün
- `detection_confidence` değerini artırarak yanlış pozitifleri azaltın
- `detection_cooldown_ms` ile spam bildirimleri engelleyin
- Debug modu performansı düşürür, production'da kapatın

## 🐛 Sorun Giderme

### Yaygın Sorunlar

**OpenCV hatası:**
```bash
# macOS
export DYLD_LIBRARY_PATH=/opt/homebrew/lib:$DYLD_LIBRARY_PATH

# Linux
sudo ldconfig
```

**Kamera açılmıyor:**
```bash
# Kamera izinlerini kontrol et
cargo run -- --camera-info

# Başka camera ID dene
# config.toml'de camera_id = 1 yap
```

**Haar cascade dosyası bulunamıyor:**
Uygulama otomatik olarak indirecek, ancak manual indirmek için:
```bash
wget https://raw.githubusercontent.com/opencv/opencv/master/data/haarcascades/haarcascade_frontalface_alt.xml
```

## 🤝 Katkıda Bulunma

1. Repo'yu fork edin
2. Feature branch oluşturun (`git checkout -b feature/amazing-feature`)
3. Değişikliklerinizi commit edin (`git commit -m 'Add amazing feature'`)
4. Branch'i push edin (`git push origin feature/amazing-feature`)
5. Pull Request açın

## 📄 Lisans

Bu proje MIT lisansı altında lisanslanmıştır.

## 🙏 Teşekkürler

- [OpenCV](https://opencv.org/) - Computer vision kütüphanesi
- [Rust](https://www.rust-lang.org/) - Programlama dili
- [Tokio](https://tokio.rs/) - Async runtime

---

⚡ **Auto-Aim** ile gerçek zamanlı yüz tespit deneyimini yaşayın! 