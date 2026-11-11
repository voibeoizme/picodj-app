# PicoDJ

Phần mềm DJ cho **bàn DJ tự chế dùng Raspberry Pi Pico** (firmware ở repo [`picodj`](https://github.com/voibeoizme/picodj)).

PicoDJ được xây trên mã nguồn **[Mixxx](https://mixxx.org) 2.5.6**, phần mềm DJ mã nguồn mở. Engine âm thanh, phân tích BPM/key, hiệu ứng và thư viện nhạc lấy từ Mixxx. PicoDJ bổ sung:

- mapping cho bàn **Bàn DJ ver 2** (`res/controllers/PicoDJ Ban DJ ver 2.midi.xml` và `PicoDJ-scripts.js`), có sẵn phần đèn LED cho bàn ver 3;
- quy trình build tự động ra file cài Windows (`.github/workflows/build-windows.yml`);
- *(đang làm)* skin giao diện PicoDJ theo bố cục Rekordbox 6, import thư viện Rekordbox XML.

## Tải bản cài Windows

1. Vào tab **Actions** của repo → chọn lần chạy **Build PicoDJ for Windows** mới nhất có dấu ✅.
2. Kéo xuống mục **Artifacts** → tải **PicoDJ-Windows-Installer** (file `.zip`, bên trong là file `.msi`).
3. Giải nén và chạy file `.msi`. Bản build chưa được ký số, nên Windows SmartScreen có thể cảnh báo: bấm **More info → Run anyway**.

Muốn build lại bằng tay: tab **Actions** → **Build PicoDJ for Windows** → **Run workflow**. Mỗi lần build mất khoảng 1–1,5 giờ.

## Dùng bàn DJ với PicoDJ

1. Cắm bàn vào máy tính qua USB.
2. Mở PicoDJ → **Options → Preferences → Controllers** → chọn thiết bị MIDI của bàn ở cột trái.
3. Ở mục **Load Mapping**, chọn **PicoDJ Bàn DJ ver 2** → tick **Enabled** → **Apply**.

| Phần cứng | MIDI | Chức năng |
|---|---|---|
| Nút 0 / 4 | Note 60 / 64 | Pitch Bend − (deck 1 / 2) |
| Nút 13 / 14 | Note 73 / 74 | Pitch Bend + |
| Nút 1 / 5 | Note 61 / 65 | Sync |
| Nút 2 / 8 | Note 62 / 68 | Cue (giữ để nghe thử) |
| Nút 3 / 9 | Note 63 / 69 | Play / Pause |
| Nút 6 / 7 | Note 66 / 67 | Load bài đang chọn |
| MUX CH0–2 / CH4–6 | CC10–12 / CC14–16 | EQ High / Mid / Low |
| MUX CH3 / CH7 | CC13 / CC17 | Color FX (Quick Effect) |
| Fader 1 / 2 | CC22 / CC23 | Channel fader |
| Nút 10 | Note 70 | Đổi Color FX |
| MUX CH9 + Nút 12 | CC19 + Note 72 | FX1 hiệu ứng 1: mức / bật-tắt |
| MUX CH8 + Nút 11 | CC18 + Note 71 | FX2 hiệu ứng 1: mức / bật-tắt |
| Encoder EC11 | CC30 (1 / 127) | Cuộn thư viện |
| Nút encoder | Note 75 | Chuyển giữa cây thư mục và danh sách bài |

> **Bluetooth MIDI (Pico W):** Windows chưa tự nhận BLE MIDI cho các app dùng WinMM. Tạm thời dùng USB, hoặc dùng cầu nối như *MIDIberry* + *loopMIDI*.

## Giấy phép

PicoDJ là bản phái sinh của Mixxx nên dùng giấy phép **GNU GPL phiên bản 2 trở lên** (xem `LICENSE`). Dùng cá nhân thì thoải mái. Nếu phát hành bản build cho người khác, bạn phải cung cấp kèm mã nguồn (repo này) theo GPL.

Mọi công lao về engine thuộc về đội ngũ Mixxx. README gốc của Mixxx nằm ở [`README.mixxx.md`](README.mixxx.md).
