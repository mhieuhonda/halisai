//! Chuỗi hiển thị theo ngôn ngữ (vi mặc định, en tuỳ chọn trong Cài đặt).

/// Bảng chuỗi hiển thị; trường được template đọc qua macro Askama nên
/// tắt cảnh báo dead_code (giống Ctx).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Strings {
    pub lang: &'static str,
    // ===== Chung =====
    pub new_chat: &'static str,
    pub no_conversations: &'static str,
    pub admin_panel: &'static str,
    pub settings: &'static str,
    pub logout: &'static str,
    pub open_menu: &'static str,
    pub close_menu: &'static str,
    pub delete: &'static str,
    pub rename: &'static str,
    pub confirm_delete_conv: &'static str,
    // ===== Nhóm thời gian =====
    pub today: &'static str,
    pub yesterday: &'static str,
    pub last7: &'static str,
    pub last30: &'static str,
    pub older: &'static str,
    // ===== Trang chat =====
    pub msg_placeholder: &'static str,
    pub send_message: &'static str,
    pub stop_generating: &'static str,
    pub regenerate: &'static str,
    pub rename_conv: &'static str,
    pub delete_conv: &'static str,
    pub disclaimer: &'static str,
    pub you: &'static str,
    pub thinking: &'static str,
    pub thought_for: &'static str,
    pub running: &'static str,
    pub empty_sub: &'static str,
    pub chip_explain: &'static str,
    pub chip_write: &'static str,
    pub chip_plan: &'static str,
    pub chip_code: &'static str,
    pub chip_translate: &'static str,
    pub chip_explain_fill: &'static str,
    pub chip_write_fill: &'static str,
    pub chip_plan_fill: &'static str,
    pub chip_code_fill: &'static str,
    pub chip_translate_fill: &'static str,
    pub stopped_gen: &'static str,
    pub err_generic: &'static str,
    pub truncated: &'static str,
    pub scroll_bottom: &'static str,
    pub copy_msg: &'static str,
    pub cannot_create_conv: &'static str,
    pub cannot_rename: &'static str,
    pub msg_empty: &'static str,
    pub msg_too_long: &'static str,
    pub conv_not_found: &'static str,
    pub too_fast: &'static str,
    // ===== Mức dùng =====
    pub usage: &'static str,
    pub usage_per_hour: &'static str,
    pub messages_left: &'static str,
    pub message_left: &'static str,
    pub limit_reached: &'static str,
    pub resets_at: &'static str,
    pub usage_note: &'static str,
    pub usage_cooldown_note: &'static str,
    pub usage_blocked_title: &'static str,
    pub usage_blocked_body: &'static str,
    // ===== Cài đặt =====
    pub settings_title: &'static str,
    pub settings_sub: &'static str,
    pub appearance: &'static str,
    pub appearance_sub: &'static str,
    pub theme: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub theme_sub: &'static str,
    pub language: &'static str,
    pub language_sub: &'static str,
    pub lang_vi: &'static str,
    pub lang_en: &'static str,
    pub back_to_chat: &'static str,
    pub account: &'static str,
    pub usage_window: &'static str,
    pub usage_used: &'static str,
    // ===== Trợ lý & auth =====
    pub login_title: &'static str,
    pub login_sub: &'static str,
    pub login_google: &'static str,
    pub login_foot: &'static str,
    pub login_pagefoot: &'static str,
    pub banned_title: &'static str,
    pub banned_perm: &'static str,
    pub banned_temp: &'static str,
    pub banned_until: &'static str,
    pub banned_reason: &'static str,
    pub try_again: &'static str,
    // ===== Lỗi hệ thống =====
    pub err_bad_request: &'static str,
    pub err_not_found: &'static str,
    pub err_forbidden: &'static str,
    pub err_server: &'static str,
    pub err_not_found_title: &'static str,
    pub err_forbidden_title: &'static str,
    pub err_server_title: &'static str,
}

pub const VI: Strings = Strings {
    lang: "vi",
    new_chat: "Cuộc trò chuyện mới",
    no_conversations: "Chưa có cuộc trò chuyện nào",
    admin_panel: "Bảng quản trị",
    settings: "Cài đặt",
    logout: "Đăng xuất",
    open_menu: "Mở menu",
    close_menu: "Đóng menu",
    delete: "Xóa",
    rename: "Đổi tên",
    confirm_delete_conv: "Xóa cuộc trò chuyện này?",
    today: "Hôm nay",
    yesterday: "Hôm qua",
    last7: "7 ngày qua",
    last30: "30 ngày qua",
    older: "Cũ hơn",
    msg_placeholder: "Nhắn tin cho Halis…",
    send_message: "Gửi tin nhắn",
    stop_generating: "Dừng tạo câu trả lời",
    regenerate: "Trả lời lại",
    rename_conv: "Đổi tên cuộc trò chuyện",
    delete_conv: "Xóa cuộc trò chuyện",
    disclaimer: "Halis có thể mắc lỗi. Hãy kiểm chứng những thông tin quan trọng.",
    you: "Bạn",
    thinking: "Đang suy nghĩ",
    thought_for: "Đã suy nghĩ",
    running: "đang chạy…",
    empty_sub: "Trợ lý AI của bạn — hỏi bất cứ điều gì.",
    chip_explain: "Giải thích một khái niệm",
    chip_write: "Viết một email",
    chip_plan: "Lên kế hoạch học",
    chip_code: "Xem xét code",
    chip_translate: "Dịch văn bản",
    chip_explain_fill: "Giải thích cho tôi sự khác nhau giữa TCP và UDP một cách dễ hiểu",
    chip_write_fill: "Viết giúp tôi một email xin nghỉ phép gửi sếp, giọng lịch sự chuyên nghiệp",
    chip_plan_fill: "Giúp tôi lập kế hoạch học tiếng Anh 30 ngày, mỗi ngày 45 phút",
    chip_code_fill: "Review đoạn code này và gợi ý cải thiện: ",
    chip_translate_fill: "Dịch sang tiếng Anh: ",
    stopped_gen: "Đã dừng tạo câu trả lời.",
    err_generic: "Halis tạm thời không phản hồi. Vui lòng thử lại sau ít phút.",
    truncated: "Câu trả lời này đã bị ngắt vì đạt giới hạn đầu ra.",
    scroll_bottom: "Xuống cuối hội thoại",
    copy_msg: "Copy tin nhắn",
    cannot_create_conv: "Không tạo được cuộc trò chuyện. Thử lại nhé.",
    cannot_rename: "Không đổi được tên.",
    msg_empty: "Tin nhắn không được để trống.",
    msg_too_long: "Tin nhắn giới hạn 32.000 ký tự.",
    conv_not_found: "Cuộc trò chuyện không tồn tại.",
    too_fast: "Bạn gửi tin quá nhanh, vui lòng chờ một chút.",
    usage: "Mức dùng",
    usage_per_hour: "tin nhắn / giờ",
    messages_left: "tin nhắn còn lại",
    message_left: "tin nhắn còn lại",
    limit_reached: "Đã đạt giới hạn",
    resets_at: "đặt lại lúc",
    usage_note: "Bạn có 200 tin nhắn mỗi giờ. Khi đạt giới hạn, hãy chờ 1 giờ 5 phút để tiếp tục.",
    usage_cooldown_note: "Hạn mức sẽ mở lại sau 1 giờ 5 phút kể từ lúc đạt giới hạn.",
    usage_blocked_title: "Đã đạt giới mức dùng",
    usage_blocked_body: "Bạn đã dùng hết 200 tin nhắn trong giờ này. Hãy chờ hết thời gian dưới đây để tiếp tục trò chuyện.",
    settings_title: "Cài đặt",
    settings_sub: "Tuỳ chỉnh HalisAI theo cách của bạn",
    appearance: "Giao diện",
    appearance_sub: "Chủ đề màu sắc của ứng dụng",
    theme: "Chủ đề",
    theme_light: "Sáng",
    theme_dark: "Tối",
    theme_sub: "Chủ đề áp dụng ngay và được lưu cho các lần đăng nhập sau.",
    language: "Ngôn ngữ",
    language_sub: "Ngôn ngữ hiển thị của giao diện và ngôn ngữ Halis mặc định trả lời",
    lang_vi: "Tiếng Việt",
    lang_en: "English",
    back_to_chat: "Về trang chat",
    account: "Tài khoản",
    usage_window: "Gói miễn phí",
    usage_used: "đã dùng",
    login_title: "Chào mừng trở lại",
    login_sub: "Trợ lý AI cá nhân của bạn — nhanh, riêng tư và luôn sẵn sàng.",
    login_google: "Đăng nhập bằng Google",
    login_foot: "Bằng việc tiếp tục, bạn đồng ý với điều khoản sử dụng dịch vụ HalisAI.",
    login_pagefoot: "HalisAI · được tạo ra bởi Hieu Louis",
    banned_title: "Tài khoản đã bị cấm",
    banned_perm: "Tài khoản của bạn bị cấm vĩnh viễn khỏi HalisAI.",
    banned_temp: "Tài khoản của bạn tạm thời không thể sử dụng HalisAI.",
    banned_until: "Đến",
    banned_reason: "Lý do",
    try_again: "Thử lại",
    err_bad_request: "Yêu cầu không hợp lệ",
    err_not_found: "Không tìm thấy",
    err_not_found_title: "Không tìm thấy trang",
    err_forbidden: "Không đủ quyền",
    err_forbidden_title: "Không đủ quyền truy cập",
    err_server: "Lỗi hệ thống",
    err_server_title: "Đã có lỗi xảy ra",
};

pub const EN: Strings = Strings {
    lang: "en",
    new_chat: "New chat",
    no_conversations: "No conversations yet",
    admin_panel: "Admin",
    settings: "Settings",
    logout: "Log out",
    open_menu: "Open menu",
    close_menu: "Close menu",
    delete: "Delete",
    rename: "Rename",
    confirm_delete_conv: "Delete this conversation?",
    today: "Today",
    yesterday: "Yesterday",
    last7: "Previous 7 days",
    last30: "Previous 30 days",
    older: "Older",
    msg_placeholder: "Message Halis…",
    send_message: "Send message",
    stop_generating: "Stop generating",
    regenerate: "Regenerate",
    rename_conv: "Rename conversation",
    delete_conv: "Delete conversation",
    disclaimer: "Halis can make mistakes. Please verify important information.",
    you: "You",
    thinking: "Thinking",
    thought_for: "Thought for",
    running: "running…",
    empty_sub: "Your AI assistant — ask anything.",
    chip_explain: "Explain a concept",
    chip_write: "Write an email",
    chip_plan: "Make a study plan",
    chip_code: "Review code",
    chip_translate: "Translate text",
    chip_explain_fill: "Explain the difference between TCP and UDP in simple terms",
    chip_write_fill: "Write a polite, professional PTO request email to my manager",
    chip_plan_fill: "Build me a 30-day English study plan, 45 minutes a day",
    chip_code_fill: "Review this code and suggest improvements: ",
    chip_translate_fill: "Translate to Vietnamese: ",
    stopped_gen: "Stopped generating.",
    err_generic: "Halis is temporarily unavailable. Please try again in a few minutes.",
    truncated: "This response was cut off due to output limits.",
    scroll_bottom: "Scroll to bottom",
    copy_msg: "Copy message",
    cannot_create_conv: "Could not create the conversation. Please retry.",
    cannot_rename: "Could not rename.",
    msg_empty: "Message must not be empty.",
    msg_too_long: "Messages are limited to 32,000 characters.",
    conv_not_found: "Conversation not found.",
    too_fast: "You are sending messages too fast. Please wait a moment.",
    usage: "Usage",
    usage_per_hour: "messages / hour",
    messages_left: "messages left",
    message_left: "message left",
    limit_reached: "Limit reached",
    resets_at: "resets at",
    usage_note: "You get 200 messages per hour. When the limit is reached, wait 1 hour 5 minutes to continue.",
    usage_cooldown_note: "The allowance reopens 1 hour 5 minutes after the limit is reached.",
    usage_blocked_title: "Usage limit reached",
    usage_blocked_body: "You have used all 200 messages in this hour. Wait for the countdown below to keep chatting.",
    settings_title: "Settings",
    settings_sub: "Make HalisAI yours",
    appearance: "Appearance",
    appearance_sub: "How the app looks",
    theme: "Theme",
    theme_light: "Light",
    theme_dark: "Dark",
    theme_sub: "Applies instantly and is remembered for future sessions.",
    language: "Language",
    language_sub: "Interface language, also the language Halis replies in by default",
    lang_vi: "Tiếng Việt",
    lang_en: "English",
    back_to_chat: "Back to chat",
    account: "Account",
    usage_window: "Free plan",
    usage_used: "used",
    login_title: "Welcome back",
    login_sub: "Your personal AI assistant — fast, private and always ready.",
    login_google: "Continue with Google",
    login_foot: "By continuing, you agree to the HalisAI terms of service.",
    login_pagefoot: "HalisAI · built by Hieu Louis",
    banned_title: "Account banned",
    banned_perm: "Your account has been permanently banned from HalisAI.",
    banned_temp: "Your account is temporarily unable to use HalisAI.",
    banned_until: "Until",
    banned_reason: "Reason",
    try_again: "Try again",
    err_bad_request: "Invalid request",
    err_not_found: "Not found",
    err_not_found_title: "Page not found",
    err_forbidden: "Insufficient permissions",
    err_forbidden_title: "Access denied",
    err_server: "Server error",
    err_server_title: "Something went wrong",
};

impl Strings {
    pub fn for_lang(lang: &str) -> &'static Strings {
        if lang == "en" { &EN } else { &VI }
    }
}

const DAYS_VI: [&str; 7] = ["Chủ nhật", "Thứ hai", "Thứ ba", "Thứ tư", "Thứ năm", "Thứ sáu", "Thứ bảy"];
const DAYS_EN: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// Thứ trong tuần theo ngôn ngữ, index từ 0 = Thứ hai.
pub fn weekday_name(t: time::OffsetDateTime, lang: &str) -> &'static str {
    let idx = t.weekday().number_days_from_monday() as usize % 7;
    if lang == "en" { DAYS_EN[idx] } else { DAYS_VI[idx] }
}

/// Thời gian tương đối: "5 phút trước" / "5 min ago".
pub fn relative_time(t: time::OffsetDateTime, now: time::OffsetDateTime, lang: &str) -> String {
    let secs = (now - t).whole_seconds().max(0);
    let en = lang == "en";
    match secs {
        0..=45 => if en { "just now".into() } else { "vừa xong".into() },
        46..=1500 => {
            let m = ((secs + 59) / 60).max(1);
            if en { format!("{m} min ago") } else { format!("{m} phút trước") }
        }
        1501..=86_400 => {
            let h = ((secs + 3599) / 3600).max(1);
            if en { format!("{h} h ago") } else { format!("{h} giờ trước") }
        }
        _ => {
            let d = (secs + 43_199) / 86_400;
            if d <= 2 {
                if en { format!("{d} d ago") } else { format!("{d} ngày trước") }
            } else {
                format!("{:02}/{:02}/{}", t.day(), t.month() as u8, t.year())
            }
        }
    }
}

/// Giờ kiểu "14:30" (vi) hoặc "2:30 PM" (en).
pub fn clock_time(t: time::OffsetDateTime, lang: &str) -> String {
    let (h, m) = (t.hour(), t.minute());
    if lang == "en" {
        let ampm = if h < 12 { "AM" } else { "PM" };
        let h12 = match h % 12 { 0 => 12, x => x };
        format!("{h12}:{m:02} {ampm}")
    } else {
        format!("{h:02}:{m:02}")
    }
}

/// "Thứ Tư, 30/10/2026 14:30:05 (GMT+7)" — dùng cho trang cấm tài khoản.
pub fn format_full_vi(t: time::OffsetDateTime) -> String {
    let local = t.to_offset(time::macros::offset!(+7));
    format!(
        "{}, {:02}/{:02}/{} {:02}:{:02} (GMT+7)",
        weekday_name(local, "vi"),
        local.day(),
        local.month() as u8,
        local.year(),
        local.hour(),
        local.minute(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn test_relative_vi() {
        let now = time::OffsetDateTime::now_utc();
        assert_eq!(relative_time(now, now, "vi"), "vừa xong");
        assert_eq!(relative_time(now - Duration::minutes(5), now, "vi"), "5 phút trước");
        assert_eq!(relative_time(now - Duration::hours(3), now, "vi"), "3 giờ trước");
    }

    #[test]
    fn test_relative_en() {
        let now = time::OffsetDateTime::now_utc();
        assert_eq!(relative_time(now - Duration::minutes(5), now, "en"), "5 min ago");
        assert_eq!(relative_time(now - Duration::hours(3), now, "en"), "3 h ago");
    }

    #[test]
    fn test_clock() {
        let t = time::OffsetDateTime::from_unix_timestamp(0).unwrap()
            .to_offset(time::macros::offset!(+7))
            .replace_time(time::Time::from_hms(14, 5, 0).unwrap());
        assert_eq!(clock_time(t, "vi"), "14:05");
        assert_eq!(clock_time(t, "en"), "2:05 PM");
    }
}
