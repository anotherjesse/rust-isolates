use v8;

use std::fs;
use std::net::SocketAddr;

use hyper::server::conn::Http;
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, StatusCode};
use tokio::net::TcpListener;

async fn echo(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // Serve some instructions at /
        (&Method::GET, "/") => {
            let contents = fs::read_to_string("ide.html").unwrap();
            Ok(Response::new(Body::from(contents)))
        }

        // Simply echo the body back to the client.
        (&Method::POST, "/run") => {
            let body_bytes = hyper::body::to_bytes(req.into_body()).await?;
            let src = String::from_utf8(body_bytes.to_vec()).unwrap();
            let result = run(&src);
            Ok(Response::new(Body::from(result)))
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn print(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let result = args
        .get(0)
        .to_string(scope)
        .unwrap()
        .to_rust_string_lossy(scope);

    let name = v8::String::new(scope, "returned string").unwrap();

    // rv.set_empty_string();
    rv.set(name.into());
    // rv.set(name);
    println!("printing: {}", result);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    init_v8();

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;

        tokio::task::spawn(async move {
            if let Err(err) = Http::new().serve_connection(stream, service_fn(echo)).await {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

fn init_v8() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
}

fn run(code_str: &str) -> String {
    println!("src: {}", code_str);

    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

    let object_template = v8::ObjectTemplate::new(scope);
    let function_template = v8::FunctionTemplate::new(scope, print);
    let name = v8::String::new(scope, "print").unwrap();

    object_template.set(name.into(), function_template.into());
    let context = v8::Context::new_from_template(scope, object_template);

    let scope = &mut v8::ContextScope::new(scope, context);

    let code = v8::String::new(scope, code_str).unwrap();
    // println!("javascript code: {}", code.to_rust_string_lossy(scope));

    match v8::Script::compile(scope, code, None) {
        Some(script) => {
            let result = script.run(scope).unwrap();
            let result = result.to_string(scope).unwrap();
            return result.to_rust_string_lossy(scope);
        }
        None => {
            return "error compiling".to_string();
        }
    }
}
