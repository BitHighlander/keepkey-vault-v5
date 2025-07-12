import Database from '@tauri-apps/plugin-sql';

// Use the .keepkey/db/db.sql path as requested
const DB_PATH = 'sqlite:.keepkey/db/db.sql';

export async function initFooBarTable() {
  const db = await Database.load(DB_PATH);
  await db.execute(`CREATE TABLE IF NOT EXISTS foo_bar (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    foo TEXT NOT NULL,
    bar TEXT NOT NULL
  )`);
  return db;
}

export async function insertHelloWorld(db: Database) {
  await db.execute(
    'INSERT INTO foo_bar (foo, bar) VALUES ($1, $2)',
    ['hello', 'world']
  );
}

export async function getAllFooBar(db: Database) {
  return await db.select('SELECT * FROM foo_bar');
}

// Example usage (call from your React app or a test file):
// (async () => {
//   const db = await initFooBarTable();
//   await insertHelloWorld(db);
//   const rows = await getAllFooBar(db);
//   console.log(rows);
// })();
