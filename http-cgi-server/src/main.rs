use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    println!("CGI HTTP Server listening on http://127.0.0.1:8080");
    println!("CGI scripts should be placed in ./cgi-bin/");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    handle_connection(stream);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer).unwrap();
    let request = String::from_utf8_lossy(&buffer[..n]);

    let mut lines = request.lines();
    let first_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 3 {
        send_error(stream, "400 Bad Request");
        return;
    }

    let method = parts[0];
    let path = parts[1];

    // Parse headers
    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(": ") {
            headers.insert(key.to_lowercase(), value.to_string());
        }
    }

    // Check if this is a CGI request
    if path.starts_with("/cgi-bin/") {
        handle_cgi_request(stream, method, path, headers);
    } else {
        send_error(stream, "404 Not Found");
    }
}

fn handle_cgi_request(
    mut stream: TcpStream,
    method: &str,
    path: &str,
    headers: HashMap<String, String>,
) {
    let script_name = path.trim_start_matches("/cgi-bin/");
    let script_path = format!("./cgi-bin/{}", script_name);

    let mut absolute_script_path = std::env::current_dir().unwrap();
    absolute_script_path.push(&script_path);
    absolute_script_path = absolute_script_path.canonicalize().unwrap();

    // Check if script exists
    if !absolute_script_path.exists() {
        send_error(stream, "404 Not Found");
        return;
    }

    // Set up CGI environment variables
    let mut env_vars = HashMap::new();
    let content_length = headers
        .get("content-length")
        .unwrap_or(&"0".to_string())
        .clone();
    let content_type = headers
        .get("content-type")
        .unwrap_or(&"text/plain".to_string())
        .clone();

    env_vars.insert("REQUEST_METHOD".to_string(), method.to_string());
    env_vars.insert("CONTENT_LENGTH".to_string(), content_length);
    env_vars.insert("REQUEST_URI".to_string(), path.to_string());
    env_vars.insert("QUERY_STRING".to_string(), "".to_string());
    env_vars.insert("CONTENT_TYPE".to_string(), content_type);
    env_vars.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
    env_vars.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
    env_vars.insert(
        "SERVER_SOFTWARE".to_string(),
        "http-cgi-server/1.0".to_string(),
    );
    env_vars.insert("REMOTE_ADDR".to_string(), "127.0.0.1".to_string());
    env_vars.insert("REMOTE_PORT".to_string(), "12345".to_string());
    env_vars.insert("SERVER_ADDR".to_string(), "127.0.0.1".to_string());
    env_vars.insert("SERVER_PORT".to_string(), "8080".to_string());
    env_vars.insert("SERVER_NAME".to_string(), "localhost".to_string());
    env_vars.insert("DOCUMENT_ROOT".to_string(), ".".to_string());
    env_vars.insert("SCRIPT_NAME".to_string(), path.to_string());
    env_vars.insert("PATH_INFO".to_string(), "".to_string());
    env_vars.insert("PATH_TRANSLATED".to_string(), script_path.clone());

    // Add HTTP headers as environment variables
    for (key, value) in headers {
        let env_key = format!("HTTP_{}", key.to_uppercase().replace("-", "_"));
        env_vars.insert(env_key, value);
    }

    // Execute the CGI script
    let mut command = Command::new(&absolute_script_path);

    // Set environment variables
    for (key, value) in env_vars {
        command.env(key, value);
    }

    let output = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            // Send any input to the CGI script
            if let Some(mut stdin) = child.stdin.take() {
                // For POST requests, you'd read the body here
                stdin.write_all(b"").unwrap();
            }

            // Read the output
            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();

            let mut output = Vec::new();
            let mut error_output = Vec::new();

            stdout.read_to_end(&mut output).unwrap();
            stderr.read_to_end(&mut error_output).unwrap();

            // Wait for the process to complete
            let status = child.wait().unwrap();

            // Parse and send the CGI response
            let response = String::from_utf8_lossy(&output);
            send_cgi_response(stream, &response);

            if !status.success() {
                eprintln!("CGI script exited with status: {}", status);
            }
        }
        Err(e) => {
            eprintln!("Failed to execute CGI script: {}", e);
            send_error(stream, "500 Internal Server Error");
        }
    }
}

fn send_cgi_response(mut stream: TcpStream, cgi_output: &str) {
    // Split the CGI output into headers and body
    let parts: Vec<&str> = cgi_output.split("\r\n\r\n").collect();

    if parts.len() >= 2 {
        // CGI output has headers and body
        let headers = parts[0];
        let body = parts[1..].join("\r\n\r\n");

        // Send HTTP status line
        stream.write_all(b"HTTP/1.1 200 OK\r\n").unwrap();

        // Send CGI headers
        stream.write_all(headers.as_bytes()).unwrap();
        stream.write_all(b"\r\n\r\n").unwrap();

        // Send body
        stream.write_all(body.as_bytes()).unwrap();
    } else {
        // No headers found, treat as raw content
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n")
            .unwrap();
        stream.write_all(cgi_output.as_bytes()).unwrap();
    }
}

fn send_error(mut stream: TcpStream, status: &str) {
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        status,
        status.len(),
        status
    );
    stream.write_all(response.as_bytes()).unwrap();
}
