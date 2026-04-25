CREATE TABLE IF NOT EXISTS players (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nickname TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id INTEGER NOT NULL,

    template_id TEXT NOT NULL,
    template_name TEXT NOT NULL,
    total_time REAL NOT NULL,

    created_at INTEGER NOT NULL,

    FOREIGN KEY(player_id) REFERENCES players(id)
);

CREATE TABLE IF NOT EXISTS splits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    split_index INTEGER NOT NULL,
    split_name TEXT NOT NULL,
    split_time REAL NOT NULL,

    FOREIGN KEY(run_id) REFERENCES runs(id)
);

-- Глобальный счётчик сбросов на уровне игрок + шаблон + сложность.
-- Увеличивается на 1 при каждом сбросе активного рана.
-- Не привязан к конкретному run — это накопительная статистика.
CREATE TABLE IF NOT EXISTS aborts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id INTEGER NOT NULL,
    template_id TEXT NOT NULL,
    abort_count INTEGER NOT NULL DEFAULT 0,

    UNIQUE(player_id, template_id),
    FOREIGN KEY(player_id) REFERENCES players(id)
);

CREATE INDEX IF NOT EXISTS idx_runs_player_template ON runs(player_id, template_id);
CREATE INDEX IF NOT EXISTS idx_runs_created ON runs(created_at);
CREATE INDEX IF NOT EXISTS idx_splits_run ON splits(run_id);
CREATE INDEX IF NOT EXISTS idx_aborts_lookup ON aborts(player_id, template_id);