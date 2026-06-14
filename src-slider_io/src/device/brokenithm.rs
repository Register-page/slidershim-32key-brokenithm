use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use hyper::{
  header,
  server::conn::AddrStream,
  service::{make_service_fn, service_fn},
  upgrade::{self, Upgraded},
  Body, Method, Request, Response, Server, StatusCode,
};
use log::{error, info};
use phf::phf_map;
use std::{
  convert::Infallible,
  future::Future,
  net::SocketAddr,
  sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
  },
};
use tokio::{
  select,
  sync::mpsc,
  time::{sleep, Duration},
};
use tokio_tungstenite::WebSocketStream;
use tungstenite::{handshake, Message};

use crate::{device::config::BrokenithmSpec, shared::worker::AsyncHaltableJob, state::SliderState};

// https://levelup.gitconnected.com/handling-websocket-and-http-on-the-same-port-with-rust-f65b770722c9

enum BrokenithmMessage {
  Alive,
  Input { ground: [u8; 32], air: [u8; 6] },
}

fn parse_brokenithm_message(message: &str) -> Option<BrokenithmMessage> {
  if message == "alive?" {
    return Some(BrokenithmMessage::Alive);
  }

  let bytes = message.as_bytes();
  if bytes.len() != 39 || bytes[0] != b'b' || !bytes[1..].iter().all(|b| matches!(b, b'0' | b'1')) {
    return None;
  }

  let mut ground = [0; 32];
  let mut air = [0; 6];
  for (target, source) in ground.iter_mut().zip(bytes[1..33].iter()) {
    *target = if *source == b'1' { 255 } else { 0 };
  }
  for (target, source) in air.iter_mut().zip(bytes[33..39].iter()) {
    *target = if *source == b'1' { 1 } else { 0 };
  }

  Some(BrokenithmMessage::Input { ground, air })
}

fn reset_input(state: &SliderState) {
  let mut input_handle = state.input.lock();
  input_handle.ground.fill(0);
  input_handle.air.fill(0);
}

async fn error_response() -> Result<Response<Body>, Infallible> {
  Ok(
    Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body(Body::from("Not found"))
      .unwrap(),
  )
}

// static x: &'static [u8] = include_bytes!("./brokenithm-www/favicon.ico");

static BROKENITHM_STR_FILES: phf::Map<&'static str, (&'static str, &'static str)> = phf_map! {
  "app.js" => (include_str!("./brokenithm-www/app.js"), "text/javascript"),
  "config.js" => (include_str!("./brokenithm-www/config.js"), "text/javascript"),
  "index-ns.html" => (include_str!("./brokenithm-www/index-ns.html"), "text/html"),
  "index-go.html" => (include_str!("./brokenithm-www/index-go.html"), "text/html"),
  "index.html" => (include_str!("./brokenithm-www/index.html"), "text/html"),
};

static BROKENITHM_BIN_FILES: phf::Map<&'static str, (&'static [u8], &'static str)> = phf_map! {
  "favicon.ico" => (include_bytes!("./brokenithm-www/favicon.ico"), "image/x-icon"),
  "icon.png" => (include_bytes!("./brokenithm-www/icon.png"), "image/png"),
};

async fn serve_file(path: &str) -> Result<Response<Body>, Infallible> {
  match (
    BROKENITHM_STR_FILES.get(path),
    BROKENITHM_BIN_FILES.get(path),
  ) {
    (Some((data, mime)), _) => Ok(
      Response::builder()
        .header(header::CONTENT_TYPE, *mime)
        .body(Body::from(*data))
        .unwrap(),
    ),
    (_, Some((data, mime))) => Ok(
      Response::builder()
        .header(header::CONTENT_TYPE, *mime)
        .body(Body::from(*data))
        .unwrap(),
    ),
    (None, None) => error_response().await,
  }
}

async fn handle_brokenithm(
  ws_stream: WebSocketStream<Upgraded>,
  state: SliderState,
  lights_enabled: bool,
  active_session: Arc<AtomicUsize>,
  session_id: usize,
) {
  let (mut ws_write, mut ws_read) = ws_stream.split();

  let (msg_write, mut msg_read) = mpsc::unbounded_channel::<Message>();

  let write_task = async move {
    // info!("Websocket write task open");
    while let Some(msg) = msg_read.recv().await {
      if ws_write.send(msg).await.is_err() {
        break;
      }
    }
    // info!("Websocket write task done");
  };

  let msg_write_handle = msg_write.clone();
  let state_handle = state.clone();
  let read_active_session = active_session.clone();
  let read_task = async move {
    // info!("Websocket read task open");
    while let Some(msg) = ws_read.next().await {
      match msg {
        Ok(msg) => match msg {
          Message::Text(msg) => match parse_brokenithm_message(&msg) {
            Some(BrokenithmMessage::Alive) => {
              msg_write_handle
                .send(Message::Text("alive".to_string()))
                .ok();
            }
            Some(BrokenithmMessage::Input { ground, air }) => {
              if read_active_session.load(Ordering::SeqCst) == session_id {
                let mut input_handle = state_handle.input.lock();
                input_handle.ground = ground;
                input_handle.air = air;
              }
            }
            None => {
              error!("Invalid Brokenithm message");
              break;
            }
          },
          Message::Close(_) => {
            info!("Websocket connection closed");
            break;
          }
          _ => {}
        },
        Err(e) => {
          error!("Websocket connection error: {}", e);
          break;
        }
      }
    }
    // info!("Websocket read task done");
  };

  match lights_enabled {
    false => {
      select! {
        _ = read_task => {}
        _ = write_task => {}
      };
    }
    true => {
      let msg_write_handle = msg_write.clone();
      let state_handle = state.clone();
      let lights_task = async move {
        loop {
          let mut lights_data = vec![0; 93];
          {
            let lights_handle = state_handle.lights.lock();
            lights_data.copy_from_slice(&lights_handle.ground);
          }
          msg_write_handle.send(Message::Binary(lights_data)).ok();

          sleep(Duration::from_millis(50)).await;
        }
      };

      select! {
        _ = read_task => {}
        _ = write_task => {}
        _ = lights_task => {}
      };
    }
  }

  if active_session.load(Ordering::SeqCst) == session_id {
    reset_input(&state);
  }
}

async fn handle_websocket(
  mut request: Request<Body>,
  state: SliderState,
  lights_enabled: bool,
  active_session: Arc<AtomicUsize>,
) -> Result<Response<Body>, Infallible> {
  let res = match handshake::server::create_response_with_body(&request, Body::empty) {
    Ok(res) => {
      tokio::spawn(async move {
        match upgrade::on(&mut request).await {
          Ok(upgraded) => {
            let session_id = active_session.fetch_add(1, Ordering::SeqCst) + 1;
            reset_input(&state);
            let ws_stream = WebSocketStream::from_raw_socket(
              upgraded,
              tokio_tungstenite::tungstenite::protocol::Role::Server,
              None,
            )
            .await;

            handle_brokenithm(ws_stream, state, lights_enabled, active_session, session_id).await;
          }

          Err(e) => {
            error!("Websocket upgrade error: {}", e);
          }
        }
      });

      res
    }
    Err(e) => {
      error!("Websocket creation error: {}", e);
      Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(format!("Failed to create websocket: {}", e)))
        .unwrap()
    }
  };
  Ok(res)
}

async fn handle_request(
  request: Request<Body>,
  remote_addr: SocketAddr,
  state: SliderState,
  spec: BrokenithmSpec,
  lights_enabled: bool,
  active_session: Arc<AtomicUsize>,
) -> Result<Response<Body>, Infallible> {
  let method = request.method();
  let path = request.uri().path();
  if method != Method::GET {
    error!(
      "Server unknown method {} -> {} {}",
      remote_addr, method, path
    );
    return error_response().await;
  }
  info!("Server {} -> {} {}", remote_addr, method, path);

  match (
    request.uri().path(),
    request.headers().contains_key(header::UPGRADE),
  ) {
    ("/", false) | ("/index.html", false) => match spec {
      BrokenithmSpec::Basic => serve_file("index.html").await,
      BrokenithmSpec::GroundOnly => serve_file("index-go.html").await,
      BrokenithmSpec::Nostalgia => serve_file("index-ns.html").await,
    },
    (filename, false) => serve_file(&filename[1..]).await,
    ("/ws", true) => handle_websocket(request, state, lights_enabled, active_session).await,
    _ => error_response().await,
  }
}

pub struct BrokenithmJob {
  state: SliderState,
  spec: BrokenithmSpec,
  lights_enabled: bool,
  port: u16,
}

impl BrokenithmJob {
  pub fn new(
    state: &SliderState,
    spec: &BrokenithmSpec,
    lights_enabled: &bool,
    port: &u16,
  ) -> Self {
    Self {
      state: state.clone(),
      spec: spec.clone(),
      lights_enabled: *lights_enabled,
      port: *port,
    }
  }
}

#[async_trait]
impl AsyncHaltableJob for BrokenithmJob {
  async fn run<F: Future<Output = ()> + Send>(self, stop_signal: F) {
    let state = self.state.clone();
    let spec = self.spec.clone();
    let lights_enabled = self.lights_enabled;
    let active_session = Arc::new(AtomicUsize::new(0));
    let make_svc = make_service_fn(|conn: &AddrStream| {
      let remote_addr = conn.remote_addr();
      let make_svc_state = state.clone();
      let make_spec = spec.clone();
      let make_active_session = active_session.clone();
      async move {
        Ok::<_, Infallible>(service_fn(move |request: Request<Body>| {
          let svc_state = make_svc_state.clone();
          let spec = make_spec.clone();
          let active_session = make_active_session.clone();
          handle_request(
            request,
            remote_addr,
            svc_state,
            spec,
            lights_enabled,
            active_session,
          )
        }))
      }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
    info!("Brokenithm server listening on {}", addr);

    let server = Server::bind(&addr)
      // .http1_keepalive(false)
      // .http2_keep_alive_interval(None)
      // .tcp_keepalive(None)
      .serve(make_svc)
      .with_graceful_shutdown(stop_signal);

    if let Err(e) = server.await {
      info!("Brokenithm server stopped: {}", e);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{parse_brokenithm_message, BrokenithmMessage};

  #[test]
  fn parses_heartbeat() {
    assert!(matches!(
      parse_brokenithm_message("alive?"),
      Some(BrokenithmMessage::Alive)
    ));
  }

  #[test]
  fn parses_all_ground_and_air_bits() {
    let message = format!("b{}{}", "10".repeat(16), "101010");
    match parse_brokenithm_message(&message) {
      Some(BrokenithmMessage::Input { ground, air }) => {
        assert_eq!(ground[0], 255);
        assert_eq!(ground[1], 0);
        assert_eq!(ground[30], 255);
        assert_eq!(ground[31], 0);
        assert_eq!(air, [1, 0, 1, 0, 1, 0]);
      }
      _ => panic!("expected input message"),
    }
  }

  #[test]
  fn rejects_malformed_messages() {
    assert!(parse_brokenithm_message("b000").is_none());
    assert!(parse_brokenithm_message(&format!("x{}{}", "0".repeat(32), "0".repeat(6))).is_none());
    assert!(parse_brokenithm_message(&format!("b{}{}", "0".repeat(31), "x".repeat(7))).is_none());
  }
}
