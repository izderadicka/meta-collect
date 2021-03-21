-- Add migration script here

create table if not exists tags (
 id INTEGER PRIMARY KEY,
 path TEXT NOT NULL UNIQUE,
 title TEXT,
 artist TEXT,
 composer TEXT,
 album TEXT,
 year TEXT,
 comment TEXT,
 description TEXT,
 genre TEXT,
 duration INTEGER DEFAULT 0,
 bitrate INTEGER NOT NULL,
 num_chapters INTEGER DEFAULT 0,
 ts NUMERIC DEFAULT CURRENT_TIMESTAMP

)
