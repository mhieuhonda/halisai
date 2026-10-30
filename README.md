# HalisAI

Ứng dụng chat AI ưu tiên điện thoại, tương thích máy tính. Giao diện tối kiểu terminal, hỗ trợ hiển thị quá trình suy nghĩ, gọi công cụ và streaming theo thời gian thực.

> Halis là trợ lý AI được tạo ra và huấn luyện bởi Hieu Louis.

## Tính năng

**Chat**
- Streaming câu trả lời theo thời gian thực (SSE)
- Khối "Đã suy nghĩ" gập/mở hiển thị quá trình reasoning
- Agent loop: AI tự gọi công cụ (`current_time`, `calculate`, `fetch_url`) khi cần rồi tiếp tục trả lời
- Markdown đầy đủ: code block kèm nút copy, bảng, danh sách, trích dẫn
- Dừng tạo giữa chừng (giữ lại phần đã tạo), tạo lại câu trả lời, đổi tên/xóa cuộc trò chuyện
- Đầu ra tối đa 32k token; câu trả lời bị cắt do chạm giới hạn sẽ được đánh dấu rõ

**Tài khoản**
- Đăng nhập bằng Google (OAuth 2.0, xác thực id_token với JWKS)
- Session lưu database, cookie HttpOnly + SameSite=Lax
- CSRF token cho mọi thao tác thay đổi dữ liệu

**Bảng quản trị** (`/admin`)
- Tổng quan: số liệu người dùng, online, cuộc trò chuyện, tin nhắn; biểu đồ tin nhắn theo ngày
- Quản lý người dùng: tìm kiếm, phân trang, trạng thái online/offline
- Cấm tài khoản theo thời hạn (1h/24h/7d/30d/vĩnh viễn) kèm lý do, bỏ cấm
- Thăng/hạ quyền giữa `user` → `moderator` → `admin` (chỉ admin)
- Đăng xuất tài khoản từ xa, xóa tài khoản (chỉ admin)
- Nhật ký thao tác (audit log) ghi lại mọi hành động quản trị
- Cấu hình: system prompt, thông báo toàn hệ thống

**Phân quyền**

| Quyền | User | Moderator | Admin |
|---|---|---|---|
| Chat | ✓ | ✓ | ✓ |
| Xem bảng quản trị | ✗ | ✓ | ✓ |
| Cấm/bỏ cấm user thường | ✗ | ✓ | ✓ |
| Đổi quyền, xóa tài khoản | ✗ | ✗ | ✓ |
| Cấu hình hệ thống | ✗ | ✗ | ✓ |

Người dùng đầu tiên đăng nhập sẽ tự động trở thành admin. Có thể chỉ định thêm qua biến `ADMIN_EMAILS`.

## Tech stack

| Thành phần | Phiên bản |
|---|---|
| Rust | 1.99.1 |
| Axum | 0.8.9 |
| SQLx | 0.9.0 |
| PostgreSQL | 18.6 |
| Askama | 0.16.1 |
| HTMX | 4.0.0 |
| Alpine.js | 3.17.4 |

Frontend không cần build step: HTMX + Alpine + marked + DOMPurify được vendor sẵn trong `static/vendor/`.

## Chạy với Docker

```bash
cp .env.example .env
# điền GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET
docker compose up --build
```

Mặc định app lắng nghe ở `0.0.0.0:3000`.

## Chạy thủ công

Yêu cầu: Rust 1.99.1 (rustup sẽ tự đọc `rust-toolchain.toml`), PostgreSQL 18.

```bash
# 1. Tạo database
createdb halisai

# 2. Cấu hình
cp .env.example .env
# điền DATABASE_URL, GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, APP_BASE_URL

# 3. Build và chạy (migration tự áp dụng khi khởi động)
cargo run --release
```

## Biến môi trường

| Biến | Mặc định | Mô tả |
|---|---|---|
| `APP_BASE_URL` | `http://localhost:3000` | URL công khai, dùng tạo redirect URI OAuth |
| `BIND_ADDR` | `0.0.0.0:3000` | Địa chỉ lắng nghe |
| `DATABASE_URL` | — | Chuỗi kết nối PostgreSQL |
| `GOOGLE_CLIENT_ID` | — | Client ID từ Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | — | Client secret (nên lấy từ GitHub Secrets / secret manager khi triển khai) |
| `AI_BASE_URL` | `https://api.kilo.ai/api/gateway` | Endpoint gateway AI |
| `AI_MODEL` | `kilo-auto/free` | Model sử dụng |
| `AI_MAX_OUTPUT_TOKENS` | `32768` | Giới hạn đầu ra |
| `ADMIN_EMAILS` | — | Danh sách email tự động lên admin, cách nhau bằng dấu phẩy |
| `DEV_LOGIN` | `false` | Bật endpoint `/dev/login` tạo user giả để test UI — **tắt ở production** |

## Cấu hình Google OAuth

1. Vào [Google Cloud Console](https://console.cloud.google.com/) → tạo OAuth client (loại Web application)
2. Thêm Authorized redirect URI: `{APP_BASE_URL}/auth/google/callback`
3. Điền client id/secret vào biến môi trường

## Cấu trúc dự án

```
src/
  main.rs          # Khởi động, router, middleware bảo mật
  config.rs        # Cấu hình từ biến môi trường
  ai/              # Client gateway, agent loop (turn/step), streaming SSE
  auth/            # Google OAuth, session, CSRF, phân quyền
  db/              # Truy vấn SQLx (user, session, chat, audit, settings)
  routes/          # Handler: login, chat, admin
  tools.rs         # Công cụ cho agent: thời gian, máy tính, đọc trang web
  util.rs          # Markdown, format thời gian, escape
templates/         # Askama
static/            # CSS, JS, thư viện vendor
migrations/        # SQL migration (chạy tự động khi khởi động)
```

## Triển khai

- Dockerfile nhiều tầng, image chạy chỉ chứa binary + static + templates
- Đặt `APP_BASE_URL` đúng domain (https) để cookie tự bật cờ `Secure`
- Khuyến nghị đặt PostgreSQL và secret riêng, chỉ expose port app

## Phát triển

```bash
cargo test     # unit test
cargo clippy   # lint
```

## Giấy phép

MIT
