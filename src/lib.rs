pub mod ws {
    use tungstenite::{connect, Message, WebSocket};
    use tungstenite::stream::{Stream};
    use serde::{Deserialize};
    use std::net::{TcpStream};
    use native_tls::TlsStream;
    use std::collections::HashMap;
    use url::Url;
    use serde_json::{json, Result, Value};
    use std::thread;
    use std::time::Duration;
    
    pub struct ClientConfig {
        pub token: String,
        pub device: String,
        pub app: String,
        pub url: String
    }

    #[derive(Deserialize)]
    struct RegisterResponse {
        state: bool,
        message: String
    }

    #[derive(Deserialize)]
    struct Command {
        command: String,
        meta: Value,
        callbackId: String
    }

    pub struct Client {
        socket: WebSocket<Stream<TcpStream, TlsStream<TcpStream>>>,
        registered: bool,
        command_map: HashMap<String, Box<dyn 'static + Fn(String) -> Option<String>>>,
    }
    
    impl Client {
        fn construct_register_request(token: &str, device: &str, app: &str) -> Value {
            let register = json!({
                "type": "register",
                "payload": {
                    "gToken": token,
                    "devName": device,
                    "appName": app
                }
            });
            register
        }
    
        pub fn new(config: ClientConfig) -> Option<Client> {
            let (mut socket, response) = connect(Url::parse(&config.url).unwrap()).expect("Can't connect");
            let register_json = Client::construct_register_request(&config.token, &config.device, &config.app);
            socket
                .write_message(Message::Text(register_json.to_string()))
                .unwrap();
            let client = Client {
                socket: socket,
                registered: false,
                command_map: HashMap::new()
            };
            Some(client)
        }
    
        pub fn disconnect(&mut self) {
            self.socket.close(None).unwrap();
        }
    
        pub fn start(&mut self) {
            // TODO: Figure out a way for the client to handle it's own reading in a thread.
            // thread::spawn(|| {
            //     loop {
            //         self.read();
            //     }
            // });
            loop {
                self.read().unwrap();
                thread::sleep(Duration::from_millis(1));
            }
        }
    
        pub fn on<T> (
            self: &'_ mut Client,
            command: String,
            callback: impl 'static + Fn(T) -> Option<String>,
        )
        where
            for<'any>
                T : Deserialize<'any>
            ,
        {
            let handler = move |payload: String| -> Option<String> {
                // Deserialize the input into the generic struct that implements Deserialize
                let payload_value: T = serde_json::from_str(&payload).unwrap();
                // Call the callback with the deserialized value and return the string
                callback(payload_value)
            };
            self.command_map.insert(command, Box::new(handler));
        }
    
        fn read(&mut self) -> Result<()> {
            let res = self.socket.read_message().unwrap();
            let res_text = res.to_text().unwrap();
            let json_msg: Value = serde_json::from_str(&res_text)?;
            let payload_text = json_msg["payload"].to_string();
            let res_type = &json_msg["type"];
            if res_type == "registerResponse" {
                let register_res: RegisterResponse = serde_json::from_str(&payload_text).unwrap();
                self.registered = register_res.state;
                println!("Registered: {}", self.registered);
            } else if res_type == "command" {
                let command_obj: Command = serde_json::from_str(&payload_text).unwrap();
                let command = &command_obj.command;
                let meta = command_obj.meta.to_string();
                let callback_id = &command_obj.callbackId;
                if self.command_map.contains_key(command) {
                    let res = match self.command_map.get(command) {
                        Some(handle) => handle(meta),
                        None => None
                    };
                    if let Some(response) = res {
                        let res_json = json!({
                            "type": "response",
                            "payload": {
                                "callbackId": callback_id,
                                "meta": response
                            }
                        });
                        self.write(&res_json).unwrap();
                    }
                }
            }
            Ok(())
        }
    
        fn write(&mut self, val: &Value) -> Result<()> {
            self.socket.write_message(Message::Text(val.to_string())).unwrap();
            Ok(())
        }
    
        // fn send(&mut self, command: String, meta: Value, )
    }
}
