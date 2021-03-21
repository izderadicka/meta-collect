use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use mime_guess::mime;
use sqlx::{Row, sqlite::SqlitePool};
use std::path::{Path, PathBuf};
use std::{borrow::Cow, env};
use structopt::StructOpt;

const DATABASE_URI: &str = "sqlite:tags.db";

#[derive(StructOpt)]
struct Args {
    #[structopt(short, long, help = "database URL - like sqlite:file_name")]
    database_url: Option<String>,

    #[structopt(subcommand)]
    cmd: Cmd,
}

#[derive(StructOpt)]
enum Cmd {
    List,
    Scan { dirs: Vec<PathBuf> },
    Clear,
}

fn s(s: &Option<String>) -> &str {
    s.as_ref().map(|s| s.as_str()).unwrap_or("")
}

#[async_std::main]
async fn main() -> Result<()> {
    let mut args = Args::from_args();
    let db_url = args.database_url.take();
    let pool = SqlitePool::connect(
        env::var("DATABASE_URL")
            .or_else(|e| db_url.ok_or(e))
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(DATABASE_URI))
            .as_ref(),
    )
    .await?;

    match args.cmd {
        Cmd::List => {
            let mut recs = sqlx::query(
                r#"
                select id, path, title, album, artist from tags
                order by path
                ;
                "#
            )
            .fetch(&pool);
            println!("id|path|title|album|artist");
            while let Some(r) = recs.try_next().await? {
                println!("{}|{}|{}|{}|{}", r.get::<i64,_>("id"), r.get::<&str,_>("path"), s(&r.get("title")), 
                s(&r.get("album")), s(&r.get("artist")));
            }
        }

        Cmd::Scan { dirs } => {
            media_info::init();
            for dir in dirs {
                //println!("Processing {:?}", dir);

                for f in walkdir::WalkDir::new(&dir) {
                    match f {
                        Ok(f) => {
                            if f.file_type().is_file() && is_audio(f.path()) {
                                let string_path = f
                                    .path()
                                    .as_os_str()
                                    .to_str()
                                    .ok_or_else(|| anyhow!("Non UTF path"));
                                let media_file = string_path.and_then(|string_path| {
                                    media_info::MediaFile::open(string_path)
                                        .map_err(|e| anyhow!(e))
                                        .map(|f| (f, string_path))
                                });

                                match media_file {
                                    Ok((media_file, path)) => {
                                        let title = media_file.title();
                                        let artist = media_file.artist();
                                        let composer = media_file.composer();
                                        let album = media_file.album();
                                        let year = media_file.meta("year");
                                        let comment = media_file.meta("comment");
                                        let description = media_file.meta("description");
                                        let genre = media_file.genre();
                                        let duration = media_file.duration() as i64;
                                        let bitrate = media_file.bitrate();
                                        let num_chapters = media_file.chapters_count() as i64;

                                        println!("File {} metadata: {:?}", path, media_file.all_meta());

                                        let id = sqlx::query_scalar!(
                                            r#"select id as "id!" from tags where path = ?"#,
                                            path
                                        )
                                        .fetch_optional(&pool)
                                        .await?;

                                        match id {
                                            Some(id) => {
                                                let n = sqlx::query!(r#"
                                                update tags set title = ?, 
                                                artist = ?,
                                                composer =?,
                                                album = ?,
                                                year = ?,
                                                comment = ?,
                                                description = ?,
                                                genre = ?,
                                                duration = ?,
                                                bitrate = ?,
                                                num_chapters = ?,
                                                ts = CURRENT_TIMESTAMP
                                                where id = ?"#,
                                                title, artist, composer, album, year, comment, description, genre, duration, 
                                                bitrate, num_chapters, id)
                                                .execute(&pool)
                                                .await?
                                                .rows_affected();
                                                debug_assert_eq!(n,1);
                                            }
                                            None => {
                                                let _id =sqlx::query!(r#"
                                                insert into tags 
                                                (path, title, artist, composer, album, year, comment, description, 
                                                    genre, duration, bitrate, num_chapters) 
                                                values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                                                "#,
                                                path,
                                                title,
                                                artist,
                                                composer,
                                                album,
                                                year,
                                                comment,
                                                description,
                                                genre,
                                                duration,
                                                bitrate,
                                                num_chapters
                                                ).execute(&pool)
                                                .await?
                                                .last_insert_rowid();
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Error {} when extracting meta from {:?}",
                                            e,
                                            f.path()
                                        )
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e)
                        }
                    }
                }
            }
        }
        Cmd::Clear => {
            let n = sqlx::query!("delete from tags")
                .execute(&pool)
                .await?
                .rows_affected();
            println!("Deleted {} rows", n);
        }
    }

    Ok(())
}

fn is_audio<P: AsRef<Path>>(path: P) -> bool {
    mime_guess::from_path(path)
        .first()
        .map(|m| m.type_() == mime::AUDIO)
        .unwrap_or(false)
}
