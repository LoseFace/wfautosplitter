use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct Split {
    pub split_index: i64,
    pub split_name: String,
    pub split_time: f64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Run {
    pub id: Option<i64>,

    pub nickname: String,

    pub template_id: String,
    pub template_name: String,

    pub created_at: i64,
    pub total_time: f64,

    pub splits: Vec<Split>,
}

// Template cell for the home screen
#[derive(Serialize, Clone)]
pub struct TemplateSummary {
    pub template_id: String,
    pub template_name: String,
    pub best_time: f64,
    pub best_run_date: i64,
    pub runs_count: i64,
    pub abort_count: i64,
}

// Simplified entry for the schedule
#[derive(Serialize, Clone)]
pub struct RunChartPoint {
    pub run_id: i64,
    pub created_at: i64,
    pub total_time: f64,
    pub splits: Vec<Split>,
}

fn load_splits(conn: &Connection, run_id: i64) -> rusqlite::Result<Vec<Split>> {
    let mut stmt = conn.prepare(
        "SELECT split_index, split_name, split_time
         FROM splits
         WHERE run_id = ?1
         ORDER BY split_index",
    )?;

    let iter = stmt.query_map(params![run_id], |row| {
        Ok(Split {
            split_index: row.get(0)?,
            split_name: row.get(1)?,
            split_time: row.get(2)?,
        })
    })?;

    iter.collect()
}

fn ensure_player(conn: &Connection, nickname: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO players (nickname) VALUES (?1)",
        params![nickname],
    )?;
    conn.query_row(
        "SELECT id FROM players WHERE nickname = ?1",
        params![nickname],
        |row| row.get(0),
    )
}

pub fn insert_run(conn: &mut Connection, run: Run) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;

    let player_id = ensure_player(&tx, &run.nickname)?;

    tx.execute(
        "INSERT INTO runs (
            player_id, template_id, template_name,
            created_at, total_time
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            player_id,
            &run.template_id,
            &run.template_name,
            run.created_at,
            run.total_time,
        ],
    )?;

    let run_id = tx.last_insert_rowid();

    for split in &run.splits {
        tx.execute(
            "INSERT INTO splits (run_id, split_index, split_name, split_time)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, split.split_index, &split.split_name, split.split_time],
        )?;
    }

    tx.commit()?;
    Ok(run_id)
}

pub fn increment_aborts(
    conn: &mut Connection,
    nickname: &str,
    template_id: &str,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    let player_id = ensure_player(&tx, nickname)?;

    let has_runs: bool = tx
        .query_row(
            "SELECT COUNT(*) FROM runs
             WHERE player_id = ?1
               AND template_id = ?2",
            params![player_id, template_id],
            |row| row.get::<_, i64>(0),
        )?
        > 0;

    if !has_runs {
        tx.commit()?;
        return Ok(());
    }

    tx.execute(
        "INSERT INTO aborts (player_id, template_id, abort_count)
         VALUES (?1, ?2, 1)
         ON CONFLICT(player_id, template_id)
         DO UPDATE SET abort_count = abort_count + 1",
        params![player_id, template_id],
    )?;

    tx.commit()
}

pub fn get_runs(
    conn: &mut Connection,
    player_nickname: &str,
    template_id: Option<&str>,
) -> rusqlite::Result<Vec<Run>> {
    let mut query = String::from(
        "SELECT r.id, r.template_id, r.template_name,
                r.created_at, r.total_time, p.nickname
         FROM runs r
         JOIN players p ON r.player_id = p.id
         WHERE p.nickname = ?1",
    );

    if template_id.is_some() {
        query.push_str(" AND r.template_id = ?2");
    }

    query.push_str(" ORDER BY r.created_at DESC");

    struct RawRun {
        id: i64,
        template_id: String,
        template_name: String,
        created_at: i64,
        total_time: f64,
        nickname: String,
    }

    let raw_runs: Vec<RawRun> = {
        let mut stmt = conn.prepare(&query)?;

        macro_rules! map_row {
            ($row:expr) => {
                Ok(RawRun {
                    id: $row.get(0)?,
                    template_id: $row.get(1)?,
                    template_name: $row.get(2)?,
                    created_at: $row.get(3)?,
                    total_time: $row.get(4)?,
                    nickname: $row.get(5)?,
                })
            };
        }

        match template_id {
            Some(t) => stmt
                .query_map(params![player_nickname, t], |r| map_row!(r))?
                .collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt
                .query_map(params![player_nickname], |r| map_row!(r))?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        }
    };

    let mut runs = Vec::with_capacity(raw_runs.len());
    for r in raw_runs {
        let splits = load_splits(conn, r.id)?;
        runs.push(Run {
            id: Some(r.id),
            nickname: r.nickname,
            template_id: r.template_id,
            template_name: r.template_name,
            created_at: r.created_at,
            total_time: r.total_time,
            splits,
        });
    }

    Ok(runs)
}

pub fn get_best_run(
    conn: &mut Connection,
    player_nickname: &str,
    template_id: &str,
) -> rusqlite::Result<Option<Run>> {
    let row = {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.template_id, r.template_name,
                    r.created_at, r.total_time, p.nickname
             FROM runs r
             JOIN players p ON r.player_id = p.id
             WHERE p.nickname = ?1
               AND r.template_id = ?2
             ORDER BY r.total_time ASC
             LIMIT 1",
        )?;

        struct RawRun {
            id: i64, template_id: String,
            template_name: String, created_at: i64,
            total_time: f64, nickname: String,
        }

        stmt.query_row(params![player_nickname, template_id], |r| {
            Ok(RawRun {
                id: r.get(0)?,
                template_id: r.get(1)?,
                template_name: r.get(2)?,
                created_at: r.get(3)?,
                total_time: r.get(4)?,
                nickname: r.get(5)?,
            })
        }).optional()?
    };

    let Some(r) = row else { return Ok(None) };
    let splits = load_splits(conn, r.id)?;

    Ok(Some(Run {
        id: Some(r.id), nickname: r.nickname,
        template_id: r.template_id, template_name: r.template_name,
        created_at: r.created_at, total_time: r.total_time, splits,
    }))
}

pub fn get_run_by_id(conn: &mut Connection, run_id: i64) -> rusqlite::Result<Option<Run>> {
    struct RawRun {
        id: i64,
        template_id: String,
        template_name: String,
        created_at: i64,
        total_time: f64,
        nickname: String,
    }

    let row = {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.template_id, r.template_name,
                    r.created_at, r.total_time, p.nickname
             FROM runs r
             JOIN players p ON r.player_id = p.id
             WHERE r.id = ?1",
        )?;

        let result = stmt
            .query_row(params![run_id], |r| {
                Ok(RawRun {
                    id: r.get(0)?,
                    template_id: r.get(1)?,
                    template_name: r.get(2)?,
                    created_at: r.get(3)?,
                    total_time: r.get(4)?,
                    nickname: r.get(5)?,
                })
            })
            .optional()?;

        result
    };

    let Some(r) = row else { return Ok(None) };

    let splits = load_splits(conn, r.id)?;

    Ok(Some(Run {
        id: Some(r.id),
        nickname: r.nickname,
        template_id: r.template_id,
        template_name: r.template_name,
        created_at: r.created_at,
        total_time: r.total_time,
        splits,
    }))
}

pub fn delete_run(conn: &mut Connection, run_id: i64) -> rusqlite::Result<bool> {
    let tx = conn.transaction()?;

    let run_meta: Option<(i64, String)> = tx
        .query_row(
            "SELECT player_id, template_id FROM runs WHERE id = ?1",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    tx.execute("DELETE FROM splits WHERE run_id = ?1", params![run_id])?;
    let affected = tx.execute("DELETE FROM runs WHERE id = ?1", params![run_id])?;

    if let Some((player_id, template_id)) = run_meta {
        let remaining: i64 = tx.query_row(
            "SELECT COUNT(*) FROM runs
             WHERE player_id = ?1
               AND template_id = ?2",
            params![player_id, &template_id],
            |row| row.get(0),
        )?;

        if remaining == 0 {
            tx.execute(
                "DELETE FROM aborts
                 WHERE player_id = ?1
                   AND template_id = ?2",
                params![player_id, &template_id],
            )?;
        }
    }

    tx.commit()?;
    Ok(affected > 0)
}

pub fn get_best_time(
    conn: &mut Connection,
    player_nickname: &str,
    template_id: &str,
) -> rusqlite::Result<Option<f64>> {
    conn.query_row(
        "SELECT MIN(r.total_time)
         FROM runs r
         JOIN players p ON r.player_id = p.id
         WHERE p.nickname = ?1
           AND r.template_id = ?2",
        params![player_nickname, template_id],
        |row| row.get(0),
    )
    .optional()
    .map(|opt| opt.flatten())
}

pub fn get_best_splits(
    conn: &mut Connection,
    player_nickname: &str,
    template_id: &str,
) -> rusqlite::Result<Vec<Split>> {
    let mut stmt = conn.prepare(
        "SELECT s.split_index, s.split_name, MIN(s.split_time) as split_time
         FROM splits s
         JOIN runs r ON s.run_id = r.id
         JOIN players p ON r.player_id = p.id
         WHERE p.nickname = ?1
           AND r.template_id = ?2
         GROUP BY s.split_index",
    )?;

    let iter = stmt.query_map(params![player_nickname, template_id], |row| {
        Ok(Split {
            split_index: row.get(0)?,
            split_name: row.get(1)?,
            split_time: row.get(2)?,
        })
    })?;

    iter.collect()
}

pub fn get_best_segments(
    conn: &mut Connection,
    player_nickname: &str,
    template_id: &str,
) -> rusqlite::Result<Vec<Split>> {
    let mut stmt2 = conn.prepare(
        "SELECT s.run_id, s.split_index, s.split_name, s.split_time
         FROM splits s
         JOIN runs r ON s.run_id = r.id
         JOIN players p ON r.player_id = p.id
         WHERE p.nickname = ?1
           AND r.template_id = ?2
         ORDER BY s.run_id, s.split_index",
    )?;

    // run_id → vec of (split_index, split_name, split_time)
    let mut runs_map: std::collections::HashMap<i64, Vec<(i64, String, f64)>> =
        std::collections::HashMap::new();

    let iter = stmt2.query_map(params![player_nickname, template_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;

    for row in iter {
        let (run_id, split_index, split_name, split_time) = row?;
        runs_map
            .entry(run_id)
            .or_default()
            .push((split_index, split_name, split_time));
    }

    let mut best_segments: std::collections::HashMap<i64, (String, f64)> =
        std::collections::HashMap::new();

    for splits in runs_map.values() {
        let mut sorted = splits.clone();
        sorted.sort_by_key(|(idx, _, _)| *idx);

        for (i, (split_index, split_name, split_time)) in sorted.iter().enumerate() {
            let segment = if i == 0 {
                *split_time
            } else {
                let prev_time = sorted[i - 1].2;
                split_time - prev_time
            };

            let entry = best_segments
                .entry(*split_index)
                .or_insert((split_name.clone(), f64::MAX));

            if segment < entry.1 {
                *entry = (split_name.clone(), segment);
            }
        }
    }

    let mut result: Vec<Split> = best_segments
        .into_iter()
        .map(|(split_index, (split_name, split_time))| Split {
            split_index,
            split_name,
            split_time,
        })
        .collect();

    result.sort_by_key(|s| s.split_index);
    Ok(result)
}

pub fn get_template_summaries(
    conn: &mut Connection,
    player_nickname: &str,
) -> rusqlite::Result<Vec<TemplateSummary>> {
    struct RawSummary {
        template_id: String,
        best_time: f64,
        runs_count: i64,
    }

    let raw: Vec<RawSummary> = {
        let mut stmt = conn.prepare(
            "SELECT r.template_id,
            MIN(r.total_time) AS best_time,
            COUNT(*) AS runs_count
            FROM runs r
            JOIN players p ON r.player_id = p.id
            WHERE p.nickname = ?1
            GROUP BY r.template_id
            ORDER BY r.template_id",
        )?;

        let result = stmt
            .query_map(params![player_nickname], |row| {
                Ok(RawSummary {
                    template_id: row.get(0)?,
                    best_time: row.get(1)?,
                    runs_count: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        result
    };

    let mut summaries = Vec::with_capacity(raw.len());

    for r in raw {
        let template_name: String = conn.query_row(
            "SELECT r.template_name
             FROM runs r
             JOIN players p ON r.player_id = p.id
             WHERE p.nickname = ?1
               AND r.template_id = ?2
             ORDER BY r.created_at DESC
             LIMIT 1",
            params![player_nickname, &r.template_id],
            |row| row.get(0),
        )?;

        let best_run_date: i64 = conn.query_row(
            "SELECT r.created_at
             FROM runs r
             JOIN players p ON r.player_id = p.id
             WHERE p.nickname = ?1
               AND r.template_id = ?2
             ORDER BY r.total_time ASC
             LIMIT 1",
            params![player_nickname, &r.template_id],
            |row| row.get(0),
        )?;

        let abort_count: i64 = conn
            .query_row(
                "SELECT a.abort_count
                 FROM aborts a
                 JOIN players p ON a.player_id = p.id
                 WHERE p.nickname = ?1
                   AND a.template_id = ?2",
                params![player_nickname, &r.template_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);

        summaries.push(TemplateSummary {
            template_id: r.template_id,
            template_name,
            best_time: r.best_time,
            best_run_date,
            runs_count: r.runs_count,
            abort_count,
        });
    }

    Ok(summaries)
}

pub fn get_runs_for_chart(
    conn: &mut Connection,
    player_nickname: &str,
    template_id: &str,
) -> rusqlite::Result<Vec<RunChartPoint>> {
    struct RawPoint {
        run_id: i64,
        created_at: i64,
        total_time: f64,
    }

    let raw: Vec<RawPoint> = {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.created_at, r.total_time
             FROM runs r
             JOIN players p ON r.player_id = p.id
             WHERE p.nickname = ?1
               AND r.template_id = ?2
             ORDER BY r.created_at ASC",
        )?;

        let result = stmt
            .query_map(params![player_nickname, template_id], |row| {
                Ok(RawPoint {
                    run_id: row.get(0)?,
                    created_at: row.get(1)?,
                    total_time: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        result
    };

    let mut points = Vec::with_capacity(raw.len());
    for r in raw {
        let splits = load_splits(conn, r.run_id)?;
        points.push(RunChartPoint {
            run_id: r.run_id,
            created_at: r.created_at,
            total_time: r.total_time,
            splits,
        });
    }

    Ok(points)
}

pub fn sync_runs_to_template(
    conn: &mut Connection,
    nickname: &str,
    from_template_id: &str,
    to_template_id: &str,
    to_template_name: &str,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    let player_id = ensure_player(&tx, nickname)?;

    tx.execute(
        "UPDATE runs SET template_id = ?1, template_name = ?2
         WHERE player_id = ?3
           AND template_id = ?4",
        params![to_template_id, to_template_name, player_id, from_template_id],
    )?;

    let from_aborts: Option<i64> = tx.query_row(
        "SELECT abort_count FROM aborts
         WHERE player_id = ?1 AND template_id = ?2",
        params![player_id, from_template_id],
        |row| row.get(0),
    ).optional()?;

    if let Some(from_count) = from_aborts {
        let to_exists: bool = tx.query_row(
            "SELECT COUNT(*) FROM aborts
             WHERE player_id = ?1 AND template_id = ?2",
            params![player_id, to_template_id],
            |row| row.get::<_, i64>(0),
        )? > 0;

        if to_exists {
            tx.execute(
                "UPDATE aborts SET abort_count = abort_count + ?1
                 WHERE player_id = ?2 AND template_id = ?3",
                params![from_count, player_id, to_template_id],
            )?;
        } else {
            tx.execute(
                "INSERT INTO aborts (player_id, template_id, abort_count)
                 VALUES (?1, ?2, ?3, ?4)",
                params![player_id, to_template_id, from_count],
            )?;
        }

        tx.execute(
            "DELETE FROM aborts
             WHERE player_id = ?1 AND template_id = ?2",
            params![player_id, from_template_id],
        )?;
    }

    tx.commit()
}