-- Tùy chỉnh người dùng: ngôn ngữ giao diện và chủ đề
ALTER TABLE users
    ADD COLUMN lang  TEXT NOT NULL DEFAULT 'vi'  CHECK (lang  IN ('vi', 'en')),
    ADD COLUMN theme TEXT NOT NULL DEFAULT 'light' CHECK (theme IN ('light', 'dark'));
