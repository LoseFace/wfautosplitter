CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    template_id TEXT NOT NULL,
    template_name TEXT NOT NULL,
    total_time REAL NOT NULL,
    created_at INTEGER NOT NULL,
    success INTEGER NOT NULL DEFAULT 1,
    visibility INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS splits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    split_index INTEGER NOT NULL,
    split_name TEXT NOT NULL,
    split_time REAL NOT NULL,
    group_index INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY(run_id) REFERENCES runs(id)
);

CREATE TABLE IF NOT EXISTS aborts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    template_id TEXT NOT NULL UNIQUE,
    abort_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_runs_template ON runs(template_id);
CREATE INDEX IF NOT EXISTS idx_runs_created ON runs(created_at);
CREATE INDEX IF NOT EXISTS idx_splits_run ON splits(run_id);
CREATE INDEX IF NOT EXISTS idx_aborts_lookup ON aborts(template_id);