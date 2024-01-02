use std::io::prelude::*; // Contains the read/write traits
use std::net::{TcpListener, TcpStream};
use std::io;
use std::thread;
use url::Url;

const PORT: i32 = 8888;

fn handle_client(mut stream: TcpStream) {
    
    let mut buf = [0; 4096];

    // Read the bytes from the stream
    _ = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return () // early return if an error
    }; 
    
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut request = httparse::Request::new(&mut headers);
    _ = request.parse(&buf).unwrap();

    let method = request.method.unwrap();
    let mut website = String::from(request.path.unwrap());

    if !method.eq("CONNECT") {
        let url = Url::parse(&website).unwrap();
        let port_site = match url.port() {
            None => 80,
            Some(p) => p
        }; 
        
        website.push_str(":");
        website.push_str(&port_site.to_string());
    }

    println!("[*] Connecting to {}", website);
    let mut tunnel = match TcpStream::connect(website) {
        Ok(t) => t,
        Err(err) => {
            println!("Error: {err}");
            return ()
        }
    };

    // Send an ack to the client
    if method.eq("CONNECT") {
        match stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n") {
            Ok(_) => (),
            Err(_) => return ()
        };
    } else {
        match tunnel.write_all( &buf) {
            Ok(_) => (),
            Err(_) => return ()
        };
       
    }

    // Set both sockets to nonblocking mode
    match stream.set_nonblocking(true) {
        Ok(()) => (),
        Err(_) => return ()
    };
    match tunnel.set_nonblocking(true) {
        Ok(()) => (),
        Err(_) => return ()
    }
    let mut stream_buf = [0u8; 4096]; // Buffer containing data received from stream
    let mut tunnel_buf = [0u8; 4096]; // Buffer containing data received from tunnel
    let mut stream_nbytes = 0usize; // The number of bytes pending in stream_buf to be written to tunnel
    let mut tunnel_nbytes = 0usize; // The number of bytes pending in tunnel_buf to be written to stream
    
    // Keep copying data back and forth
    loop {
        // Read data from stream to be sent to tunnel -- only read if stream_buf is empty
        if stream_nbytes == 0 {
            stream_nbytes = match stream.read(&mut stream_buf) {
                Ok(0) => return (), // Socket closed 
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => 0, // If there is no data, return 0 bytes written
                Err(_) => return ()
            };
        }
        // Read data from tunnel to be sent to stream -- only read if tunnel_buf is empty
        if tunnel_nbytes == 0 {
            tunnel_nbytes = match tunnel.read(&mut tunnel_buf) {
                Ok(0) => return (), // Socket closed 
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => 0, // If there is no data, return 0 bytes written
                Err(_) => return ()
            };
        }
        // Write data from stream to tunnel
        if stream_nbytes > 0 {
            // Pass the slice corresponding to first `stream_nbytes`
            match tunnel.write(&mut stream_buf[0..stream_nbytes]) {
                Ok(0) => return (), // Socket closed 
                Ok(n) if n == stream_nbytes => stream_nbytes = 0, // If we get equal 
                Ok(_) => { println!("Cannot write partially :("); return () }, // No support for partial nbytes
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => (), // Write the bytes in later
                Err(_) => return ()
            }
        }
        // Write data from tunnel to stream
        if tunnel_nbytes > 0 {
            // Pass the slice corresponding to first `stream_nbytes`
            match stream.write(&mut tunnel_buf[0..tunnel_nbytes]) {
                Ok(0) => return (), // Socket closed
                Ok(n) if n == tunnel_nbytes => tunnel_nbytes = 0, // If we get equal 
                Ok(_) => { println!("Cannot write partially :("); return () }, // No support for partial nbytes
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => (), // Write the bytes in later
                Err(_) => return ()
            }
        }
    }
 }


fn main() -> std::io::Result<()> {
    // Create a server
    let local_addr = format!("localhost:{}", PORT);
    let server = TcpListener::bind(local_addr)?;
    println!("[*] Listening on port {}", PORT);
    // Keep spinning and spawn threads for any incoming connections
    for stream_result in server.incoming() {
        match stream_result {
            Ok(stream) => thread::spawn(move || handle_client(stream)), // Spawn a new thread, ignore the return value because we don't need to join threads
            _          => continue
        };
    }
    Ok(())
}