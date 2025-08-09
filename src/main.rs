use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

// --- Shared State for Viewer Tracking ---
#[derive(Clone)]
struct AppState {
    viewers: web::Data<Mutex<HashMap<String, DateTime<Utc>>>>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            viewers: web::Data::new(Mutex::new(HashMap::new())),
        }
    }

    fn prune_and_get_count(&self) -> usize {
        let mut viewers = self.viewers.lock().unwrap();
        let now = Utc::now();
        viewers.retain(|_, last_seen| now.signed_duration_since(*last_seen).num_seconds() < 15);
        viewers.len()
    }
}

#[derive(Deserialize)]
struct HeartbeatPayload {
    client_id: String,
}

#[derive(Serialize)]
struct ViewersResponse {
    count: usize,
}

// --- Video Data Structures ---
#[derive(Serialize)]
struct Video {
    name: String,
}

// --- API Endpoints ---

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(include_str!("../static/index.html"))
}

#[get("/videos")]
async fn list_videos() -> impl Responder {
    let mut videos = Vec::new();
    let paths = fs::read_dir("./av").unwrap();

    for path in paths {
        let path = path.unwrap().path();
        if let Some(extension) = path.extension() {
            if extension == "mp4" {
                videos.push(Video {
                    name: path.file_name().unwrap().to_str().unwrap().to_string(),
                });
            }
        }
    }

    HttpResponse::Ok().json(videos)
}

#[get("/video/{filename}")]
async fn stream_video(req: HttpRequest) -> impl Responder {
    let filename: PathBuf = req.match_info().query("filename").parse().unwrap();
    let video_path = PathBuf::from("./av").join(filename);

    if !video_path.exists() {
        return HttpResponse::NotFound().finish();
    }

    let file = fs::File::open(&video_path).unwrap();
    let file_size = file.metadata().unwrap().len();

    let range = req
        .headers()
        .get("range")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("bytes="))
        .and_then(|s| {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() == 2 {
                let start = parts[0].parse::<u64>().ok();
                let end = parts[1].parse::<u64>().ok();
                Some((start, end))
            } else {
                None
            }
        });

    match range {
        Some((Some(start), end_opt)) => {
            let end = end_opt.unwrap_or(file_size - 1);
            let chunk_size = end - start + 1;

            let mut stream = file;
            let mut buffer = vec![0; chunk_size as usize];
            use std::io::Seek;
            stream.seek(std::io::SeekFrom::Start(start)).unwrap();
            stream.read_exact(&mut buffer).unwrap();

            HttpResponse::PartialContent()
                .append_header(("Content-Type", "video/mp4"))
                .append_header(("Content-Length", chunk_size.to_string()))
                .append_header((
                    "Content-Range",
                    format!("bytes {}-{}/{}", start, end, file_size),
                ))
                .body(buffer)
        }
        _ => {
            let mut buffer = Vec::new();
            let mut file = fs::File::open(&video_path).unwrap();
            file.read_to_end(&mut buffer).unwrap();

            HttpResponse::Ok()
                .append_header(("Content-Type", "video/mp4"))
                .append_header(("Content-Length", file_size.to_string()))
                .body(buffer)
        }
    }
}

#[post("/heartbeat")]
async fn heartbeat(
    state: web::Data<AppState>,
    payload: web::Json<HeartbeatPayload>,
) -> impl Responder {
    let mut viewers = state.viewers.lock().unwrap();
    viewers.insert(payload.client_id.clone(), Utc::now());
    HttpResponse::Ok().finish()
}

#[get("/viewers")]
async fn get_viewers(state: web::Data<AppState>) -> impl Responder {
    let count = state.prune_and_get_count();
    HttpResponse::Ok().json(ViewersResponse { count })
}

// --- Main Server Setup ---
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_state = AppState::new();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(cors)
            .service(index)
            .service(list_videos)
            .service(stream_video)
            .service(heartbeat)
            .service(get_viewers)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
