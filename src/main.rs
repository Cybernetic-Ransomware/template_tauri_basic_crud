#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct Todo {
    id: u64,
    title: String,
    completed: bool,
    created_at: String,
    deadline: Option<String>,
}

struct AppState {
    db: Mutex<Connection>,
}

fn init_db(conn: &Connection) {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            completed BOOLEAN NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            deadline TEXT
        )",
        [],
    )
    .expect("Failed to create table");
}

// --- Database Logic Functions (Testable) ---

fn db_get_todos(conn: &Connection) -> Vec<Todo> {
    let mut stmt = conn
        .prepare("SELECT id, title, completed, created_at, deadline FROM todos")
        .unwrap();

    let todo_iter = stmt
        .query_map([], |row| {
            Ok(Todo {
                id: row.get(0)?,
                title: row.get(1)?,
                completed: row.get(2)?,
                created_at: row.get(3)?,
                deadline: row.get(4)?,
            })
        })
        .unwrap();

    let mut todos = Vec::new();
    for todo in todo_iter {
        todos.push(todo.unwrap());
    }
    todos
}

fn db_add_todo(conn: &Connection, title: String, deadline: Option<String>) -> Todo {
    let created_at = Local::now().to_rfc3339();

    conn.execute(
        "INSERT INTO todos (title, completed, created_at, deadline) VALUES (?1, ?2, ?3, ?4)",
        (&title, false, &created_at, &deadline),
    )
    .expect("Failed to insert todo");

    let id = conn.last_insert_rowid() as u64;

    Todo {
        id,
        title,
        completed: false,
        created_at,
        deadline,
    }
}

fn db_update_todo(
    conn: &Connection,
    id: u64,
    title: Option<String>,
    completed: Option<bool>,
    deadline: Option<String>,
) -> bool {
    let mut success = false;

    if let Some(t) = title {
        conn.execute("UPDATE todos SET title = ?1 WHERE id = ?2", (&t, id))
            .unwrap();
        success = true;
    }
    if let Some(c) = completed {
        conn.execute("UPDATE todos SET completed = ?1 WHERE id = ?2", (c, id))
            .unwrap();
        success = true;
    }
    if let Some(d) = deadline {
        let val = if d.is_empty() { None } else { Some(d) };
        conn.execute("UPDATE todos SET deadline = ?1 WHERE id = ?2", (val, id))
            .unwrap();
        success = true;
    }

    success
}

fn db_delete_todo(conn: &Connection, id: u64) -> bool {
    let count = conn
        .execute("DELETE FROM todos WHERE id = ?1", (id,))
        .unwrap();
    count > 0
}

// --- Tauri Commands ---

#[tauri::command]
fn get_todos(state: State<AppState>) -> Vec<Todo> {
    let conn = state.db.lock().unwrap();
    db_get_todos(&conn)
}

#[tauri::command]
fn add_todo(title: String, deadline: Option<String>, state: State<AppState>) -> Todo {
    let conn = state.db.lock().unwrap();
    db_add_todo(&conn, title, deadline)
}

#[tauri::command]
fn update_todo(
    id: u64,
    title: Option<String>,
    completed: Option<bool>,
    deadline: Option<String>,
    state: State<AppState>,
) -> bool {
    let conn = state.db.lock().unwrap();
    db_update_todo(&conn, id, title, completed, deadline)
}

#[tauri::command]
fn delete_todo(id: u64, state: State<AppState>) -> bool {
    let conn = state.db.lock().unwrap();
    db_delete_todo(&conn, id)
}

fn main() {
    let db_connection = Connection::open("todos.db").expect("Failed to open database");
    init_db(&db_connection);

    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(db_connection),
        })
        .invoke_handler(tauri::generate_handler![
            get_todos,
            add_todo,
            update_todo,
            delete_todo
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn);
        conn
    }

    #[test]
    fn test_add_and_get_todo() {
        let conn = setup_test_db();

        let todo = db_add_todo(
            &conn,
            "Test Todo".to_string(),
            Some("2023-12-31".to_string()),
        );

        assert_eq!(todo.title, "Test Todo");
        assert_eq!(todo.completed, false);
        assert_eq!(todo.deadline, Some("2023-12-31".to_string()));

        let todos = db_get_todos(&conn);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].title, "Test Todo");
    }

    #[test]
    fn test_update_todo() {
        let conn = setup_test_db();
        let todo = db_add_todo(&conn, "Update Me".to_string(), None);

        // Update completion
        let updated = db_update_todo(&conn, todo.id, None, Some(true), None);
        assert!(updated);

        let todos = db_get_todos(&conn);
        assert!(todos[0].completed);

        // Update title
        db_update_todo(&conn, todo.id, Some("Updated".to_string()), None, None);
        let todos = db_get_todos(&conn);
        assert_eq!(todos[0].title, "Updated");
    }

    #[test]
    fn test_delete_todo() {
        let conn = setup_test_db();
        let todo = db_add_todo(&conn, "Delete Me".to_string(), None);

        let todos_before = db_get_todos(&conn);
        assert_eq!(todos_before.len(), 1);

        let deleted = db_delete_todo(&conn, todo.id);
        assert!(deleted);

        let todos_after = db_get_todos(&conn);
        assert_eq!(todos_after.len(), 0);
    }
}
