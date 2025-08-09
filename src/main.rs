use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use actix::{
    fut, Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message as ActixMessage, Recipient, StreamHandler, WrapFuture,
};
use actix_cors::Cors;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use chrono::{DateTime, Utc};
use log;
use rand::{self, Rng};
use serde::{Deserialize, Serialize};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

// --- WebSocket Actor: WsChatSession ---

struct WsChatSession {
    id: usize,
    hb: Instant,
    addr: Addr<BroadcastServer>,
}

impl WsChatSession {
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                log::info!("Websocket Client {} timed out", act.id);
                act.addr.do_send(Disconnect { id: act.id });
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        let addr = ctx.address();
        self.addr
            .send(Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        self.addr.do_send(Disconnect { id: self.id });
        actix::Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

impl Handler<WsMessage> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

// --- WebSocket Actor: BroadcastServer ---

#[derive(ActixMessage)]
#[rtype(result = "()")]
struct WsMessage(String);

#[derive(ActixMessage)]
#[rtype(result = "usize")]
struct Connect {
    addr: Recipient<WsMessage>,
}

#[derive(ActixMessage)]
#[rtype(result = "()")]
struct Disconnect {
    id: usize,
}

struct BroadcastServer {
    sessions: HashMap<usize, Recipient<WsMessage>>,
    rng: rand::rngs::ThreadRng,
}

impl BroadcastServer {
    fn new() -> Self {
        BroadcastServer {
            sessions: HashMap::new(),
            rng: rand::thread_rng(),
        }
    }

    fn send_message(&self, message: &str) {
        for addr in self.sessions.values() {
            addr.do_send(WsMessage(message.to_owned()));
        }
    }
}

impl Actor for BroadcastServer {
    type Context = actix::Context<Self>;
}

impl Handler<Connect> for BroadcastServer {
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Self::Context) -> Self::Result {
        let id = self.rng.gen::<usize>();
        self.sessions.insert(id, msg.addr);
        id
    }
}

impl Handler<Disconnect> for BroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Self::Context) {
        self.sessions.remove(&msg.id);
    }
}

impl Handler<WsMessage> for BroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, _: &mut Self::Context) {
        self.send_message(&msg.0);
    }
}

// --- Shared State for Viewer Tracking ---

#[derive(Clone)]
struct AppState {
    viewers: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    server: Addr<BroadcastServer>,
}

impl AppState {
    fn new(server: Addr<BroadcastServer>) -> Self {
        AppState {
            viewers: Arc::new(Mutex::new(HashMap::new())),
            server,
        }
    }

    fn prune_and_get_count(&self) -> usize {
        let mut viewers = self.viewers.lock().unwrap();
        let now = Utc::now();
        viewers.retain(|_, last_seen| now.signed_duration_since(*last_seen).num_seconds() < 15);
        viewers.len()
    }
}

// --- API Data Structures ---

#[derive(Deserialize)]
struct HeartbeatPayload {
    client_id: String,
}

#[derive(Serialize)]
struct Video {
    name: String,
}

#[derive(Serialize)]
struct ViewersResponse {
    count: usize,
}

// --- API Endpoints ---

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(include_str!("../static/index.html"))
}

async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let session = WsChatSession {
        id: 0,
        hb: Instant::now(),
        addr: state.get_ref().server.clone(),
    };
    ws::start(session, &req, stream)
}

#[get("/videos")]
async fn list_videos() -> impl Responder {
    let mut videos = Vec::new();
    if let Ok(paths) = fs::read_dir("./av") {
        for path in paths.flatten() {
            let path = path.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mp4") {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    videos.push(Video {
                        name: name.to_string(),
                    });
                }
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

    let file = match fs::File::open(&video_path) {
        Ok(file) => file,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let file_size = file.metadata().unwrap().len();

    let range_header = req.headers().get("range").and_then(|h| h.to_str().ok());

    if let Some(range_str) = range_header {
        if let Some(range) = parse_range(range_str, file_size) {
            let (start, end) = range;
            let chunk_size = end - start + 1;
            let mut stream = file;
            let mut buffer = vec![0; chunk_size as usize];

            if stream.seek(std::io::SeekFrom::Start(start)).is_err()
                || stream.read_exact(&mut buffer).is_err()
            {
                return HttpResponse::InternalServerError().finish();
            }

            return HttpResponse::PartialContent()
                .append_header(("Content-Type", "video/mp4"))
                .append_header(("Content-Length", chunk_size.to_string()))
                .append_header((
                    "Content-Range",
                    format!("bytes {}-{}/{}", start, end, file_size),
                ))
                .body(buffer);
        }
    }

    match fs::read(&video_path) {
        Ok(buffer) => HttpResponse::Ok()
            .append_header(("Content-Type", "video/mp4"))
            .append_header(("Content-Length", file_size.to_string()))
            .body(buffer),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

fn parse_range(range_str: &str, file_size: u64) -> Option<(u64, u64)> {
    let range_str = range_str.strip_prefix("bytes=")?;
    let parts: Vec<&str> = range_str.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start = parts[0].parse::<u64>().ok()?;
    if start >= file_size {
        return None;
    }

    let end = parts[1].parse::<u64>().unwrap_or(file_size - 1);
    Some((start, end.min(file_size - 1)))
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

fn start_viewer_count_broadcaster(app_state: web::Data<AppState>) {
    let mut last_count = 0;
    actix::spawn(async move {
        loop {
            actix_web::rt::time::sleep(Duration::from_secs(2)).await;
            let current_count = app_state.prune_and_get_count();
            if current_count != last_count {
                last_count = current_count;
                let response = ViewersResponse {
                    count: current_count,
                };
                if let Ok(json_response) = serde_json::to_string(&response) {
                    let server = &app_state.server;
                    server.do_send(WsMessage(json_response));
                }
            }
        }
    });
}

// --- Main Server Setup ---
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let server = BroadcastServer::new().start();
    let app_state = web::Data::new(AppState::new(server));

    start_viewer_count_broadcaster(app_state.clone());

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(app_state.clone())
            .wrap(cors)
            .service(index)
            .service(list_videos)
            .service(stream_video)
            .service(heartbeat)
            .route("/ws", web::get().to(ws_route))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
