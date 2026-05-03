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
    pub template_id: String,
    pub template_name: String,
    pub created_at: i64,
    pub total_time: f64,
    pub splits: Vec<Split>,
    pub success: bool,
}

#[derive(Serialize, Clone)]
pub struct TemplateSummary {
    pub template_id: String,
    pub template_name: String,
    pub best_time: f64,
    pub best_run_date: i64,
    pub runs_count: i64,
    pub abort_count: i64,
}

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
    let result: rusqlite::Result<Vec<Split>> = stmt.query_map(params![run_id], |row| {
        Ok(Split {
            split_index: row.get(0)?,
            split_name: row.get(1)?,
            split_time: row.get(2)?,
        })
    })?.collect();
    result
}

pub fn insert_run(conn: &mut Connection, run: Run) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO runs (template_id, template_name, created_at, total_time, success)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            &run.template_id,
            &run.template_name,
            run.created_at,
            run.total_time,
            run.success as i64,
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

pub fn increment_aborts(conn: &mut Connection, template_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO aborts (template_id, abort_count)
         VALUES (?1, 1)
         ON CONFLICT(template_id)
         DO UPDATE SET abort_count = abort_count + 1",
        params![template_id],
    )?;
    Ok(())
}

pub fn get_runs(
    conn: &mut Connection,
    template_id: Option<&str>,
) -> rusqlite::Result<Vec<Run>> {
    struct RawRun {
        id: i64,
        template_id: String,
        template_name: String,
        created_at: i64,
        total_time: f64,
        success: bool,
    }

    let raw_runs: Vec<RawRun> = match template_id {
        Some(t) => {
            let mut stmt = conn.prepare(
                "SELECT id, template_id, template_name, created_at, total_time, COALESCE(success, 1)
                 FROM runs WHERE template_id = ?1 ORDER BY created_at DESC",
            )?;
            let result: rusqlite::Result<Vec<RawRun>> = stmt.query_map(params![t], |row| {
                Ok(RawRun {
                    id: row.get(0)?,
                    template_id: row.get(1)?,
                    template_name: row.get(2)?,
                    created_at: row.get(3)?,
                    total_time: row.get(4)?,
                    success: row.get::<_, i64>(5)? != 0,
                })
            })?.collect();
            result?
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, template_id, template_name, created_at, total_time, COALESCE(success, 1)
                 FROM runs ORDER BY created_at DESC",
            )?;
            let result: rusqlite::Result<Vec<RawRun>> = stmt.query_map([], |row| {
                Ok(RawRun {
                    id: row.get(0)?,
                    template_id: row.get(1)?,
                    template_name: row.get(2)?,
                    created_at: row.get(3)?,
                    total_time: row.get(4)?,
                    success: row.get::<_, i64>(5)? != 0,
                })
            })?.collect();
            result?
        }
    };

    let mut runs = Vec::with_capacity(raw_runs.len());
    for r in raw_runs {
        let splits = load_splits(conn, r.id)?;
        runs.push(Run {
            id: Some(r.id),
            template_id: r.template_id,
            template_name: r.template_name,
            created_at: r.created_at,
            total_time: r.total_time,
            splits,
            success: r.success,
        });
    }

    Ok(runs)
}

pub fn get_best_run(conn: &mut Connection, template_id: &str) -> rusqlite::Result<Option<Run>> {
    struct RawRun {
        id: i64,
        template_id: String,
        template_name: String,
        created_at: i64,
        total_time: f64,
        success: bool,
    }

    let row = {
        let mut stmt = conn.prepare(
            "SELECT id, template_id, template_name, created_at, total_time, COALESCE(success, 1)
             FROM runs
             WHERE template_id = ?1 AND COALESCE(success, 1) = 1
             ORDER BY total_time ASC LIMIT 1",
        )?;
        let result = stmt.query_row(params![template_id], |r| {
            Ok(RawRun {
                id: r.get(0)?,
                template_id: r.get(1)?,
                template_name: r.get(2)?,
                created_at: r.get(3)?,
                total_time: r.get(4)?,
                success: r.get::<_, i64>(5)? != 0,
            })
        }).optional()?;
        result
    };

    let Some(r) = row else { return Ok(None) };
    let splits = load_splits(conn, r.id)?;
    Ok(Some(Run {
        id: Some(r.id),
        template_id: r.template_id,
        template_name: r.template_name,
        created_at: r.created_at,
        total_time: r.total_time,
        splits,
        success: r.success,
    }))
}

pub fn get_run_by_id(conn: &mut Connection, run_id: i64) -> rusqlite::Result<Option<Run>> {
    struct RawRun {
        id: i64,
        template_id: String,
        template_name: String,
        created_at: i64,
        total_time: f64,
        success: bool,
    }

    let row = {
        let mut stmt = conn.prepare(
            "SELECT id, template_id, template_name, created_at, total_time, COALESCE(success, 1)
             FROM runs WHERE id = ?1",
        )?;
        let result = stmt.query_row(params![run_id], |r| {
            Ok(RawRun {
                id: r.get(0)?,
                template_id: r.get(1)?,
                template_name: r.get(2)?,
                created_at: r.get(3)?,
                total_time: r.get(4)?,
                success: r.get::<_, i64>(5)? != 0,
            })
        }).optional()?;
        result
    };

    let Some(r) = row else { return Ok(None) };
    let splits = load_splits(conn, r.id)?;
    Ok(Some(Run {
        id: Some(r.id),
        template_id: r.template_id,
        template_name: r.template_name,
        created_at: r.created_at,
        total_time: r.total_time,
        splits,
        success: r.success,
    }))
}

pub fn delete_run(conn: &mut Connection, run_id: i64) -> rusqlite::Result<bool> {
    let tx = conn.transaction()?;

    let run_info: Option<(String, i64)> = tx
        .query_row(
            "SELECT template_id, COALESCE(success, 1) FROM runs WHERE id = ?1",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    tx.execute("DELETE FROM splits WHERE run_id = ?1", params![run_id])?;
    let affected = tx.execute("DELETE FROM runs WHERE id = ?1", params![run_id])?;

    if let Some((tid, success)) = run_info {
        let remaining: i64 = tx.query_row(
            "SELECT COUNT(*) FROM runs WHERE template_id = ?1",
            params![&tid],
            |row| row.get(0),
        )?;

        if remaining == 0 {
            tx.execute("DELETE FROM aborts WHERE template_id = ?1", params![&tid])?;
        } else if success == 0 {
            tx.execute(
                "UPDATE aborts SET abort_count = MAX(0, abort_count - 1)
                 WHERE template_id = ?1",
                params![&tid],
            )?;
        }
    }

    tx.commit()?;
    Ok(affected > 0)
}

pub fn get_best_time(conn: &mut Connection, template_id: &str) -> rusqlite::Result<Option<f64>> {
    conn.query_row(
        "SELECT MIN(total_time) FROM runs
         WHERE template_id = ?1 AND COALESCE(success, 1) = 1",
        params![template_id],
        |row| row.get(0),
    )
    .optional()
    .map(|opt| opt.flatten())
}

pub fn get_best_splits(conn: &mut Connection, template_id: &str) -> rusqlite::Result<Vec<Split>> {
    let mut stmt = conn.prepare(
        "SELECT s.split_index, s.split_name, MIN(s.split_time) as split_time
         FROM splits s
         JOIN runs r ON s.run_id = r.id
         WHERE r.template_id = ?1 AND COALESCE(r.success, 1) = 1
         GROUP BY s.split_index",
    )?;
    let result: rusqlite::Result<Vec<Split>> = stmt.query_map(params![template_id], |row| {
        Ok(Split {
            split_index: row.get(0)?,
            split_name: row.get(1)?,
            split_time: row.get(2)?,
        })
    })?.collect();
    result
}

pub fn get_best_segments(conn: &mut Connection, template_id: &str) -> rusqlite::Result<Vec<Split>> {
    let rows: Vec<(i64, i64, String, f64)> = {
        let mut stmt = conn.prepare(
            "SELECT s.run_id, s.split_index, s.split_name, s.split_time
             FROM splits s
             JOIN runs r ON s.run_id = r.id
             WHERE r.template_id = ?1 AND COALESCE(r.success, 1) = 1
             ORDER BY s.run_id, s.split_index",
        )?;
        let result: rusqlite::Result<Vec<(i64, i64, String, f64)>> = stmt.query_map(params![template_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?.collect();
        result?
    };

    let mut runs_map: std::collections::HashMap<i64, Vec<(i64, String, f64)>> =
        std::collections::HashMap::new();

    for (run_id, split_index, split_name, split_time) in rows {
        runs_map.entry(run_id).or_default().push((split_index, split_name, split_time));
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
                split_time - sorted[i - 1].2
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

pub fn get_template_summaries(conn: &mut Connection) -> rusqlite::Result<Vec<TemplateSummary>> {
    struct RawSummary {
        template_id: String,
        best_time: f64,
        runs_count: i64,
    }

    let raw: Vec<RawSummary> = {
        let mut stmt = conn.prepare(
            "SELECT template_id,
                    MIN(CASE WHEN COALESCE(success, 1) = 1 THEN total_time ELSE NULL END) AS best_time,
                    COUNT(*) AS runs_count
             FROM runs
             GROUP BY template_id
             ORDER BY template_id",
        )?;
        let result: rusqlite::Result<Vec<RawSummary>> = stmt.query_map([], |row| {
            Ok(RawSummary {
                template_id: row.get(0)?,
                best_time: row.get::<_, Option<f64>>(1)?.unwrap_or(f64::MAX),
                runs_count: row.get(2)?,
            })
        })?.collect();
        result?
    };

    let mut summaries = Vec::with_capacity(raw.len());

    for r in raw {
        let template_name: String = conn.query_row(
            "SELECT template_name FROM runs
             WHERE template_id = ?1 ORDER BY created_at DESC LIMIT 1",
            params![&r.template_id],
            |row| row.get(0),
        )?;

        let best_run_date: i64 = conn
            .query_row(
                "SELECT created_at FROM runs
                 WHERE template_id = ?1 AND COALESCE(success, 1) = 1
                 ORDER BY total_time ASC LIMIT 1",
                params![&r.template_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);

        let abort_count: i64 = conn
            .query_row(
                "SELECT abort_count FROM aborts WHERE template_id = ?1",
                params![&r.template_id],
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

pub fn get_runs_for_chart(conn: &mut Connection, template_id: &str) -> rusqlite::Result<Vec<RunChartPoint>> {
    struct RawPoint {
        run_id: i64,
        created_at: i64,
        total_time: f64,
    }

    let raw: Vec<RawPoint> = {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, total_time
             FROM runs
             WHERE template_id = ?1 AND COALESCE(success, 1) = 1
             ORDER BY created_at ASC",
        )?;
        let result: rusqlite::Result<Vec<RawPoint>> = stmt.query_map(params![template_id], |row| {
            Ok(RawPoint {
                run_id: row.get(0)?,
                created_at: row.get(1)?,
                total_time: row.get(2)?,
            })
        })?.collect();
        result?
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

pub fn rename_template_runs(
    conn: &mut Connection,
    template_id: &str,
    new_name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE runs SET template_name = ?1 WHERE template_id = ?2",
        params![new_name, template_id],
    )?;
    Ok(())
}