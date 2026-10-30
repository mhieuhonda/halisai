-- Bộ đếm mức dùng hàng giờ của từng người dùng
CREATE TABLE usage_state (
    user_id       UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    window_start  TIMESTAMPTZ NOT NULL DEFAULT now(),
    used          INTEGER NOT NULL DEFAULT 0 CHECK (used >= 0),
    blocked_until TIMESTAMPTZ,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
