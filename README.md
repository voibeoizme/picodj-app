# PicoDJ

Phần mềm DJ nhẹ cho **bàn DJ tự chế dùng Raspberry Pi Pico** (firmware ở repo [`picodj`](https://github.com/voibeoizme/picodj)).

- **2 deck**: Play/Cue (giữ Cue để nghe thử), Sync, Master, tempo ±6/10/16 %, jog, hot cue A–H, auto loop, quantize.
- **Mixer**: Trim, EQ 3 băng (−26 dB … +6 dB), fader có đồng hồ mức tín hiệu, crossfader có gán kênh A/B.
- **FX**: mỗi bên FX1/FX2 có 3 slot (Delay, Echo, Spiral, Reverb, Flanger) cộng Vinyl Brake. Color FX có Filter, Dub Echo, Space, Crush, Noise.
- **Thư viện**: nhiều thư mục nhạc, playlist riêng, **import Rekordbox XML** (gồm beatgrid, hot cue và playlist), đọc tag ID3 kèm ảnh bìa, rating, tìm kiếm, sắp xếp.
- **Phân tích**: BPM và beatgrid, key theo Camelot, waveform 3 băng tần.
- **Bàn DJ**: nhận mọi thiết bị MIDI, tự kết nối lại khi cắm lại. Có sẵn đèn LED cho Play/Cue/Sync/FX khi bàn có đèn.

Ứng dụng viết bằng [Tauri 2](https://tauri.app): phần giao diện ở `ui/index.html`, phần native viết bằng Rust ở `src-tauri/` (lo MIDI, quét thư mục, đọc file). File cài chỉ vài MB và dùng WebView2 có sẵn trong Windows 10/11.

## Tải bản cài Windows

1. Vào tab **Actions** → chọn lần chạy **Build PicoDJ for Windows** mới nhất có dấu ✅.
2. Ở mục **Artifacts**, tải **PicoDJ-Windows** (file `.zip`).
3. Trong file zip có hai bản cài, dùng bản nào cũng được: `PicoDJ_x.y.z_x64-setup.exe` hoặc `PicoDJ_x.y.z_x64_en-US.msi`.
4. Bản build chưa được ký số, nên Windows SmartScreen có thể cảnh báo: bấm **More info → Run anyway**.

## Dùng lần đầu

1. Bấm **THƯ MỤC NHẠC** trên thanh trên cùng để thêm thư mục nhạc. Thêm được nhiều thư mục, app nhớ lại ở lần mở sau.
2. *(Tuỳ chọn)* Bấm **IMPORT XML**, chọn file Rekordbox xuất từ **File → Export Collection in xml format**.
3. Cắm bàn DJ qua USB: chữ **MIDI** chuyển sang màu xanh là app đã nhận bàn.
4. **F11** để bật/tắt toàn màn hình.

## Phát triển

```bash
npm install
npx tauri dev      # chạy thử
npx tauri build    # tạo bản cài
```

Lộ trình: chuyển engine âm thanh sang Rust (cpal) để có độ trễ thấp hơn và giữ tông khi đổi tempo; viết lại thuật toán EQ, filter, beat detection và key detection theo [Mixxx](https://github.com/mixxxdj/mixxx); làm bản Android.

## Giấy phép

GPL-2.0-or-later. Các thuật toán chuyển từ Mixxx giữ nguyên giấy phép GPL của Mixxx.

Bản fork Mixxx làm trước đây vẫn còn ở nhánh [`mixxx-fork`](../../tree/mixxx-fork).
