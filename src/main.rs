use std::io::prelude::*; 
use std::net::{TcpListener, TcpStream};
use std::{io, env};
use std::thread;
use url::Url;
use regex::Regex;

const PORT: i32 = 8888;
const PATH_DB: &str = "/data/activities.db";
static mut BYPASS: bool = false;
const DEAMON_MODE: bool = true;

struct Database { 
    in_memory: bool,
    path: String,
    connection: Option<sqlite::Connection>,
    by_pass: bool
}

impl Database {
    fn read_db(&mut self, path: String) {
        //self.in_memory = in_memory;
        self.path = path.clone();
        self.in_memory = self.path.is_empty();

        self.connection = match self.in_memory {
            true => Option::from(sqlite::open(":memory:").unwrap()),
            false => Option::from(sqlite::open(&self.path).unwrap())
        };
    }

    fn permessive_mode(&mut self) -> bool {
        let query = String::from("SELECT permessive from CONFIG");
        match self.connection.as_mut() {
            Some(connection) => {
                let mut statement = connection.prepare(query).unwrap();
                match statement.next() {
                    Ok(_) => statement.read::<i64, _>("permessive").unwrap() == 1,
                    Err(_) => false,
                }
            },
            None => false //panic!("Error"),
        }
    }

    fn enable_bypass_from_db(&mut self) -> bool {
        let query = String::from("SELECT bypass from CONFIG");
        match self.connection.as_mut() {
            Some(connection) => {
                let mut statement = connection.prepare(query).unwrap();
                match statement.next() {
                    Ok(_) => statement.read::<i64, _>("bypass").unwrap() == 1,
                    Err(_) => false,
                }
            },
            None => false //panic!("Error"),
        }
    }

    fn has_hostname(&mut self, hostname: &str) -> bool { 
        let mut query = String::from("SELECT count(hostname) as len from activity where ");

        let split_hostname: Vec<_> = hostname.split('.').collect();
        let num_sub_domain = split_hostname.len();

        let res = &split_hostname[1..(num_sub_domain-1)];

        query.push_str("hostname = '");
        query.push_str(hostname);
        query.push('\'');

        for (i, _el) in res.iter().enumerate() {
            query.push_str(" or ");
            
            let host2 = split_hostname[(i+1)..(num_sub_domain)].join(".");
            query.push_str("hostname = '*.");
            query.push_str(&host2);
            query.push('\'');
        }

        match self.connection.as_mut() {
            Some(connection) => {
                let mut statement = connection.prepare(query).unwrap();
                match statement.next() {
                    Ok(_) => statement.read::<i64, _>("len").unwrap() > 0,
                    Err(err) =>  {
                        println!("{}", err);
                        false
                    }
                }
            },
            None => false //panic!("Error"),
        }
    }

    fn valid_hostname(&mut self, hostname: &str) -> bool {
        let mut query = String::from("SELECT coalesce(valid, 0) as valid from ( SELECT max(priority), valid from activity where ");
        
        let split_hostname: Vec<_> = hostname.split('.').collect();
        let num_sub_domain = split_hostname.len();

        let res = &split_hostname[1..(num_sub_domain-1)];

        query.push_str("hostname = '");
        query.push_str(hostname);
        query.push('\'');

        for (i, _el) in res.iter().enumerate() {
            query.push_str(" or ");
            
            let host2 = split_hostname[(i+1)..(num_sub_domain)].join(".");
            query.push_str("hostname = '*.");
            query.push_str(&host2);
            query.push('\'');
        }
        query.push(')');
        // println!("{}", query);

        match self.connection.as_mut() {
            Some(connection) => {
                let mut statement = connection.prepare(query).unwrap();
                match statement.next() {
                    Ok(_) => statement.read::<i64, _>("valid").unwrap() == 1,
                    Err(_) => false,
                }
            },
            None => false //panic!("Error"),
        }

    }
}

// "/data/activities.db"



fn handle_client(mut stream: TcpStream) {
    let mut db = Database { 
        in_memory: true, 
        path: String::from(PATH_DB), 
        connection: None,
        by_pass: unsafe { BYPASS }
    };
    db.read_db(String::from(PATH_DB));

    let mut buf = [0; 4096];

    // Read the bytes from the stream
    _ = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return 
    }; 
    
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut request = httparse::Request::new(&mut headers);
    // _ = request.parse(&buf).unwrap();
    _ = match request.parse(&buf) {
        Ok(_) => true,
        Err(_) => return,
    };
    
    let mut website = String::from(request.path.unwrap());

    let re = Regex::new(r":[0-9]*").unwrap();
    let result = re.replace_all(&website, "");

    let bypass = !db.enable_bypass_from_db() && !db.by_pass;
    let permissive_mode = db.permessive_mode();

    let has_hostname = db.has_hostname(&result);
    let is_not_valid = db.valid_hostname(&result);

    if permissive_mode && !has_hostname {

    } else if !is_not_valid && !bypass {
        if !DEAMON_MODE { 
            println!("Blocking {result}");
        }

        let response =  format!("Blocked website: {}\n", website);

        let s = format!(
            "\
            HTTP/1.1 423 Blocked\r\n\
            Server: Traffic Service\r\n\
            Content-Length: {}\r\n\
            \r\n\
            {}",
            response.len(),
            response
        );

        match stream.write_all(s.as_bytes()) {
            Ok(_) => {
                stream.flush().unwrap();
                _ = stream.try_clone().expect("clone failed...");
                return
            },
            Err(_) => return
        }
    }

    if !DEAMON_MODE { 
        println!("Connecting to {}", website);
    }

    let method = request.method.unwrap();

    if !method.eq("CONNECT") {
        let url = Url::parse(&website).unwrap();
        let port_site = url.port().unwrap_or(80);

        let host = url.host().unwrap();
        website = format!("{}:{}", host, &port_site);
    }

    let mut tunnel = match TcpStream::connect(website) {
        Ok(t) => t,
        Err(err) => {
            println!("Error: {err}");
            return
        }
    };

    // Send an ack to the client
    if method.eq("CONNECT") {
        match stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n") {
            Ok(_) => (),
            Err(_) => return
        };
    } else {
        match tunnel.write_all( &buf) {
            Ok(_) => (),
            Err(_) => return
        };
       
    }

    // Set both sockets to nonblocking mode
    match stream.set_nonblocking(true) {
        Ok(()) => (),
        Err(_) => return
    };
    match tunnel.set_nonblocking(true) {
        Ok(()) => (),
        Err(_) => return
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
                Ok(0) => return, // Socket closed 
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => 0, // If there is no data, return 0 bytes written
                Err(_) => return
            };
        }
        // Read data from tunnel to be sent to stream -- only read if tunnel_buf is empty
        if tunnel_nbytes == 0 {
            tunnel_nbytes = match tunnel.read(&mut tunnel_buf) {
                Ok(0) => return , // Socket closed 
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => 0, // If there is no data, return 0 bytes written
                Err(_) => return 
            };
        }
        // Write data from stream to tunnel
        if stream_nbytes > 0 {
            // Pass the slice corresponding to first `stream_nbytes`
            match tunnel.write(&stream_buf[0..stream_nbytes]) {
                Ok(0) => return, // Socket closed 
                Ok(n) if n == stream_nbytes => stream_nbytes = 0, // If we get equal 
                Ok(_) => { println!("Cannot write partially :("); return }, // No support for partial nbytes
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => (), // Write the bytes in later
                Err(_) => return
            }
        }
        // Write data from tunnel to stream
        if tunnel_nbytes > 0 {
            // Pass the slice corresponding to first `stream_nbytes`
            match stream.write(& tunnel_buf[0..tunnel_nbytes]) {
                Ok(0) => return, // Socket closed
                Ok(n) if n == tunnel_nbytes => tunnel_nbytes = 0, // If we get equal 
                Ok(_) => { println!("Cannot write partially :("); return }, // No support for partial nbytes
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => (), // Write the bytes in later
                Err(_) => return
            }
        }
    }
 }

unsafe fn set_enable_by_pass() -> bool {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let enable_by_pass = &args[1];
        BYPASS = enable_by_pass == "1";
    } else {
        BYPASS = false;
    }

    BYPASS
}


fn main() -> std::io::Result<()> {
    unsafe { set_enable_by_pass() };
   
    // Create a server
    let local_addr = format!("localhost:{}", PORT);
    let server = TcpListener::bind(local_addr)?;
    println!("[*] Listening on port {}", PORT);
    // Keep spinning and spawn threads for any incoming connections
    for stream_result in server.incoming() {
        match stream_result {
            Ok(stream) => thread::spawn(move || handle_client(stream)), 
            _                     => continue
        };
    }
    Ok(())
}