# PicoDJ

Phần mềm DJ nhẹ cho **bàn DJ tự chế dùng Raspberry Pi Pico** (firmware ở repo [`picodj`](https://github.com/voibeoizme/picodj)).

- **2 deck**: Play/Cue (giữ Cue để nghe thử), Sync, Master, tempo ±6/10/16 %, jog, auto loop, quantize, beat jump.
  - **MT (Master Tempo)**: giữ nguyên tông khi đổi tempo. **KEY SYNC**: đổi tông cho hợp key deck kia.
  - **SLIP**: khi bỏ loop, thả hot cue hoặc thả slicer, nhạc chạy tiếp từ chỗ lẽ ra đang phát (vạch xanh SLIP trên waveform).
  - **Pad** (bấm chữ dưới pad để đổi chế độ): HOT CUE, PAD FX, SLICER, BEAT JUMP, BEAT LOOP, KEYBOARD, KEY SHIFT. PAD FX, BEAT JUMP, BEAT LOOP có 2 trang (nút ‹ ›).
- **Mixer**: Trim, EQ 3 băng (−26 dB … +6 dB), fader có đồng hồ mức tín hiệu, crossfader có gán kênh A/B.
- **Beat FX**: mỗi bên FX1/FX2 có 3 slot. Bấm tên hiệu ứng để bật/tắt, bấm ⇕ bên cạnh để chọn hiệu ứng khác trong 35 loại như Rekordbox (Delay, Echo, Spiral, Reverb, Rev Delay, MT Delay, Trans, Pan, Filter, Flanger, Phaser, Roll, Slip Roll, Rev Roll, Pitch, Robot, Enigma Jet, Ping Pong, Helix, Vinyl Brake, Stretch…). Có thêm Vinyl Brake để dừng deck.
- **Color FX**: DEFAULT chỉ có Filter. Chọn USER rồi bấm ⇕ để chọn Filter, Jet, Crush, Noise, Pitch, Space, Dub Echo, Sweep, Gate Comp. Nút tên CFX là nút bật/tắt.
- **Thư viện**: nhiều thư mục nhạc, playlist riêng, **import Rekordbox XML** (gồm beatgrid, hot cue và playlist), đọc tag ID3 kèm ảnh bìa, rating, tìm kiếm, sắp xếp.
- **Phân tích**: BPM và beatgrid, key theo Camelot, waveform 3 băng tần.
- **Bàn DJ**: nhận mọi thiết bị MIDI, tự kết nối lại khi cắm lại. Có sẵn đèn LED cho Play/Cue/Sync/FX khi bàn có đèn.

Ứng dụng viết bằng [Tauri 2](https://tauri.app): phần giao diện ở `ui/index.html`, phần native viết bằng Rust ở `src-tauri/` (lo MIDI, quét thư mục, đọc file). File cài chỉ vài MB và dùng WebView2 có sẵn trong Windows 10/11.

## Tải bản cài Windows

1. Vào tab **Actions** → chọn lần chạy **Build PicoDJ for Windows** mới nhất có dấu ✅.
2. Ở mục **Artifacts**, tải **PicoDJ-Windows** (file `.zip`).
3. Trong file zip có hai bản cài, dùng bản nào cũng được: `PicoDJ_x.y.z_x64-setup.exe` hoặc `PicoDJ_x.y.z_x64_en-US.msi`.
4. Bản build chưa được ký số, nên Windows SmartScreen có thể cảnh báo: bấm **More info → Run anyway**.

## Tải bản Android

1. Vào tab **Actions** → chọn lần chạy **Build PicoDJ for Android** mới nhất có dấu ✅ → mục **Artifacts** → tải **PicoDJ-Android**.
2. Giải nén, chép file `PicoDJ_x.y.z_arm64.apk` vào điện thoại và mở để cài (cho phép *cài ứng dụng không rõ nguồn gốc* khi máy hỏi).
3. Lần đầu mở, cho phép app đọc **nhạc và âm thanh**. App tự quét thư mục **Music** và **Download**. Muốn thêm chỗ khác thì bấm **THƯ MỤC NHẠC**.
4. Cắm bàn DJ bằng cáp **OTG**: chữ **MIDI** chuyển xanh là app đã nhận bàn.

Bài trong XML được tìm theo tên file trong các thư mục nhạc trên điện thoại, vì đường dẫn trong XML là của máy tính.
File ký số `tools/android/picodj-release.jks` chỉ dùng để tự cài, nhờ nó bản mới cài đè được lên bản cũ.

## Dùng lần đầu

Không bắt buộc chọn thư mục nhạc, app tự nhớ mọi thứ cho lần mở sau.

- **IMPORT XML**: chọn file Rekordbox xuất từ **File → Export Collection in xml format**. App phát thẳng từ đường dẫn ghi trong XML. Bấm lại nút này để import XML khác hoặc **xoá XML** đã import.
- **Kéo file nhạc vào app**: file được lưu vào thư viện riêng của app (mục *Nhạc đã kéo vào app*).
- **THƯ MỤC NHẠC** *(tuỳ chọn)*: thêm thư mục để app quét toàn bộ nhạc trong đó.
- Cắm bàn DJ qua USB: chữ **MIDI** chuyển sang màu xanh là app đã nhận bàn.
- **F11** để bật/tắt toàn màn hình.

## Phát triển

```bash
npm install
npx tauri dev      # chạy thử
npx tauri build    # tạo bản cài
```

Lộ trình: chuyển engine âm thanh sang Rust (cpal) để có độ trễ thấp hơn; viết lại thuật toán EQ, filter, beat detection và key detection theo [Mixxx](https://github.com/mixxxdj/mixxx); làm bản Android.

## Giấy phép

GPL-2.0-or-later. Các thuật toán chuyển từ Mixxx giữ nguyên giấy phép GPL của Mixxx.

Bản fork Mixxx làm trước đây vẫn còn ở nhánh [`mixxx-fork`](../../tree/mixxx-fork).
