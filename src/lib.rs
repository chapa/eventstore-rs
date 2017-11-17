extern crate chan;
extern crate core;
extern crate bytes;
extern crate uuid;

use core::option::Option;
use std::io::{ Cursor, Read };
use std::net::{ TcpStream, SocketAddrV4 };
use std::thread::{ spawn, JoinHandle };

use bytes::{ Buf, BytesMut, LittleEndian };
use bytes::buf::BufMut;
use chan::{ Sender, Receiver, async };
use uuid::Uuid;

pub struct Pkg {
    pub cmd:         u8,
    pub correlation: Uuid,
}

impl Pkg {
    pub fn new(cmd: u8, correlation: Uuid) -> Pkg {
        Pkg {
            cmd:         cmd,
            correlation: correlation,
        }
    }

    pub fn size(&self) -> u32 {
        18
    }

    // Copies the Pkg except its payload.
    pub fn copy_headers_only(&self) -> Pkg {
        Pkg {
            cmd:         self.cmd,
            correlation: self.correlation,
        }
    }

    pub fn to_bytes(&self) -> BytesMut {
        // FIXME - Use with_capacity instead.
        let mut bytes = BytesMut::new();

        bytes.put_u32::<LittleEndian>(self.size());
        bytes.put_u8(self.cmd);
        bytes.put_u8(0); // Package credential flag.
        bytes.put_slice(self.correlation.as_bytes());

        bytes
    }

    pub fn from_stream(stream: &mut TcpStream) -> Pkg {
        let mut frame: [u8; 4] = [0; 4];

        stream.read_exact(&mut frame).unwrap();

        let mut frame_cursor = Cursor::new(frame);
        let     frame_size   = frame_cursor.get_u32::<LittleEndian>() as usize;
        let mut pkg_buf      = vec![0; frame_size];

        stream.read_exact(&mut pkg_buf).unwrap();

        let cmd         = pkg_buf[0];
        let correlation = Uuid::from_bytes(&pkg_buf[2..18]).unwrap();

        Pkg::new(cmd, correlation)
    }
}

enum Msg {
    Start,
    Shutdown,
    Established(Uuid),
}

pub struct Client {
    worker: JoinHandle<()>,
    sender: Sender<Msg>,
}

struct Connection {
    id:     Uuid,
    sender: Sender<Pkg>,
    worker: JoinHandle<()>,
}

impl Connection {
    fn new(bus: Sender<Msg>, addr: SocketAddrV4) -> Connection {
        let (sender, recv) = async();
        let id             = Uuid::new_v4();
        let worker         = spawn(move || Connection::create_conn(id, recv, bus, addr));

        Connection {
            id:     id,
            sender: sender,
            worker: worker,
        }
    }

    fn create_conn(id: Uuid, rx: Receiver<Pkg>, bus: Sender<Msg>, addr: SocketAddrV4) {
        let stream = TcpStream::connect(addr).unwrap();

        bus.send(Msg::Established(id));
    }
}

impl Client {
    pub fn new(addr: SocketAddrV4) -> Client {
        let (sender, recv) = async();
        let tx             = sender.clone();
        let handle         = spawn(move || Client::worker_thread(addr, tx, recv));

        Client {
            worker: handle,
            sender: sender,
        }
    }

    fn worker_thread(addr: SocketAddrV4, bus: Sender<Msg>, queue: Receiver<Msg>) {
        let mut keep_going  = true;
        let mut connection = Option::None;
        let mut connected  = false;

        while keep_going {
            let msg_opt = queue.recv();

            match msg_opt {
                Option::Some(msg) => match msg {
                    Msg::Start => {
                        connection = Option::Some(Connection::new(bus.clone(), addr));
                    },

                    Msg::Shutdown => {
                        keep_going = false;
                        println!("Shutting down...");
                    },

                    Msg::Established(id) => {
                        for conn in &connection {
                            if conn.id == id {
                                connected = true;
                            }
                        }
                    },
                },

                Option::None => {
                    println!("Main bus closed");
                    keep_going = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let mut core = Core::new().unwrap();
        let handle   = core.handle();
        let host = "127.0.0.1".parse();

        let _ = TcpStream::connect(host, handle);

        assert_eq!(2 + 2, 4);

    }
}
